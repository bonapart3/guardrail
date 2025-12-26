//! GuardRail Policy Engine
//!
//! Evaluates actions against Rego policies using the regorus engine.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use guardrail_shared::{
    Action, ActionContext, ApiResponse, CheckActionRequest, CreatePolicyRequest,
    Decision, GuardRailError, PaginatedResponse, Policy, PolicyDecision, Result,
};
use regorus::Engine;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::RwLock;
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
    pub engine: Arc<RwLock<PolicyEngine>>,
}

/// Policy engine wrapper around regorus
pub struct PolicyEngine {
    engine: Engine,
    loaded_policies: Vec<Uuid>,
}

impl PolicyEngine {
    pub fn new() -> Self {
        Self {
            engine: Engine::new(),
            loaded_policies: Vec::new(),
        }
    }

    /// Load a policy into the engine
    pub fn load_policy(&mut self, policy_id: Uuid, name: &str, rego_source: &str) -> Result<()> {
        // Create a unique module name for this policy
        let module_name = format!("policy/{}", name);
        
        self.engine
            .add_policy(module_name, rego_source.to_string())
            .map_err(|e| GuardRailError::PolicyEvaluation(format!("Failed to load policy: {}", e)))?;
        
        self.loaded_policies.push(policy_id);
        Ok(())
    }

    /// Evaluate an action against loaded policies
    pub fn evaluate(&mut self, input: &serde_json::Value) -> Result<PolicyEvalResult> {
        // Set the input for evaluation - convert serde_json::Value to regorus::Value
        self.engine.set_input(input.clone().into());

        // Query for the decision
        // Default policy structure expects: data.guardrail.decision
        let query = "data.guardrail";
        
        let results = self.engine
            .eval_query(query.to_string(), false)
            .map_err(|e| GuardRailError::PolicyEvaluation(format!("Failed to evaluate: {}", e)))?;

        // Parse the results
        let decision = self.parse_decision(&results)?;
        
        Ok(decision)
    }

    fn parse_decision(&self, results: &regorus::QueryResults) -> Result<PolicyEvalResult> {
        // Default to ALLOW if no policies match
        let mut decision = Decision::Allow;
        let mut reasons: Vec<String> = Vec::new();
        let mut required_approvers: Vec<String> = Vec::new();

        for result in results.result.iter() {
            // Get bindings as object if possible
            if let Ok(obj) = result.bindings.as_object() {
                // Check for deny
                if let Some(deny) = obj.get(&"deny".into()) {
                    if let Ok(arr) = deny.as_array() {
                        if !arr.is_empty() {
                            decision = Decision::Deny;
                            for reason in arr {
                                if let Ok(s) = reason.as_string() {
                                    reasons.push(s.to_string());
                                }
                            }
                        }
                    } else if deny.as_bool().ok() == Some(&true) {
                        decision = Decision::Deny;
                    }
                }

                // Check for require_approval
                if let Some(approval) = obj.get(&"require_approval".into()) {
                    if let Ok(arr) = approval.as_array() {
                        if !arr.is_empty() && decision != Decision::Deny {
                            decision = Decision::RequireApproval;
                            for approver in arr {
                                if let Ok(s) = approver.as_string() {
                                    required_approvers.push(s.to_string());
                                }
                            }
                        }
                    } else if approval.as_bool().ok() == Some(&true) && decision != Decision::Deny {
                        decision = Decision::RequireApproval;
                    }
                }

                // Check for reasons
                if let Some(r) = obj.get(&"reasons".into()) {
                    if let Ok(arr) = r.as_array() {
                        for reason in arr {
                            if let Ok(s) = reason.as_string() {
                                if !reasons.contains(&s.to_string()) {
                                    reasons.push(s.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(PolicyEvalResult {
            decision,
            reasons,
            required_approvers,
        })
    }

    /// Reload all policies from database
    pub fn clear(&mut self) {
        self.engine = Engine::new();
        self.loaded_policies.clear();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyEvalResult {
    pub decision: Decision,
    pub reasons: Vec<String>,
    pub required_approvers: Vec<String>,
}

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub page: Option<i32>,
    pub per_page: Option<i32>,
    pub active_only: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct SimulateRequest {
    pub identity: serde_json::Value,
    pub action: Action,
    pub context: ActionContext,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub version: String,
    pub loaded_policies: usize,
}

// ============================================================================
// Handlers
// ============================================================================

async fn health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let engine = state.engine.read().await;
    Json(HealthResponse {
        status: "healthy".to_string(),
        service: "policy-engine".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        loaded_policies: engine.loaded_policies.len(),
    })
}

async fn create_policy(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreatePolicyRequest>,
) -> impl IntoResponse {
    match create_policy_impl(&state, req).await {
        Ok(policy) => (StatusCode::CREATED, Json(ApiResponse::success(policy))),
        Err(e) => {
            let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(ApiResponse::<Policy>::error(e.error_code(), e.to_string())))
        }
    }
}

async fn create_policy_impl(state: &AppState, req: CreatePolicyRequest) -> Result<Policy> {
    let id = Uuid::new_v4();
    let now = chrono::Utc::now();
    let version = "1.0.0".to_string();

    // Validate Rego syntax by trying to load it
    {
        let mut test_engine = PolicyEngine::new();
        test_engine.load_policy(id, &req.name, &req.rego_source)?;
    }

    let policy = sqlx::query_as!(
        Policy,
        r#"
        INSERT INTO policies (id, name, description, version, rego_source, is_active, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, true, $6, $6)
        RETURNING id, name, description, version, rego_source, is_active as "is_active!", created_by as "created_by!", created_at as "created_at!", updated_at as "updated_at!"
        "#,
        id,
        req.name,
        req.description,
        version,
        req.rego_source,
        now,
    )
    .fetch_one(&state.db)
    .await?;

    // Load into active engine
    {
        let mut engine = state.engine.write().await;
        engine.load_policy(id, &policy.name, &policy.rego_source)?;
    }

    Ok(policy)
}

async fn list_policies(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListQuery>,
) -> impl IntoResponse {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).min(100);
    let offset = (page - 1) * per_page;
    let active_only = query.active_only.unwrap_or(true);

    match list_policies_impl(&state.db, offset, per_page, active_only).await {
        Ok((policies, total)) => {
            let response = PaginatedResponse::new(policies, total, page, per_page);
            (StatusCode::OK, Json(ApiResponse::success(response)))
        }
        Err(e) => {
            let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(ApiResponse::<PaginatedResponse<Policy>>::error(e.error_code(), e.to_string())))
        }
    }
}

async fn list_policies_impl(
    db: &PgPool,
    offset: i32,
    limit: i32,
    active_only: bool,
) -> Result<(Vec<Policy>, i64)> {
    let policies = sqlx::query_as!(
        Policy,
        r#"
        SELECT id, name, description, version, rego_source, is_active as "is_active!", created_by as "created_by!", created_at as "created_at!", updated_at as "updated_at!"
        FROM policies
        WHERE ($3::boolean = false OR is_active = true)
        ORDER BY created_at DESC
        LIMIT $1 OFFSET $2
        "#,
        limit as i64,
        offset as i64,
        active_only,
    )
    .fetch_all(db)
    .await?;

    let total: i64 = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) as "count!"
        FROM policies
        WHERE ($1::boolean = false OR is_active = true)
        "#,
        active_only,
    )
    .fetch_one(db)
    .await?;

    Ok((policies, total))
}

async fn get_policy(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match get_policy_impl(&state.db, id).await {
        Ok(policy) => (StatusCode::OK, Json(ApiResponse::success(policy))),
        Err(e) => {
            let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(ApiResponse::<Policy>::error(e.error_code(), e.to_string())))
        }
    }
}

async fn get_policy_impl(db: &PgPool, id: Uuid) -> Result<Policy> {
    let policy = sqlx::query_as!(
        Policy,
        r#"
        SELECT id, name, description, version, rego_source, is_active as "is_active!", created_by as "created_by!", created_at as "created_at!", updated_at as "updated_at!"
        FROM policies
        WHERE id = $1
        "#,
        id,
    )
    .fetch_optional(db)
    .await?
    .ok_or_else(|| GuardRailError::PolicyNotFound(id.to_string()))?;

    Ok(policy)
}

async fn check_action(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CheckActionRequest>,
) -> impl IntoResponse {
    match check_action_impl(&state, req).await {
        Ok(decision) => (StatusCode::OK, Json(ApiResponse::success(decision))),
        Err(e) => {
            let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(ApiResponse::<PolicyDecision>::error(e.error_code(), e.to_string())))
        }
    }
}

async fn check_action_impl(state: &AppState, req: CheckActionRequest) -> Result<PolicyDecision> {
    // Get identity with credentials
    let identity = sqlx::query!(
        r#"
        SELECT id, identity_type as "identity_type: String", display_name, metadata
        FROM identities
        WHERE id = $1 AND is_active = true
        "#,
        req.identity_id,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| GuardRailError::IdentityNotFound(req.identity_id.to_string()))?;

    // Get credentials for identity
    let credentials = sqlx::query!(
        r#"
        SELECT credential_type as "credential_type: String", provider, value
        FROM credentials
        WHERE identity_id = $1
        "#,
        req.identity_id,
    )
    .fetch_all(&state.db)
    .await?;

    // Build input for policy evaluation
    let input = serde_json::json!({
        "identity": {
            "id": identity.id.to_string(),
            "type": identity.identity_type,
            "display_name": identity.display_name,
            "metadata": identity.metadata,
            "credentials": credentials.iter().map(|c| serde_json::json!({
                "type": c.credential_type,
                "provider": c.provider,
                "value": c.value,
            })).collect::<Vec<_>>(),
        },
        "action": req.action,
        "context": req.context,
    });

    // Evaluate policies
    let eval_result = {
        let mut engine = state.engine.write().await;
        engine.evaluate(&input)?
    };

    // Get first active policy for recording (simplified - should aggregate in production)
    let policy = sqlx::query!(
        r#"
        SELECT id, version
        FROM policies
        WHERE is_active = true
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .fetch_optional(&state.db)
    .await?;

    let decision_id = Uuid::new_v4();
    let now = chrono::Utc::now();

    // Record decision in database
    if let Some(p) = &policy {
        sqlx::query!(
            r#"
            INSERT INTO policy_decisions (id, identity_id, policy_id, policy_version, action_type, action_payload, context, decision, reasons, required_approvers, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
            decision_id,
            req.identity_id,
            p.id,
            p.version,
            format!("{:?}", req.action.action_type),
            serde_json::to_value(&req.action)?,
            serde_json::to_value(&req.context)?,
            eval_result.decision as Decision,
            &eval_result.reasons,
            &eval_result.required_approvers,
            now,
        )
        .execute(&state.db)
        .await?;
    }

    Ok(PolicyDecision {
        decision_id,
        decision: eval_result.decision,
        reasons: eval_result.reasons,
        required_approvers: eval_result.required_approvers,
        policy_id: policy.as_ref().map(|p| p.id).unwrap_or(Uuid::nil()),
        policy_version: policy.as_ref().map(|p| p.version.clone()).unwrap_or_default(),
        evaluated_at: now,
    })
}

async fn simulate_policy(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<SimulateRequest>,
) -> impl IntoResponse {
    match simulate_policy_impl(&state, id, req).await {
        Ok(result) => (StatusCode::OK, Json(ApiResponse::success(result))),
        Err(e) => {
            let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(ApiResponse::<PolicyEvalResult>::error(e.error_code(), e.to_string())))
        }
    }
}

async fn simulate_policy_impl(
    state: &AppState,
    policy_id: Uuid,
    req: SimulateRequest,
) -> Result<PolicyEvalResult> {
    // Get the policy
    let policy = get_policy_impl(&state.db, policy_id).await?;

    // Create a fresh engine with just this policy
    let mut engine = PolicyEngine::new();
    engine.load_policy(policy.id, &policy.name, &policy.rego_source)?;

    // Build input
    let input = serde_json::json!({
        "identity": req.identity,
        "action": req.action,
        "context": req.context,
    });

    // Evaluate
    let result = engine.evaluate(&input)?;

    Ok(result)
}

async fn activate_policy(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match activate_policy_impl(&state, id, true).await {
        Ok(policy) => (StatusCode::OK, Json(ApiResponse::success(policy))),
        Err(e) => {
            let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(ApiResponse::<Policy>::error(e.error_code(), e.to_string())))
        }
    }
}

async fn deactivate_policy(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match activate_policy_impl(&state, id, false).await {
        Ok(policy) => (StatusCode::OK, Json(ApiResponse::success(policy))),
        Err(e) => {
            let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(ApiResponse::<Policy>::error(e.error_code(), e.to_string())))
        }
    }
}

async fn activate_policy_impl(state: &AppState, id: Uuid, active: bool) -> Result<Policy> {
    let now = chrono::Utc::now();

    let policy = sqlx::query_as!(
        Policy,
        r#"
        UPDATE policies
        SET is_active = $2, updated_at = $3
        WHERE id = $1
        RETURNING id, name, description, version, rego_source, is_active as "is_active!", created_by as "created_by!", created_at as "created_at!", updated_at as "updated_at!"
        "#,
        id,
        active,
        now,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| GuardRailError::PolicyNotFound(id.to_string()))?;

    // Reload policies in engine
    reload_policies(state).await?;

    Ok(policy)
}

async fn reload_policies(state: &AppState) -> Result<()> {
    let policies = sqlx::query!(
        r#"
        SELECT id, name, rego_source
        FROM policies
        WHERE is_active = true
        "#,
    )
    .fetch_all(&state.db)
    .await?;

    let mut engine = state.engine.write().await;
    engine.clear();

    for policy in policies {
        if let Err(e) = engine.load_policy(policy.id, &policy.name, &policy.rego_source) {
            tracing::error!("Failed to load policy {}: {}", policy.name, e);
        }
    }

    Ok(())
}

// ============================================================================
// Router
// ============================================================================

fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        // Health check
        .route("/health", get(health))
        // Policy CRUD
        .route("/api/v1/policies", post(create_policy))
        .route("/api/v1/policies", get(list_policies))
        .route("/api/v1/policies/:id", get(get_policy))
        .route("/api/v1/policies/:id/activate", post(activate_policy))
        .route("/api/v1/policies/:id/deactivate", post(deactivate_policy))
        .route("/api/v1/policies/:id/simulate", post(simulate_policy))
        // Action checking
        .route("/api/v1/check", post(check_action))
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
                .unwrap_or_else(|_| "policy_engine=debug,tower_http=debug".into()),
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

    // Create policy engine
    let engine = Arc::new(RwLock::new(PolicyEngine::new()));

    // Create app state
    let state = Arc::new(AppState { db: db.clone(), engine });

    // Load active policies
    reload_policies(&state).await?;
    tracing::info!("Loaded active policies");

    // Create router
    let app = create_router(state);

    // Start server
    let port = std::env::var("PORT").unwrap_or_else(|_| "3002".to_string());
    let addr = format!("0.0.0.0:{}", port);
    
    tracing::info!("Policy Engine listening on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
