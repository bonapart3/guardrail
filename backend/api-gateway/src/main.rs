//! GuardRail API Gateway
//!
//! Unified entry point for all API requests with:
//! - JWT and API key authentication
//! - Rate limiting
//! - Request routing to internal services
//! - CORS handling

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, Method, Request, StatusCode, Uri},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{any, delete, get, patch, post},
    Json, Router,
};
use guardrail_shared::{crypto, ApiResponse, GuardRailError, Result};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

// ============================================================================
// Application State
// ============================================================================

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<GatewayConfig>,
    pub http_client: reqwest::Client,
}

#[derive(Clone, Debug)]
pub struct GatewayConfig {
    pub jwt_secret: String,
    pub jwt_expiry_hours: i64,
    pub identity_service_url: String,
    pub policy_engine_url: String,
    pub movement_ledger_url: String,
    pub chain_anchor_url: String,
    pub rate_limit_requests: u32,
    pub rate_limit_window_secs: u64,
}

// ============================================================================
// Authentication Types
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,        // User ID
    pub email: String,
    pub role: String,
    pub org_id: Option<String>,
    pub exp: usize,         // Expiry timestamp
    pub iat: usize,         // Issued at
}

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: Uuid,
    pub email: String,
    pub role: String,
    pub org_id: Option<Uuid>,
    pub auth_method: AuthMethod,
}

#[derive(Debug, Clone)]
pub enum AuthMethod {
    Jwt,
    ApiKey,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub user: UserInfo,
}

#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub id: Uuid,
    pub email: String,
    pub role: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateApiKeyRequest {
    pub name: String,
    pub scopes: Vec<String>,
    pub expires_in_days: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct CreateApiKeyResponse {
    pub id: Uuid,
    pub name: String,
    pub key: String,  // Only shown once
    pub prefix: String,
    pub scopes: Vec<String>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

// ============================================================================
// Health Check
// ============================================================================

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub version: String,
    pub services: HashMap<String, ServiceHealth>,
}

#[derive(Debug, Serialize)]
pub struct ServiceHealth {
    pub status: String,
    pub latency_ms: u64,
}

async fn health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let mut services = HashMap::new();
    
    // Check each service
    for (name, url) in [
        ("identity", &state.config.identity_service_url),
        ("policy", &state.config.policy_engine_url),
        ("ledger", &state.config.movement_ledger_url),
        ("anchor", &state.config.chain_anchor_url),
    ] {
        let start = std::time::Instant::now();
        let status = match state.http_client
            .get(format!("{}/health", url))
            .timeout(Duration::from_secs(5))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => "healthy",
            _ => "unhealthy",
        };
        
        services.insert(name.to_string(), ServiceHealth {
            status: status.to_string(),
            latency_ms: start.elapsed().as_millis() as u64,
        });
    }
    
    Json(HealthResponse {
        status: "healthy".to_string(),
        service: "api-gateway".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        services,
    })
}

// ============================================================================
// Authentication Handlers
// ============================================================================

async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> impl IntoResponse {
    match login_impl(&state, req).await {
        Ok(response) => (StatusCode::OK, Json(ApiResponse::success(response))),
        Err(e) => {
            let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::UNAUTHORIZED);
            (status, Json(ApiResponse::<LoginResponse>::error(e.error_code(), e.to_string())))
        }
    }
}

async fn login_impl(state: &AppState, req: LoginRequest) -> Result<LoginResponse> {
    // Find user by email
    let user = sqlx::query!(
        r#"
        SELECT id, email, password_hash, role, organization_id
        FROM users
        WHERE email = $1 AND is_active = true
        "#,
        req.email,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| GuardRailError::Unauthorized("Invalid credentials".to_string()))?;
    
    // Verify password
    let password_hash = user.password_hash
        .ok_or_else(|| GuardRailError::Unauthorized("Invalid credentials".to_string()))?;

    tracing::debug!("Password hash from DB: {}", password_hash);
    tracing::debug!("Password from request: {}", req.password);

    let parsed_hash = argon2::PasswordHash::new(&password_hash)
        .map_err(|e| {
            tracing::error!("Failed to parse hash: {:?}", e);
            GuardRailError::Unauthorized("Invalid credentials".to_string())
        })?;

    argon2::PasswordVerifier::verify_password(
        &argon2::Argon2::default(),
        req.password.as_bytes(),
        &parsed_hash,
    )
    .map_err(|e| {
        tracing::error!("Password verification failed: {:?}", e);
        GuardRailError::Unauthorized("Invalid credentials".to_string())
    })?;
    
    // Generate JWT
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as usize;
    
    let expiry = now + (state.config.jwt_expiry_hours as usize * 3600);
    
    let claims = Claims {
        sub: user.id.to_string(),
        email: user.email.clone(),
        role: user.role.clone(),
        org_id: user.organization_id.map(|id| id.to_string()),
        exp: expiry,
        iat: now,
    };
    
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.config.jwt_secret.as_bytes()),
    )
    .map_err(|e| GuardRailError::Internal(format!("Failed to generate token: {}", e)))?;
    
    // Update last login
    sqlx::query!(
        "UPDATE users SET last_login_at = NOW() WHERE id = $1",
        user.id,
    )
    .execute(&state.db)
    .await?;
    
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(state.config.jwt_expiry_hours);
    
    Ok(LoginResponse {
        token,
        expires_at,
        user: UserInfo {
            id: user.id,
            email: user.email,
            role: user.role,
        },
    })
}

async fn create_api_key(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<CreateApiKeyRequest>,
) -> impl IntoResponse {
    // Verify auth
    let user = match authenticate(&state, &headers).await {
        Ok(u) => u,
        Err(e) => return (StatusCode::UNAUTHORIZED, Json(ApiResponse::<CreateApiKeyResponse>::error("UNAUTHORIZED", e.to_string()))),
    };
    
    // Check permission (only admin can create API keys)
    if user.role != "ADMIN" && user.role != "SUPER_ADMIN" {
        return (StatusCode::FORBIDDEN, Json(ApiResponse::<CreateApiKeyResponse>::error("FORBIDDEN", "Insufficient permissions")));
    }
    
    match create_api_key_impl(&state, &user, req).await {
        Ok(response) => (StatusCode::CREATED, Json(ApiResponse::success(response))),
        Err(e) => {
            let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(ApiResponse::<CreateApiKeyResponse>::error(e.error_code(), e.to_string())))
        }
    }
}

async fn create_api_key_impl(
    state: &AppState,
    user: &AuthenticatedUser,
    req: CreateApiKeyRequest,
) -> Result<CreateApiKeyResponse> {
    let id = Uuid::new_v4();
    
    // Generate random API key
    let key_bytes: [u8; 32] = rand::random();
    let api_key = format!("gr_{}", hex::encode(key_bytes));
    let key_prefix = api_key[..10].to_string();
    
    // Hash the key for storage
    let key_hash = crypto::sha256_hex(api_key.as_bytes());
    
    let expires_at = req.expires_in_days.map(|days| {
        chrono::Utc::now() + chrono::Duration::days(days)
    });
    
    sqlx::query!(
        r#"
        INSERT INTO api_keys (id, name, key_hash, key_prefix, scopes, organization_id, created_by, expires_at, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW())
        "#,
        id,
        req.name,
        key_hash,
        &key_prefix,
        &req.scopes,
        user.org_id,
        user.user_id,
        expires_at,
    )
    .execute(&state.db)
    .await?;
    
    Ok(CreateApiKeyResponse {
        id,
        name: req.name,
        key: api_key,
        prefix: key_prefix,
        scopes: req.scopes,
        expires_at,
    })
}

// ============================================================================
// Authentication Middleware
// ============================================================================

async fn authenticate(state: &AppState, headers: &HeaderMap) -> Result<AuthenticatedUser> {
    // Check for API key first
    if let Some(api_key) = headers.get("x-api-key").and_then(|v| v.to_str().ok()) {
        return authenticate_api_key(state, api_key).await;
    }
    
    // Check for JWT
    if let Some(auth_header) = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()) {
        if auth_header.starts_with("Bearer ") {
            let token = &auth_header[7..];
            return authenticate_jwt(state, token).await;
        }
    }
    
    Err(GuardRailError::Unauthorized("No valid authentication provided".to_string()))
}

async fn authenticate_jwt(state: &AppState, token: &str) -> Result<AuthenticatedUser> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(state.config.jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|e| GuardRailError::Unauthorized(format!("Invalid token: {}", e)))?;
    
    let claims = token_data.claims;
    
    Ok(AuthenticatedUser {
        user_id: Uuid::parse_str(&claims.sub)
            .map_err(|_| GuardRailError::Unauthorized("Invalid user ID in token".to_string()))?,
        email: claims.email,
        role: claims.role,
        org_id: claims.org_id.and_then(|s| Uuid::parse_str(&s).ok()),
        auth_method: AuthMethod::Jwt,
    })
}

async fn authenticate_api_key(state: &AppState, api_key: &str) -> Result<AuthenticatedUser> {
    let key_hash = crypto::sha256_hex(api_key.as_bytes());
    
    let key_record = sqlx::query!(
        r#"
        SELECT ak.id, ak.scopes, ak.organization_id, ak.expires_at, u.id as user_id, u.email, u.role
        FROM api_keys ak
        JOIN users u ON ak.created_by = u.id
        WHERE ak.key_hash = $1 AND ak.is_active = true
        "#,
        key_hash,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| GuardRailError::Unauthorized("Invalid API key".to_string()))?;
    
    // Check expiry
    if let Some(expires_at) = key_record.expires_at {
        if expires_at < chrono::Utc::now() {
            return Err(GuardRailError::Unauthorized("API key expired".to_string()));
        }
    }
    
    // Update last used
    sqlx::query!(
        "UPDATE api_keys SET last_used_at = NOW() WHERE id = $1",
        key_record.id,
    )
    .execute(&state.db)
    .await?;
    
    Ok(AuthenticatedUser {
        user_id: key_record.user_id,
        email: key_record.email,
        role: key_record.role,
        org_id: key_record.organization_id,
        auth_method: AuthMethod::ApiKey,
    })
}

/// Auth middleware that rejects unauthenticated requests
async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    request: Request<Body>,
    next: Next,
) -> Response {
    match authenticate(&state, &headers).await {
        Ok(_user) => next.run(request).await,
        Err(e) => {
            let body = serde_json::to_string(&ApiResponse::<()>::error("UNAUTHORIZED", e.to_string()))
                .unwrap_or_default();
            Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap()
        }
    }
}

// ============================================================================
// Service Proxy
// ============================================================================

async fn proxy_to_service(
    state: &AppState,
    service_url: &str,
    method: Method,
    path: &str,
    query: Option<&str>,
    headers: &HeaderMap,
    body: Option<String>,
) -> Result<Response> {
    let url = if let Some(q) = query {
        format!("{}{}?{}", service_url, path, q)
    } else {
        format!("{}{}", service_url, path)
    };
    
    let mut request = state.http_client.request(method.clone(), &url);
    
    // Forward relevant headers
    if let Some(content_type) = headers.get(header::CONTENT_TYPE) {
        request = request.header(header::CONTENT_TYPE, content_type);
    }
    
    // Add body if present
    if let Some(body_content) = body {
        request = request.body(body_content);
    }
    
    let response = request
        .send()
        .await
        .map_err(|e| GuardRailError::ServiceUnavailable(format!("Service error: {}", e)))?;
    
    let status = response.status();
    let response_headers = response.headers().clone();
    let body = response.text().await
        .map_err(|e| GuardRailError::Internal(format!("Failed to read response: {}", e)))?;
    
    let mut builder = Response::builder().status(status);
    
    if let Some(content_type) = response_headers.get(header::CONTENT_TYPE) {
        builder = builder.header(header::CONTENT_TYPE, content_type);
    }
    
    Ok(builder.body(Body::from(body)).unwrap())
}

// ============================================================================
// Route Handlers (Proxied)
// ============================================================================

async fn proxy_identity(
    State(state): State<Arc<AppState>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Option<String>,
) -> impl IntoResponse {
    let path = uri.path();
    let query = uri.query();
    
    match proxy_to_service(&state, &state.config.identity_service_url, method, path, query, &headers, body).await {
        Ok(response) => response,
        Err(e) => {
            let body = serde_json::to_string(&ApiResponse::<()>::error(e.error_code(), e.to_string()))
                .unwrap_or_default();
            Response::builder()
                .status(StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap()
        }
    }
}

async fn proxy_policy(
    State(state): State<Arc<AppState>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Option<String>,
) -> impl IntoResponse {
    let path = uri.path();
    let query = uri.query();
    
    match proxy_to_service(&state, &state.config.policy_engine_url, method, path, query, &headers, body).await {
        Ok(response) => response,
        Err(e) => {
            let body = serde_json::to_string(&ApiResponse::<()>::error(e.error_code(), e.to_string()))
                .unwrap_or_default();
            Response::builder()
                .status(StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap()
        }
    }
}

async fn proxy_ledger(
    State(state): State<Arc<AppState>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Option<String>,
) -> impl IntoResponse {
    let path = uri.path();
    let query = uri.query();
    
    match proxy_to_service(&state, &state.config.movement_ledger_url, method, path, query, &headers, body).await {
        Ok(response) => response,
        Err(e) => {
            let body = serde_json::to_string(&ApiResponse::<()>::error(e.error_code(), e.to_string()))
                .unwrap_or_default();
            Response::builder()
                .status(StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap()
        }
    }
}

async fn proxy_anchor(
    State(state): State<Arc<AppState>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Option<String>,
) -> impl IntoResponse {
    let path = uri.path();
    let query = uri.query();
    
    match proxy_to_service(&state, &state.config.chain_anchor_url, method, path, query, &headers, body).await {
        Ok(response) => response,
        Err(e) => {
            let body = serde_json::to_string(&ApiResponse::<()>::error(e.error_code(), e.to_string()))
                .unwrap_or_default();
            Response::builder()
                .status(StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap()
        }
    }
}

// Helper to extract body from request
async fn extract_body(request: Request<Body>) -> (Method, Uri, HeaderMap, Option<String>) {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let headers = request.headers().clone();
    
    let body = match axum::body::to_bytes(request.into_body(), 10 * 1024 * 1024).await {
        Ok(bytes) if !bytes.is_empty() => Some(String::from_utf8_lossy(&bytes).to_string()),
        _ => None,
    };
    
    (method, uri, headers, body)
}

// Wrapper handlers that extract body first
async fn handle_identity(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
) -> impl IntoResponse {
    let (method, uri, headers, body) = extract_body(request).await;
    proxy_identity(State(state), method, uri, headers, body).await
}

async fn handle_policy(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
) -> impl IntoResponse {
    let (method, uri, headers, body) = extract_body(request).await;
    proxy_policy(State(state), method, uri, headers, body).await
}

async fn handle_ledger(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
) -> impl IntoResponse {
    let (method, uri, headers, body) = extract_body(request).await;
    proxy_ledger(State(state), method, uri, headers, body).await
}

async fn handle_anchor(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
) -> impl IntoResponse {
    let (method, uri, headers, body) = extract_body(request).await;
    proxy_anchor(State(state), method, uri, headers, body).await
}

// ============================================================================
// Router
// ============================================================================

fn create_router(state: Arc<AppState>) -> Router {
    // Public routes (no auth required)
    let public_routes = Router::new()
        .route("/health", get(health))
        .route("/api/v1/auth/login", post(login));
    
    // Protected routes (auth required)
    let protected_routes = Router::new()
        // API key management
        .route("/api/v1/auth/api-keys", post(create_api_key))
        // Identity service routes
        .route("/api/v1/identities", any(handle_identity))
        .route("/api/v1/identities/*path", any(handle_identity))
        // Policy engine routes
        .route("/api/v1/policies", any(handle_policy))
        .route("/api/v1/policies/*path", any(handle_policy))
        .route("/api/v1/check", any(handle_policy))
        // Movement ledger routes
        .route("/api/v1/events", any(handle_ledger))
        .route("/api/v1/events/*path", any(handle_ledger))
        .route("/api/v1/ledger/*path", any(handle_ledger))
        // Chain anchor routes
        .route("/api/v1/anchors", any(handle_anchor))
        .route("/api/v1/anchors/*path", any(handle_anchor))
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware));
    
    // CORS configuration
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::PATCH, Method::DELETE])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE, header::ACCEPT])
        .expose_headers([header::CONTENT_TYPE]);
    
    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .with_state(state)
        .layer(TraceLayer::new_for_http())
        .layer(cors)
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "api_gateway=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load environment variables
    dotenvy::dotenv().ok();

    // Database connection
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");

    let db = PgPoolOptions::new()
        .max_connections(20)
        .connect(&database_url)
        .await?;

    tracing::info!("Connected to database");

    // Load config
    let config = GatewayConfig {
        jwt_secret: std::env::var("JWT_SECRET")
            .unwrap_or_else(|_| "dev_secret_change_in_production".to_string()),
        jwt_expiry_hours: std::env::var("JWT_EXPIRY_HOURS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(24),
        identity_service_url: std::env::var("IDENTITY_SERVICE_URL")
            .unwrap_or_else(|_| "http://localhost:3001".to_string()),
        policy_engine_url: std::env::var("POLICY_ENGINE_URL")
            .unwrap_or_else(|_| "http://localhost:3002".to_string()),
        movement_ledger_url: std::env::var("MOVEMENT_LEDGER_URL")
            .unwrap_or_else(|_| "http://localhost:3003".to_string()),
        chain_anchor_url: std::env::var("CHAIN_ANCHOR_URL")
            .unwrap_or_else(|_| "http://localhost:3004".to_string()),
        rate_limit_requests: std::env::var("RATE_LIMIT_REQUESTS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(100),
        rate_limit_window_secs: std::env::var("RATE_LIMIT_WINDOW_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(60),
    };

    // HTTP client for proxying
    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    // Create app state
    let state = Arc::new(AppState {
        db,
        config: Arc::new(config),
        http_client,
    });

    // Create router
    let app = create_router(state);

    // Start server
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("0.0.0.0:{}", port);
    
    tracing::info!("API Gateway listening on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

// Need rand for API key generation
use rand::Rng;
