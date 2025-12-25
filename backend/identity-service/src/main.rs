//! GuardRail Identity Service
//!
//! Manages identities (humans, agents, organizations), their keys, and credentials.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, patch, post},
    Json, Router,
};
use guardrail_shared::{
    ApiResponse, CreateIdentityRequest, Identity, IdentityKey, Credential,
    PaginatedResponse, GuardRailError, Result,
};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

// ============================================================================
// Application State
// ============================================================================

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
}

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub page: Option<i32>,
    pub per_page: Option<i32>,
    pub identity_type: Option<String>,
    pub search: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AttachKeyRequest {
    pub key_type: String,
    pub public_key: String,
    pub chain: Option<String>,
    pub label: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddCredentialRequest {
    pub credential_type: String,
    pub provider: String,
    pub value: serde_json::Value,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub version: String,
}

// ============================================================================
// Handlers
// ============================================================================

async fn health() -> impl IntoResponse {
    Json(HealthResponse {
        status: "healthy".to_string(),
        service: "identity-service".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

async fn create_identity(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateIdentityRequest>,
) -> impl IntoResponse {
    match create_identity_impl(&state.db, req).await {
        Ok(identity) => (StatusCode::CREATED, Json(ApiResponse::success(identity))),
        Err(e) => {
            let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(ApiResponse::<Identity>::error(e.error_code(), e.to_string())))
        }
    }
}

async fn create_identity_impl(db: &PgPool, req: CreateIdentityRequest) -> Result<Identity> {
    let id = Uuid::new_v4();
    let now = chrono::Utc::now();
    let metadata = req.metadata.unwrap_or(serde_json::json!({}));

    let identity = sqlx::query_as!(
        Identity,
        r#"
        INSERT INTO identities (id, identity_type, external_id, display_name, metadata, organization_id, is_active, created_at, updated_at)
        VALUES ($1, $2::identity_type, $3, $4, $5, $6, true, $7, $7)
        RETURNING id, identity_type as "identity_type: _", external_id, display_name, metadata, organization_id, is_active, created_at, updated_at
        "#,
        id,
        req.identity_type.to_string(),
        req.external_id,
        req.display_name,
        metadata,
        req.organization_id,
        now,
    )
    .fetch_one(db)
    .await?;

    Ok(identity)
}

async fn list_identities(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListQuery>,
) -> impl IntoResponse {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).min(100);
    let offset = (page - 1) * per_page;

    match list_identities_impl(&state.db, offset, per_page, query.search).await {
        Ok((identities, total)) => {
            let response = PaginatedResponse::new(identities, total, page, per_page);
            (StatusCode::OK, Json(ApiResponse::success(response)))
        }
        Err(e) => {
            let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(ApiResponse::<PaginatedResponse<Identity>>::error(e.error_code(), e.to_string())))
        }
    }
}

async fn list_identities_impl(
    db: &PgPool,
    offset: i32,
    limit: i32,
    search: Option<String>,
) -> Result<(Vec<Identity>, i64)> {
    let search_pattern = search.map(|s| format!("%{}%", s));

    let identities = sqlx::query_as!(
        Identity,
        r#"
        SELECT id, identity_type as "identity_type: _", external_id, display_name, metadata, organization_id, is_active, created_at, updated_at
        FROM identities
        WHERE is_active = true
        AND ($3::text IS NULL OR display_name ILIKE $3 OR external_id ILIKE $3)
        ORDER BY created_at DESC
        LIMIT $1 OFFSET $2
        "#,
        limit as i64,
        offset as i64,
        search_pattern,
    )
    .fetch_all(db)
    .await?;

    let total: i64 = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) as "count!"
        FROM identities
        WHERE is_active = true
        AND ($1::text IS NULL OR display_name ILIKE $1 OR external_id ILIKE $1)
        "#,
        search_pattern,
    )
    .fetch_one(db)
    .await?;

    Ok((identities, total))
}

async fn get_identity(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match get_identity_impl(&state.db, id).await {
        Ok(identity) => (StatusCode::OK, Json(ApiResponse::success(identity))),
        Err(e) => {
            let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(ApiResponse::<Identity>::error(e.error_code(), e.to_string())))
        }
    }
}

async fn get_identity_impl(db: &PgPool, id: Uuid) -> Result<Identity> {
    let identity = sqlx::query_as!(
        Identity,
        r#"
        SELECT id, identity_type as "identity_type: _", external_id, display_name, metadata, organization_id, is_active, created_at, updated_at
        FROM identities
        WHERE id = $1 AND is_active = true
        "#,
        id,
    )
    .fetch_optional(db)
    .await?
    .ok_or_else(|| GuardRailError::IdentityNotFound(id.to_string()))?;

    Ok(identity)
}

async fn update_identity(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<serde_json::Value>,
) -> impl IntoResponse {
    match update_identity_impl(&state.db, id, req).await {
        Ok(identity) => (StatusCode::OK, Json(ApiResponse::success(identity))),
        Err(e) => {
            let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(ApiResponse::<Identity>::error(e.error_code(), e.to_string())))
        }
    }
}

async fn update_identity_impl(db: &PgPool, id: Uuid, updates: serde_json::Value) -> Result<Identity> {
    let now = chrono::Utc::now();
    
    // Get current identity first
    let current = get_identity_impl(db, id).await?;
    
    // Apply updates
    let display_name = updates.get("display_name")
        .and_then(|v| v.as_str())
        .unwrap_or(&current.display_name);
    
    let metadata = updates.get("metadata")
        .cloned()
        .unwrap_or(current.metadata);

    let identity = sqlx::query_as!(
        Identity,
        r#"
        UPDATE identities
        SET display_name = $2, metadata = $3, updated_at = $4
        WHERE id = $1 AND is_active = true
        RETURNING id, identity_type as "identity_type: _", external_id, display_name, metadata, organization_id, is_active, created_at, updated_at
        "#,
        id,
        display_name,
        metadata,
        now,
    )
    .fetch_optional(db)
    .await?
    .ok_or_else(|| GuardRailError::IdentityNotFound(id.to_string()))?;

    Ok(identity)
}

async fn delete_identity(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match delete_identity_impl(&state.db, id).await {
        Ok(_) => (StatusCode::NO_CONTENT, Json(ApiResponse::<()>::success(()))),
        Err(e) => {
            let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(ApiResponse::<()>::error(e.error_code(), e.to_string())))
        }
    }
}

async fn delete_identity_impl(db: &PgPool, id: Uuid) -> Result<()> {
    let now = chrono::Utc::now();
    
    let result = sqlx::query!(
        r#"
        UPDATE identities
        SET is_active = false, updated_at = $2
        WHERE id = $1 AND is_active = true
        "#,
        id,
        now,
    )
    .execute(db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(GuardRailError::IdentityNotFound(id.to_string()));
    }

    Ok(())
}

async fn attach_key(
    State(state): State<Arc<AppState>>,
    Path(identity_id): Path<Uuid>,
    Json(req): Json<AttachKeyRequest>,
) -> impl IntoResponse {
    match attach_key_impl(&state.db, identity_id, req).await {
        Ok(key) => (StatusCode::CREATED, Json(ApiResponse::success(key))),
        Err(e) => {
            let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(ApiResponse::<IdentityKey>::error(e.error_code(), e.to_string())))
        }
    }
}

async fn attach_key_impl(db: &PgPool, identity_id: Uuid, req: AttachKeyRequest) -> Result<IdentityKey> {
    // Verify identity exists
    let _ = get_identity_impl(db, identity_id).await?;

    let id = Uuid::new_v4();
    let now = chrono::Utc::now();

    let key = sqlx::query_as!(
        IdentityKey,
        r#"
        INSERT INTO identity_keys (id, identity_id, key_type, public_key, chain, label, is_primary, created_at)
        VALUES ($1, $2, $3::key_type, $4, $5, $6, false, $7)
        RETURNING id, identity_id, key_type as "key_type: _", public_key, chain, label, is_primary, verified_at, created_at
        "#,
        id,
        identity_id,
        req.key_type,
        req.public_key,
        req.chain,
        req.label,
        now,
    )
    .fetch_one(db)
    .await?;

    Ok(key)
}

async fn detach_key(
    State(state): State<Arc<AppState>>,
    Path((identity_id, key_id)): Path<(Uuid, Uuid)>,
) -> impl IntoResponse {
    match detach_key_impl(&state.db, identity_id, key_id).await {
        Ok(_) => (StatusCode::NO_CONTENT, Json(ApiResponse::<()>::success(()))),
        Err(e) => {
            let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(ApiResponse::<()>::error(e.error_code(), e.to_string())))
        }
    }
}

async fn detach_key_impl(db: &PgPool, identity_id: Uuid, key_id: Uuid) -> Result<()> {
    let result = sqlx::query!(
        r#"
        DELETE FROM identity_keys
        WHERE id = $1 AND identity_id = $2
        "#,
        key_id,
        identity_id,
    )
    .execute(db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(GuardRailError::IdentityNotFound(format!("key {} for identity {}", key_id, identity_id)));
    }

    Ok(())
}

async fn add_credential(
    State(state): State<Arc<AppState>>,
    Path(identity_id): Path<Uuid>,
    Json(req): Json<AddCredentialRequest>,
) -> impl IntoResponse {
    match add_credential_impl(&state.db, identity_id, req).await {
        Ok(credential) => (StatusCode::CREATED, Json(ApiResponse::success(credential))),
        Err(e) => {
            let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(ApiResponse::<Credential>::error(e.error_code(), e.to_string())))
        }
    }
}

async fn add_credential_impl(db: &PgPool, identity_id: Uuid, req: AddCredentialRequest) -> Result<Credential> {
    // Verify identity exists
    let _ = get_identity_impl(db, identity_id).await?;

    let id = Uuid::new_v4();
    let now = chrono::Utc::now();

    let credential = sqlx::query_as!(
        Credential,
        r#"
        INSERT INTO credentials (id, identity_id, credential_type, provider, value, expires_at, verified_at, created_at, updated_at)
        VALUES ($1, $2, $3::credential_type, $4, $5, $6, $7, $7, $7)
        RETURNING id, identity_id, credential_type as "credential_type: _", provider, value, expires_at, verified_at, created_at, updated_at
        "#,
        id,
        identity_id,
        req.credential_type,
        req.provider,
        req.value,
        req.expires_at,
        now,
    )
    .fetch_one(db)
    .await?;

    Ok(credential)
}

// ============================================================================
// Router
// ============================================================================

fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        // Health check
        .route("/health", get(health))
        // Identity CRUD
        .route("/api/v1/identities", post(create_identity))
        .route("/api/v1/identities", get(list_identities))
        .route("/api/v1/identities/:id", get(get_identity))
        .route("/api/v1/identities/:id", patch(update_identity))
        .route("/api/v1/identities/:id", delete(delete_identity))
        // Key management
        .route("/api/v1/identities/:id/keys", post(attach_key))
        .route("/api/v1/identities/:id/keys/:key_id", delete(detach_key))
        // Credential management
        .route("/api/v1/identities/:id/credentials", post(add_credential))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
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
                .unwrap_or_else(|_| "identity_service=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load environment variables
    dotenvy::dotenv().ok();

    // Database connection
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");

    let db = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await?;

    tracing::info!("Connected to database");

    // Create app state
    let state = Arc::new(AppState { db });

    // Create router
    let app = create_router(state);

    // Start server
    let port = std::env::var("PORT").unwrap_or_else(|_| "3001".to_string());
    let addr = format!("0.0.0.0:{}", port);
    
    tracing::info!("Identity Service listening on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
