//! GuardRail Movement Ledger
//!
//! Append-only event store with hash-chaining for tamper-evident audit trails.
//! Implements event sourcing with CQRS patterns.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use guardrail_shared::{
    crypto, ApiResponse, EventType, GuardRailError, MovementEvent, PaginatedResponse, Result,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
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
    pub last_event: Arc<RwLock<Option<LastEventInfo>>>,
}

#[derive(Clone, Debug)]
pub struct LastEventInfo {
    pub sequence_number: i64,
    pub event_hash: String,
}

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct ListEventsQuery {
    pub page: Option<i32>,
    pub per_page: Option<i32>,
    pub event_type: Option<String>,
    pub actor_id: Option<Uuid>,
    pub from_date: Option<chrono::DateTime<chrono::Utc>>,
    pub to_date: Option<chrono::DateTime<chrono::Utc>>,
    pub anchored_only: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateEventRequest {
    pub event_type: EventType,
    pub actor_id: Uuid,
    pub policy_decision_id: Option<Uuid>,
    pub payload: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct EventWithProof {
    pub event: MovementEvent,
    pub proof: Option<MerkleProof>,
}

#[derive(Debug, Serialize, Clone)]
pub struct MerkleProof {
    pub event_hash: String,
    pub siblings: Vec<ProofSibling>,
    pub root: String,
    pub anchor_batch_id: Uuid,
    pub ethereum_tx_hash: Option<String>,
    pub solana_tx_signature: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ProofSibling {
    pub hash: String,
    pub position: String, // "left" or "right"
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub version: String,
    pub total_events: i64,
    pub last_sequence: i64,
}

#[derive(Debug, Serialize)]
pub struct LedgerStats {
    pub total_events: i64,
    pub events_by_type: Vec<EventTypeCount>,
    pub unanchored_events: i64,
    pub last_anchor_time: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Serialize)]
pub struct EventTypeCount {
    pub event_type: String,
    pub count: i64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ExportRequest {
    pub from_sequence: Option<i64>,
    pub to_sequence: Option<i64>,
    pub from_date: Option<chrono::DateTime<chrono::Utc>>,
    pub to_date: Option<chrono::DateTime<chrono::Utc>>,
    pub include_proofs: bool,
}

#[derive(Debug, Serialize)]
pub struct ExportResponse {
    pub export_id: Uuid,
    pub event_count: i64,
    pub from_sequence: i64,
    pub to_sequence: i64,
    pub merkle_root: String,
    pub events: Vec<MovementEvent>,
    pub signature: String,
    pub exported_at: chrono::DateTime<chrono::Utc>,
}

// ============================================================================
// Hash Chain Implementation
// ============================================================================

/// Compute the hash of an event for chain linking
fn compute_event_hash(
    sequence_number: i64,
    event_type: &EventType,
    actor_id: &Uuid,
    payload: &serde_json::Value,
    previous_hash: &str,
    timestamp: &chrono::DateTime<chrono::Utc>,
) -> String {
    let mut hasher = Sha256::new();
    
    // Include all critical fields in hash
    hasher.update(sequence_number.to_le_bytes());
    hasher.update(format!("{:?}", event_type).as_bytes());
    hasher.update(actor_id.as_bytes());
    hasher.update(payload.to_string().as_bytes());
    hasher.update(previous_hash.as_bytes());
    hasher.update(timestamp.to_rfc3339().as_bytes());
    
    hex::encode(hasher.finalize())
}

/// Verify the hash chain integrity for a sequence of events
fn verify_hash_chain(events: &[MovementEvent]) -> bool {
    for i in 1..events.len() {
        let prev = &events[i - 1];
        let curr = &events[i];
        
        // Current event's previous_hash should match previous event's event_hash
        if curr.previous_hash != prev.event_hash {
            return false;
        }
        
        // Verify the event's own hash is correct
        let computed = compute_event_hash(
            curr.sequence_number,
            &curr.event_type,
            &curr.actor_id,
            &curr.payload,
            &curr.previous_hash,
            &curr.created_at,
        );
        
        if computed != curr.event_hash {
            return false;
        }
    }
    true
}

// ============================================================================
// Merkle Tree for Anchoring
// ============================================================================

/// Build a Merkle tree from event hashes and return the root
pub fn build_merkle_root(event_hashes: &[String]) -> String {
    if event_hashes.is_empty() {
        return "0".repeat(64);
    }
    
    if event_hashes.len() == 1 {
        return event_hashes[0].clone();
    }
    
    let mut current_level: Vec<String> = event_hashes.to_vec();
    
    // Pad to power of 2 if needed
    while current_level.len() & (current_level.len() - 1) != 0 {
        if let Some(last) = current_level.last() {
            current_level.push(last.clone());
        }
    }
    
    while current_level.len() > 1 {
        let mut next_level = Vec::new();
        
        for chunk in current_level.chunks(2) {
            let mut hasher = Sha256::new();
            hasher.update(&chunk[0]);
            hasher.update(&chunk[1]);
            next_level.push(hex::encode(hasher.finalize()));
        }
        
        current_level = next_level;
    }
    
    current_level[0].clone()
}

/// Generate a Merkle proof for a specific event
pub fn generate_merkle_proof(event_hashes: &[String], target_index: usize) -> Vec<ProofSibling> {
    if event_hashes.len() <= 1 {
        return Vec::new();
    }
    
    let mut proof = Vec::new();
    let mut current_level: Vec<String> = event_hashes.to_vec();
    let mut index = target_index;
    
    // Pad to power of 2
    while current_level.len() & (current_level.len() - 1) != 0 {
        if let Some(last) = current_level.last() {
            current_level.push(last.clone());
        }
    }
    
    while current_level.len() > 1 {
        let sibling_index = if index % 2 == 0 { index + 1 } else { index - 1 };
        let position = if index % 2 == 0 { "right" } else { "left" };
        
        if sibling_index < current_level.len() {
            proof.push(ProofSibling {
                hash: current_level[sibling_index].clone(),
                position: position.to_string(),
            });
        }
        
        // Move to next level
        let mut next_level = Vec::new();
        for chunk in current_level.chunks(2) {
            let mut hasher = Sha256::new();
            hasher.update(&chunk[0]);
            hasher.update(&chunk[1]);
            next_level.push(hex::encode(hasher.finalize()));
        }
        
        current_level = next_level;
        index /= 2;
    }
    
    proof
}

/// Verify a Merkle proof
pub fn verify_merkle_proof(event_hash: &str, proof: &[ProofSibling], root: &str) -> bool {
    let mut current_hash = event_hash.to_string();
    
    for sibling in proof {
        let mut hasher = Sha256::new();
        
        if sibling.position == "left" {
            hasher.update(&sibling.hash);
            hasher.update(&current_hash);
        } else {
            hasher.update(&current_hash);
            hasher.update(&sibling.hash);
        }
        
        current_hash = hex::encode(hasher.finalize());
    }
    
    current_hash == root
}

// ============================================================================
// Handlers
// ============================================================================

async fn health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let last = state.last_event.read().await;
    let (total, seq) = match &*last {
        Some(info) => {
            let count: i64 = sqlx::query_scalar!("SELECT COUNT(*) as \"count!\" FROM movement_events")
                .fetch_one(&state.db)
                .await
                .unwrap_or(0);
            (count, info.sequence_number)
        }
        None => (0, 0),
    };
    
    Json(HealthResponse {
        status: "healthy".to_string(),
        service: "movement-ledger".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        total_events: total,
        last_sequence: seq,
    })
}

async fn create_event(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateEventRequest>,
) -> impl IntoResponse {
    match create_event_impl(&state, req).await {
        Ok(event) => (StatusCode::CREATED, Json(ApiResponse::success(event))),
        Err(e) => {
            let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(ApiResponse::<MovementEvent>::error(e.error_code(), e.to_string())))
        }
    }
}

async fn create_event_impl(state: &AppState, req: CreateEventRequest) -> Result<MovementEvent> {
    let id = Uuid::new_v4();
    let now = chrono::Utc::now();
    
    // Get previous event info for hash chain
    let (previous_hash, next_sequence) = {
        let last = state.last_event.read().await;
        match &*last {
            Some(info) => (info.event_hash.clone(), info.sequence_number + 1),
            None => {
                // Genesis event - use zeros
                ("0".repeat(64), 1)
            }
        }
    };
    
    // Compute hash for this event
    let event_hash = compute_event_hash(
        next_sequence,
        &req.event_type,
        &req.actor_id,
        &req.payload,
        &previous_hash,
        &now,
    );
    
    // Insert event (append-only)
    let event = sqlx::query_as!(
        MovementEvent,
        r#"
        INSERT INTO movement_events (id, event_type, actor_id, policy_decision_id, payload, previous_hash, event_hash, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING id, sequence_number, event_type as "event_type: EventType", actor_id, policy_decision_id, payload, previous_hash, event_hash, anchor_batch_id, created_at as "created_at!"
        "#,
        id,
        req.event_type as EventType,
        req.actor_id,
        req.policy_decision_id,
        req.payload,
        previous_hash,
        event_hash,
        now,
    )
    .fetch_one(&state.db)
    .await?;
    
    // Update last event cache
    {
        let mut last = state.last_event.write().await;
        *last = Some(LastEventInfo {
            sequence_number: event.sequence_number,
            event_hash: event.event_hash.clone(),
        });
    }
    
    Ok(event)
}

async fn list_events(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListEventsQuery>,
) -> impl IntoResponse {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(50).min(500);
    let offset = (page - 1) * per_page;

    match list_events_impl(&state.db, offset, per_page, &query).await {
        Ok((events, total)) => {
            let response = PaginatedResponse::new(events, total, page, per_page);
            (StatusCode::OK, Json(ApiResponse::success(response)))
        }
        Err(e) => {
            let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(ApiResponse::<PaginatedResponse<MovementEvent>>::error(e.error_code(), e.to_string())))
        }
    }
}

async fn list_events_impl(
    db: &PgPool,
    offset: i32,
    limit: i32,
    query: &ListEventsQuery,
) -> Result<(Vec<MovementEvent>, i64)> {
    // Build dynamic query based on filters
    let events = sqlx::query_as!(
        MovementEvent,
        r#"
        SELECT id, sequence_number, event_type as "event_type: EventType", actor_id, policy_decision_id, payload, previous_hash, event_hash, anchor_batch_id, created_at as "created_at!"
        FROM movement_events
        WHERE ($3::uuid IS NULL OR actor_id = $3)
        AND ($4::timestamptz IS NULL OR created_at >= $4)
        AND ($5::timestamptz IS NULL OR created_at <= $5)
        AND ($6::boolean IS NULL OR ($6 = true AND anchor_batch_id IS NOT NULL) OR $6 = false)
        ORDER BY sequence_number DESC
        LIMIT $1 OFFSET $2
        "#,
        limit as i64,
        offset as i64,
        query.actor_id,
        query.from_date,
        query.to_date,
        query.anchored_only,
    )
    .fetch_all(db)
    .await?;

    let total: i64 = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) as "count!"
        FROM movement_events
        WHERE ($1::uuid IS NULL OR actor_id = $1)
        AND ($2::timestamptz IS NULL OR created_at >= $2)
        AND ($3::timestamptz IS NULL OR created_at <= $3)
        AND ($4::boolean IS NULL OR ($4 = true AND anchor_batch_id IS NOT NULL) OR $4 = false)
        "#,
        query.actor_id,
        query.from_date,
        query.to_date,
        query.anchored_only,
    )
    .fetch_one(db)
    .await?;

    Ok((events, total))
}

async fn get_event(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match get_event_impl(&state.db, id).await {
        Ok(event) => (StatusCode::OK, Json(ApiResponse::success(event))),
        Err(e) => {
            let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(ApiResponse::<MovementEvent>::error(e.error_code(), e.to_string())))
        }
    }
}

async fn get_event_impl(db: &PgPool, id: Uuid) -> Result<MovementEvent> {
    let event = sqlx::query_as!(
        MovementEvent,
        r#"
        SELECT id, sequence_number, event_type as "event_type: EventType", actor_id, policy_decision_id, payload, previous_hash, event_hash, anchor_batch_id, created_at as "created_at!"
        FROM movement_events
        WHERE id = $1
        "#,
        id,
    )
    .fetch_optional(db)
    .await?
    .ok_or_else(|| GuardRailError::NotFound(format!("Event {} not found", id)))?;

    Ok(event)
}

async fn get_event_proof(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match get_event_proof_impl(&state.db, id).await {
        Ok(proof) => (StatusCode::OK, Json(ApiResponse::success(proof))),
        Err(e) => {
            let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(ApiResponse::<EventWithProof>::error(e.error_code(), e.to_string())))
        }
    }
}

async fn get_event_proof_impl(db: &PgPool, id: Uuid) -> Result<EventWithProof> {
    // Get the event
    let event = get_event_impl(db, id).await?;
    
    // Check if anchored
    let proof = if let Some(batch_id) = event.anchor_batch_id {
        // Get anchor batch info
        let batch = sqlx::query!(
            r#"
            SELECT merkle_root, start_sequence, end_sequence, ethereum_tx_hash, solana_tx_signature
            FROM anchor_batches
            WHERE id = $1
            "#,
            batch_id,
        )
        .fetch_optional(db)
        .await?;
        
        if let Some(batch) = batch {
            // Get all events in this batch to build proof
            let batch_events = sqlx::query!(
                r#"
                SELECT event_hash
                FROM movement_events
                WHERE anchor_batch_id = $1
                ORDER BY sequence_number ASC
                "#,
                batch_id,
            )
            .fetch_all(db)
            .await?;
            
            let hashes: Vec<String> = batch_events.iter().map(|e| e.event_hash.clone()).collect();
            let target_index = hashes.iter().position(|h| h == &event.event_hash).unwrap_or(0);
            let siblings = generate_merkle_proof(&hashes, target_index);
            
            Some(MerkleProof {
                event_hash: event.event_hash.clone(),
                siblings,
                root: batch.merkle_root,
                anchor_batch_id: batch_id,
                ethereum_tx_hash: batch.ethereum_tx_hash,
                solana_tx_signature: batch.solana_tx_signature,
            })
        } else {
            None
        }
    } else {
        None
    };
    
    Ok(EventWithProof { event, proof })
}

async fn verify_chain(
    State(state): State<Arc<AppState>>,
    Query(query): Query<VerifyChainQuery>,
) -> impl IntoResponse {
    match verify_chain_impl(&state.db, query).await {
        Ok(result) => (StatusCode::OK, Json(ApiResponse::success(result))),
        Err(e) => {
            let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(ApiResponse::<ChainVerifyResult>::error(e.error_code(), e.to_string())))
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct VerifyChainQuery {
    pub from_sequence: Option<i64>,
    pub to_sequence: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ChainVerifyResult {
    pub valid: bool,
    pub events_checked: i64,
    pub from_sequence: i64,
    pub to_sequence: i64,
    pub errors: Vec<String>,
}

async fn verify_chain_impl(db: &PgPool, query: VerifyChainQuery) -> Result<ChainVerifyResult> {
    let from = query.from_sequence.unwrap_or(1);
    let to = query.to_sequence.unwrap_or(i64::MAX);
    
    let events = sqlx::query_as!(
        MovementEvent,
        r#"
        SELECT id, sequence_number, event_type as "event_type: EventType", actor_id, policy_decision_id, payload, previous_hash, event_hash, anchor_batch_id, created_at as "created_at!"
        FROM movement_events
        WHERE sequence_number >= $1 AND sequence_number <= $2
        ORDER BY sequence_number ASC
        LIMIT 10000
        "#,
        from,
        to,
    )
    .fetch_all(db)
    .await?;

    let mut errors = Vec::new();
    
    // Verify hash chain
    for i in 1..events.len() {
        let prev = &events[i - 1];
        let curr = &events[i];
        
        if curr.previous_hash != prev.event_hash {
            errors.push(format!(
                "Chain break at sequence {}: previous_hash mismatch",
                curr.sequence_number
            ));
        }
        
        let computed = compute_event_hash(
            curr.sequence_number,
            &curr.event_type,
            &curr.actor_id,
            &curr.payload,
            &curr.previous_hash,
            &curr.created_at,
        );
        
        if computed != curr.event_hash {
            errors.push(format!(
                "Hash mismatch at sequence {}: computed {} != stored {}",
                curr.sequence_number, computed, curr.event_hash
            ));
        }
    }
    
    let actual_to = events.last().map(|e| e.sequence_number).unwrap_or(from);
    
    Ok(ChainVerifyResult {
        valid: errors.is_empty(),
        events_checked: events.len() as i64,
        from_sequence: from,
        to_sequence: actual_to,
        errors,
    })
}

async fn get_stats(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match get_stats_impl(&state.db).await {
        Ok(stats) => (StatusCode::OK, Json(ApiResponse::success(stats))),
        Err(e) => {
            let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(ApiResponse::<LedgerStats>::error(e.error_code(), e.to_string())))
        }
    }
}

async fn get_stats_impl(db: &PgPool) -> Result<LedgerStats> {
    let total: i64 = sqlx::query_scalar!("SELECT COUNT(*) as \"count!\" FROM movement_events")
        .fetch_one(db)
        .await?;
    
    let unanchored: i64 = sqlx::query_scalar!(
        "SELECT COUNT(*) as \"count!\" FROM movement_events WHERE anchor_batch_id IS NULL"
    )
    .fetch_one(db)
    .await?;
    
    let by_type = sqlx::query!(
        r#"
        SELECT event_type::text as "event_type!", COUNT(*) as "count!"
        FROM movement_events
        GROUP BY event_type
        ORDER BY 2 DESC
        "#
    )
    .fetch_all(db)
    .await?;
    
    let last_anchor = sqlx::query_scalar!(
        "SELECT MAX(anchored_at) FROM anchor_batches WHERE status = 'CONFIRMED'"
    )
    .fetch_one(db)
    .await?;
    
    Ok(LedgerStats {
        total_events: total,
        events_by_type: by_type.into_iter().map(|r| EventTypeCount {
            event_type: r.event_type,
            count: r.count,
        }).collect(),
        unanchored_events: unanchored,
        last_anchor_time: last_anchor,
    })
}

async fn export_events(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ExportRequest>,
) -> impl IntoResponse {
    match export_events_impl(&state.db, req).await {
        Ok(export) => (StatusCode::OK, Json(ApiResponse::success(export))),
        Err(e) => {
            let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(ApiResponse::<ExportResponse>::error(e.error_code(), e.to_string())))
        }
    }
}

async fn export_events_impl(db: &PgPool, req: ExportRequest) -> Result<ExportResponse> {
    let events = sqlx::query_as!(
        MovementEvent,
        r#"
        SELECT id, sequence_number, event_type as "event_type: EventType", actor_id, policy_decision_id, payload, previous_hash, event_hash, anchor_batch_id, created_at as "created_at!"
        FROM movement_events
        WHERE ($1::bigint IS NULL OR sequence_number >= $1)
        AND ($2::bigint IS NULL OR sequence_number <= $2)
        AND ($3::timestamptz IS NULL OR created_at >= $3)
        AND ($4::timestamptz IS NULL OR created_at <= $4)
        ORDER BY sequence_number ASC
        LIMIT 10000
        "#,
        req.from_sequence,
        req.to_sequence,
        req.from_date,
        req.to_date,
    )
    .fetch_all(db)
    .await?;
    
    if events.is_empty() {
        return Err(GuardRailError::NotFound("No events found for export".to_string()));
    }
    
    // Build merkle root for export
    let hashes: Vec<String> = events.iter().map(|e| e.event_hash.clone()).collect();
    let merkle_root = build_merkle_root(&hashes);
    
    // Create signature over the export
    let export_id = Uuid::new_v4();
    let now = chrono::Utc::now();
    
    let first_seq = events.first()
        .map(|e| e.sequence_number)
        .ok_or_else(|| GuardRailError::Internal("Events list empty".to_string()))?;
        
    let last_seq = events.last()
        .map(|e| e.sequence_number)
        .ok_or_else(|| GuardRailError::Internal("Events list empty".to_string()))?;

    let signature_data = format!(
        "{}:{}:{}:{}",
        export_id,
        merkle_root,
        first_seq,
        last_seq
    );
    let signature = crypto::sha256_hex(signature_data.as_bytes());
    
    Ok(ExportResponse {
        export_id,
        event_count: events.len() as i64,
        from_sequence: first_seq,
        to_sequence: last_seq,
        merkle_root,
        events,
        signature,
        exported_at: now,
    })
}

// ============================================================================
// Internal Event Recording (for other services)
// ============================================================================

/// Record a policy decision event
pub async fn record_policy_decision(
    state: &AppState,
    actor_id: Uuid,
    decision_id: Uuid,
    payload: serde_json::Value,
) -> Result<MovementEvent> {
    create_event_impl(state, CreateEventRequest {
        event_type: EventType::PolicyDecision,
        actor_id,
        policy_decision_id: Some(decision_id),
        payload,
    }).await
}

/// Record an identity event
pub async fn record_identity_event(
    state: &AppState,
    event_type: EventType,
    actor_id: Uuid,
    payload: serde_json::Value,
) -> Result<MovementEvent> {
    create_event_impl(state, CreateEventRequest {
        event_type,
        actor_id,
        policy_decision_id: None,
        payload,
    }).await
}

// ============================================================================
// Router
// ============================================================================

fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        // Health and stats
        .route("/health", get(health))
        .route("/api/v1/ledger/stats", get(get_stats))
        // Event CRUD
        .route("/api/v1/events", post(create_event))
        .route("/api/v1/events", get(list_events))
        .route("/api/v1/events/:id", get(get_event))
        .route("/api/v1/events/:id/proof", get(get_event_proof))
        // Verification
        .route("/api/v1/ledger/verify", get(verify_chain))
        // Export
        .route("/api/v1/ledger/export", post(export_events))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
}

// ============================================================================
// Main
// ============================================================================

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("signal received, starting graceful shutdown");
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "movement_ledger=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer().json())
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

    // Get last event for hash chain continuity
    let last_event = sqlx::query!(
        r#"
        SELECT sequence_number, event_hash
        FROM movement_events
        ORDER BY sequence_number DESC
        LIMIT 1
        "#
    )
    .fetch_optional(&db)
    .await?
    .map(|row| LastEventInfo {
        sequence_number: row.sequence_number,
        event_hash: row.event_hash,
    });

    tracing::info!("Last event sequence: {:?}", last_event.as_ref().map(|e| e.sequence_number));

    // Create app state
    let state = Arc::new(AppState {
        db,
        last_event: Arc::new(RwLock::new(last_event)),
    });

    // Create router
    let app = create_router(state);

    // Start server
    let port = std::env::var("PORT").unwrap_or_else(|_| "3003".to_string());
    let addr = format!("0.0.0.0:{}", port);
    
    tracing::info!("Movement Ledger listening on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_merkle_root_empty() {
        let hashes = vec![];
        let root = build_merkle_root(&hashes);
        assert_eq!(root, "0".repeat(64));
    }

    #[test]
    fn test_build_merkle_root_single() {
        let hash = "a".repeat(64);
        let hashes = vec![hash.clone()];
        let root = build_merkle_root(&hashes);
        assert_eq!(root, hash);
    }

    #[test]
    fn test_build_merkle_root_pair() {
        let h1 = "a".repeat(64);
        let h2 = "b".repeat(64);
        let hashes = vec![h1.clone(), h2.clone()];
        
        let mut hasher = Sha256::new();
        hasher.update(&h1);
        hasher.update(&h2);
        let expected = hex::encode(hasher.finalize());
        
        let root = build_merkle_root(&hashes);
        assert_eq!(root, expected);
    }

    #[test]
    fn test_build_merkle_root_odd() {
        let h1 = "a".repeat(64);
        let h2 = "b".repeat(64);
        let h3 = "c".repeat(64);
        let hashes = vec![h1.clone(), h2.clone(), h3.clone()];
        
        // Level 1: h1+h2, h3+h3
        let mut hasher = Sha256::new();
        hasher.update(&h1);
        hasher.update(&h2);
        let l1_1 = hex::encode(hasher.finalize());
        
        let mut hasher = Sha256::new();
        hasher.update(&h3);
        hasher.update(&h3);
        let l1_2 = hex::encode(hasher.finalize());
        
        // Root: l1_1 + l1_2
        let mut hasher = Sha256::new();
        hasher.update(&l1_1);
        hasher.update(&l1_2);
        let expected = hex::encode(hasher.finalize());
        
        let root = build_merkle_root(&hashes);
        assert_eq!(root, expected);
    }

    #[test]
    fn test_generate_merkle_proof() {
        let h1 = "a".repeat(64);
        let h2 = "b".repeat(64);
        let h3 = "c".repeat(64);
        let h4 = "d".repeat(64);
        let hashes = vec![h1.clone(), h2.clone(), h3.clone(), h4.clone()];
        
        // Proof for h1 (index 0)
        // Sibling 1: h2 (right)
        // Sibling 2: hash(h3+h4) (right)
        
        let proof = generate_merkle_proof(&hashes, 0);
        assert_eq!(proof.len(), 2);
        assert_eq!(proof[0].hash, h2);
        assert_eq!(proof[0].position, "right");
        
        let mut hasher = Sha256::new();
        hasher.update(&h3);
        hasher.update(&h4);
        let h34 = hex::encode(hasher.finalize());
        
        assert_eq!(proof[1].hash, h34);
        assert_eq!(proof[1].position, "right");
    }

    #[test]
    fn test_verify_hash_chain() {
        use chrono::Utc;
        use guardrail_shared::{EventType, MovementEvent, GENESIS_HASH};
        use uuid::Uuid;
        use serde_json::json;
        
        let now = Utc::now();
        let ts1 = now;
        let ts2 = now + chrono::Duration::seconds(1);
        let actor_id = Uuid::new_v4();
        let payload = json!({});
        
        let h1 = compute_event_hash(1, &EventType::SystemEvent, &actor_id, &payload, GENESIS_HASH, &ts1);
        let e1 = MovementEvent {
            id: Uuid::new_v4(),
            sequence_number: 1,
            event_type: EventType::SystemEvent,
            actor_id,
            policy_decision_id: None,
            payload: payload.clone(),
            previous_hash: GENESIS_HASH.to_string(),
            event_hash: h1.clone(),
            anchor_batch_id: None,
            created_at: ts1,
        };
        
        let h2 = compute_event_hash(2, &EventType::SystemEvent, &actor_id, &payload, &h1, &ts2);
        let e2 = MovementEvent {
            id: Uuid::new_v4(),
            sequence_number: 2,
            event_type: EventType::SystemEvent,
            actor_id,
            policy_decision_id: None,
            payload: payload.clone(),
            previous_hash: h1.clone(),
            event_hash: h2.clone(),
            anchor_batch_id: None,
            created_at: ts2,
        };
        
        let events = vec![e1, e2];
        assert!(verify_hash_chain(&events));
        
        // Test broken chain (wrong previous hash)
        let mut broken_events = events.clone();
        broken_events[1].previous_hash = "broken".to_string();
        assert!(!verify_hash_chain(&broken_events));

        // Test broken chain (tampered payload)
        let mut tampered_events = events.clone();
        tampered_events[1].payload = json!({"tampered": true});
        // The hash won't match the payload anymore
        assert!(!verify_hash_chain(&tampered_events));
    }
}
