//! GuardRail Chain Anchor Service
//!
//! Periodically anchors event batches to Ethereum L2 and Solana blockchains
//! using Merkle tree commitments.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use ethers::{
    prelude::*,
    providers::{Http, Provider},
    types::{Address, H256, U256},
};
use guardrail_shared::{AnchorBatch, AnchorStatus, ApiResponse, GuardRailError, PaginatedResponse, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
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
    pub config: Arc<AnchorConfig>,
    pub ethereum: Arc<RwLock<Option<EthereumAnchor>>>,
    pub solana: Arc<RwLock<Option<SolanaAnchor>>>,
}

#[derive(Clone, Debug)]
pub struct AnchorConfig {
    pub batch_size: usize,
    pub anchor_interval_secs: u64,
    pub ethereum_enabled: bool,
    pub solana_enabled: bool,
    pub ethereum_rpc_url: Option<String>,
    pub ethereum_contract_address: Option<String>,
    pub solana_rpc_url: Option<String>,
    pub solana_program_id: Option<String>,
}

impl Default for AnchorConfig {
    fn default() -> Self {
        Self {
            batch_size: 1000,
            anchor_interval_secs: 3600, // 1 hour
            ethereum_enabled: false,
            solana_enabled: false,
            ethereum_rpc_url: None,
            ethereum_contract_address: None,
            solana_rpc_url: None,
            solana_program_id: None,
        }
    }
}

pub struct EthereumAnchor {
    pub provider: Provider<Http>,
    pub contract_address: Address,
    pub wallet: LocalWallet,
}

pub struct SolanaAnchor {
    pub client: RpcClient,
    pub program_id: Pubkey,
    pub payer: Keypair,
}

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct ListBatchesQuery {
    pub page: Option<i32>,
    pub per_page: Option<i32>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub version: String,
    pub ethereum_connected: bool,
    pub solana_connected: bool,
    pub pending_batches: i64,
}

#[derive(Debug, Serialize)]
pub struct AnchorStats {
    pub total_batches: i64,
    pub confirmed_batches: i64,
    pub pending_batches: i64,
    pub failed_batches: i64,
    pub total_events_anchored: i64,
    pub last_anchor_time: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Serialize)]
pub struct BatchDetail {
    pub batch: AnchorBatch,
    pub event_hashes: Vec<String>,
    pub verification_status: VerificationStatus,
}

#[derive(Debug, Serialize)]
pub struct VerificationStatus {
    pub ethereum_verified: Option<bool>,
    pub solana_verified: Option<bool>,
    pub merkle_root_matches: bool,
}

#[derive(Debug, Deserialize)]
pub struct ManualAnchorRequest {
    pub max_events: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct AnchorResult {
    pub batch_id: Uuid,
    pub merkle_root: String,
    pub event_count: i32,
    pub ethereum_tx_hash: Option<String>,
    pub solana_tx_signature: Option<String>,
    pub status: AnchorStatus,
}

// ============================================================================
// Merkle Tree Implementation
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
    while current_level.len().count_ones() != 1 {
        current_level.push(current_level.last().unwrap().clone());
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

// ============================================================================
// Blockchain Anchoring
// ============================================================================

/// Anchor to Ethereum L2 (Base/Arbitrum)
async fn anchor_to_ethereum(
    ethereum: &EthereumAnchor,
    merkle_root: &str,
    batch_id: &Uuid,
    event_count: u32,
) -> Result<(String, i64)> {
    // ABI for storeBatch(bytes32 merkleRoot, bytes32 batchId, uint32 eventCount)
    abigen!(
        GuardRailAnchor,
        r#"[
            function storeBatch(bytes32 merkleRoot, bytes32 batchId, uint32 eventCount) external
            function getBatch(bytes32 batchId) external view returns (bytes32 merkleRoot, uint32 eventCount, uint256 timestamp)
        ]"#
    );
    
    let client = SignerMiddleware::new(
        ethereum.provider.clone(),
        ethereum.wallet.clone(),
    );
    let client = Arc::new(client);
    
    let contract = GuardRailAnchor::new(ethereum.contract_address, client);
    
    // Convert merkle root to bytes32
    let root_bytes: [u8; 32] = hex::decode(merkle_root)
        .map_err(|e| GuardRailError::ChainAnchor(format!("Invalid merkle root: {}", e)))?
        .try_into()
        .map_err(|_| GuardRailError::ChainAnchor("Merkle root must be 32 bytes".to_string()))?;
    
    // Convert batch ID to bytes32
    let batch_bytes: [u8; 32] = {
        let mut bytes = [0u8; 32];
        bytes[..16].copy_from_slice(batch_id.as_bytes());
        bytes
    };
    
    // Send transaction
    let tx = contract
        .store_batch(root_bytes, batch_bytes, event_count)
        .send()
        .await
        .map_err(|e| GuardRailError::ChainAnchor(format!("Failed to send tx: {}", e)))?
        .await
        .map_err(|e| GuardRailError::ChainAnchor(format!("Failed to confirm tx: {}", e)))?
        .ok_or_else(|| GuardRailError::ChainAnchor("Transaction not found".to_string()))?;
    
    let tx_hash = format!("{:?}", tx.transaction_hash);
    let block_number = tx.block_number.map(|b| b.as_u64() as i64).unwrap_or(0);
    
    Ok((tx_hash, block_number))
}

/// Anchor to Solana
async fn anchor_to_solana(
    solana: &SolanaAnchor,
    merkle_root: &str,
    batch_id: &Uuid,
    event_count: u32,
) -> Result<(String, i64)> {
    // Build instruction data
    // Format: [discriminator(8)] [merkle_root(32)] [batch_id(16)] [event_count(4)]
    let mut data = Vec::with_capacity(60);
    
    // Discriminator for "store_batch" (first 8 bytes of SHA256("global:store_batch"))
    let discriminator = {
        let mut hasher = Sha256::new();
        hasher.update(b"global:store_batch");
        let hash = hasher.finalize();
        hash[..8].to_vec()
    };
    data.extend_from_slice(&discriminator);
    
    // Merkle root
    let root_bytes = hex::decode(merkle_root)
        .map_err(|e| GuardRailError::ChainAnchor(format!("Invalid merkle root: {}", e)))?;
    data.extend_from_slice(&root_bytes);
    
    // Batch ID
    data.extend_from_slice(batch_id.as_bytes());
    
    // Event count
    data.extend_from_slice(&event_count.to_le_bytes());
    
    // Create instruction
    let instruction = Instruction {
        program_id: solana.program_id,
        accounts: vec![
            AccountMeta::new(solana.payer.pubkey(), true), // payer/signer
        ],
        data,
    };
    
    // Build and send transaction
    let recent_blockhash = solana.client
        .get_latest_blockhash()
        .map_err(|e| GuardRailError::ChainAnchor(format!("Failed to get blockhash: {}", e)))?;
    
    let message = Message::new(&[instruction], Some(&solana.payer.pubkey()));
    let transaction = Transaction::new(&[&solana.payer], message, recent_blockhash);
    
    let signature = solana.client
        .send_and_confirm_transaction_with_spinner(&transaction)
        .map_err(|e| GuardRailError::ChainAnchor(format!("Failed to send tx: {}", e)))?;
    
    let slot = solana.client
        .get_slot()
        .map_err(|e| GuardRailError::ChainAnchor(format!("Failed to get slot: {}", e)))?;
    
    Ok((signature.to_string(), slot as i64))
}

// ============================================================================
// Batch Creation and Anchoring
// ============================================================================

async fn create_and_anchor_batch(state: &AppState) -> Result<Option<AnchorResult>> {
    // Get unanchored events
    let events = sqlx::query!(
        r#"
        SELECT id, sequence_number, event_hash
        FROM movement_events
        WHERE anchor_batch_id IS NULL
        ORDER BY sequence_number ASC
        LIMIT $1
        "#,
        state.config.batch_size as i64,
    )
    .fetch_all(&state.db)
    .await?;
    
    if events.is_empty() {
        return Ok(None);
    }
    
    let event_hashes: Vec<String> = events.iter().map(|e| e.event_hash.clone()).collect();
    let event_ids: Vec<Uuid> = events.iter().map(|e| e.id).collect();
    let start_sequence = events.first().unwrap().sequence_number;
    let end_sequence = events.last().unwrap().sequence_number;
    let event_count = events.len() as i32;
    
    // Build Merkle root
    let merkle_root = build_merkle_root(&event_hashes);
    
    // Create batch record
    let batch_id = Uuid::new_v4();
    let now = chrono::Utc::now();
    
    sqlx::query!(
        r#"
        INSERT INTO anchor_batches (id, merkle_root, start_sequence, end_sequence, event_count, status, created_at)
        VALUES ($1, $2, $3, $4, $5, 'PENDING'::anchor_status, $6)
        "#,
        batch_id,
        merkle_root,
        start_sequence,
        end_sequence,
        event_count,
        now,
    )
    .execute(&state.db)
    .await?;
    
    // Update status to anchoring
    sqlx::query!(
        "UPDATE anchor_batches SET status = 'ANCHORING'::anchor_status WHERE id = $1",
        batch_id,
    )
    .execute(&state.db)
    .await?;
    
    let mut ethereum_tx_hash: Option<String> = None;
    let mut ethereum_block: Option<i64> = None;
    let mut solana_tx_signature: Option<String> = None;
    let mut solana_slot: Option<i64> = None;
    let mut failed = false;
    
    // Anchor to Ethereum
    if state.config.ethereum_enabled {
        let eth = state.ethereum.read().await;
        if let Some(ethereum) = eth.as_ref() {
            match anchor_to_ethereum(ethereum, &merkle_root, &batch_id, event_count as u32).await {
                Ok((tx_hash, block)) => {
                    ethereum_tx_hash = Some(tx_hash);
                    ethereum_block = Some(block);
                    tracing::info!("Anchored batch {} to Ethereum: {}", batch_id, ethereum_tx_hash.as_ref().unwrap());
                }
                Err(e) => {
                    tracing::error!("Failed to anchor to Ethereum: {}", e);
                    failed = true;
                }
            }
        }
    }
    
    // Anchor to Solana
    if state.config.solana_enabled && !failed {
        let sol = state.solana.read().await;
        if let Some(solana) = sol.as_ref() {
            match anchor_to_solana(solana, &merkle_root, &batch_id, event_count as u32).await {
                Ok((sig, slot)) => {
                    solana_tx_signature = Some(sig);
                    solana_slot = Some(slot);
                    tracing::info!("Anchored batch {} to Solana: {}", batch_id, solana_tx_signature.as_ref().unwrap());
                }
                Err(e) => {
                    tracing::error!("Failed to anchor to Solana: {}", e);
                    failed = true;
                }
            }
        }
    }
    
    let status = if failed {
        AnchorStatus::Failed
    } else {
        AnchorStatus::Confirmed
    };
    
    // Update batch with results
    let anchored_at = if !failed { Some(chrono::Utc::now()) } else { None };
    
    sqlx::query!(
        r#"
        UPDATE anchor_batches
        SET status = $2::anchor_status,
            ethereum_tx_hash = $3,
            ethereum_block = $4,
            solana_tx_signature = $5,
            solana_slot = $6,
            anchored_at = $7
        WHERE id = $1
        "#,
        batch_id,
        status.to_string(),
        ethereum_tx_hash,
        ethereum_block,
        solana_tx_signature,
        solana_slot,
        anchored_at,
    )
    .execute(&state.db)
    .await?;
    
    // Update events with batch ID (only if successful)
    if !failed {
        for event_id in &event_ids {
            sqlx::query!(
                "UPDATE movement_events SET anchor_batch_id = $1 WHERE id = $2",
                batch_id,
                event_id,
            )
            .execute(&state.db)
            .await?;
        }
    }
    
    Ok(Some(AnchorResult {
        batch_id,
        merkle_root,
        event_count,
        ethereum_tx_hash,
        solana_tx_signature,
        status,
    }))
}

// ============================================================================
// Handlers
// ============================================================================

async fn health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let eth_connected = state.ethereum.read().await.is_some();
    let sol_connected = state.solana.read().await.is_some();
    
    let pending: i64 = sqlx::query_scalar!(
        "SELECT COUNT(*) as \"count!\" FROM anchor_batches WHERE status = 'PENDING'"
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);
    
    Json(HealthResponse {
        status: "healthy".to_string(),
        service: "chain-anchor".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        ethereum_connected: eth_connected,
        solana_connected: sol_connected,
        pending_batches: pending,
    })
}

async fn get_stats(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match get_stats_impl(&state.db).await {
        Ok(stats) => (StatusCode::OK, Json(ApiResponse::success(stats))),
        Err(e) => {
            let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(ApiResponse::<AnchorStats>::error(e.error_code(), e.to_string())))
        }
    }
}

async fn get_stats_impl(db: &PgPool) -> Result<AnchorStats> {
    let total: i64 = sqlx::query_scalar!("SELECT COUNT(*) as \"count!\" FROM anchor_batches")
        .fetch_one(db)
        .await?;
    
    let confirmed: i64 = sqlx::query_scalar!(
        "SELECT COUNT(*) as \"count!\" FROM anchor_batches WHERE status = 'CONFIRMED'"
    )
    .fetch_one(db)
    .await?;
    
    let pending: i64 = sqlx::query_scalar!(
        "SELECT COUNT(*) as \"count!\" FROM anchor_batches WHERE status IN ('PENDING', 'ANCHORING')"
    )
    .fetch_one(db)
    .await?;
    
    let failed: i64 = sqlx::query_scalar!(
        "SELECT COUNT(*) as \"count!\" FROM anchor_batches WHERE status = 'FAILED'"
    )
    .fetch_one(db)
    .await?;
    
    let total_events: i64 = sqlx::query_scalar!(
        "SELECT COALESCE(SUM(event_count), 0) as \"sum!\" FROM anchor_batches WHERE status = 'CONFIRMED'"
    )
    .fetch_one(db)
    .await?;
    
    let last_anchor = sqlx::query_scalar!(
        "SELECT MAX(anchored_at) FROM anchor_batches WHERE status = 'CONFIRMED'"
    )
    .fetch_one(db)
    .await?;
    
    Ok(AnchorStats {
        total_batches: total,
        confirmed_batches: confirmed,
        pending_batches: pending,
        failed_batches: failed,
        total_events_anchored: total_events,
        last_anchor_time: last_anchor,
    })
}

async fn list_batches(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListBatchesQuery>,
) -> impl IntoResponse {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).min(100);
    let offset = (page - 1) * per_page;

    match list_batches_impl(&state.db, offset, per_page, query.status).await {
        Ok((batches, total)) => {
            let response = PaginatedResponse::new(batches, total, page, per_page);
            (StatusCode::OK, Json(ApiResponse::success(response)))
        }
        Err(e) => {
            let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(ApiResponse::<PaginatedResponse<AnchorBatch>>::error(e.error_code(), e.to_string())))
        }
    }
}

async fn list_batches_impl(
    db: &PgPool,
    offset: i32,
    limit: i32,
    status_filter: Option<String>,
) -> Result<(Vec<AnchorBatch>, i64)> {
    let batches = sqlx::query_as!(
        AnchorBatch,
        r#"
        SELECT id, merkle_root, start_sequence, end_sequence, event_count, 
               ethereum_tx_hash, ethereum_block, solana_tx_signature, solana_slot,
               status as "status: _", created_at, anchored_at
        FROM anchor_batches
        WHERE ($3::text IS NULL OR status::text = $3)
        ORDER BY created_at DESC
        LIMIT $1 OFFSET $2
        "#,
        limit as i64,
        offset as i64,
        status_filter,
    )
    .fetch_all(db)
    .await?;

    let total: i64 = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) as "count!"
        FROM anchor_batches
        WHERE ($1::text IS NULL OR status::text = $1)
        "#,
        status_filter,
    )
    .fetch_one(db)
    .await?;

    Ok((batches, total))
}

async fn get_batch(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match get_batch_impl(&state.db, id).await {
        Ok(detail) => (StatusCode::OK, Json(ApiResponse::success(detail))),
        Err(e) => {
            let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(ApiResponse::<BatchDetail>::error(e.error_code(), e.to_string())))
        }
    }
}

async fn get_batch_impl(db: &PgPool, id: Uuid) -> Result<BatchDetail> {
    let batch = sqlx::query_as!(
        AnchorBatch,
        r#"
        SELECT id, merkle_root, start_sequence, end_sequence, event_count,
               ethereum_tx_hash, ethereum_block, solana_tx_signature, solana_slot,
               status as "status: _", created_at, anchored_at
        FROM anchor_batches
        WHERE id = $1
        "#,
        id,
    )
    .fetch_optional(db)
    .await?
    .ok_or_else(|| GuardRailError::NotFound(format!("Batch {} not found", id)))?;
    
    // Get event hashes for this batch
    let events = sqlx::query!(
        "SELECT event_hash FROM movement_events WHERE anchor_batch_id = $1 ORDER BY sequence_number",
        id,
    )
    .fetch_all(db)
    .await?;
    
    let event_hashes: Vec<String> = events.iter().map(|e| e.event_hash.clone()).collect();
    
    // Verify merkle root matches
    let computed_root = build_merkle_root(&event_hashes);
    let merkle_root_matches = computed_root == batch.merkle_root;
    
    Ok(BatchDetail {
        batch,
        event_hashes,
        verification_status: VerificationStatus {
            ethereum_verified: None, // Would need to query on-chain
            solana_verified: None,   // Would need to query on-chain
            merkle_root_matches,
        },
    })
}

async fn trigger_anchor(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ManualAnchorRequest>,
) -> impl IntoResponse {
    match create_and_anchor_batch(&state).await {
        Ok(Some(result)) => (StatusCode::OK, Json(ApiResponse::success(result))),
        Ok(None) => (
            StatusCode::OK,
            Json(ApiResponse::<AnchorResult>::error("NO_EVENTS", "No unanchored events to process")),
        ),
        Err(e) => {
            let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(ApiResponse::<AnchorResult>::error(e.error_code(), e.to_string())))
        }
    }
}

async fn retry_batch(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match retry_batch_impl(&state, id).await {
        Ok(result) => (StatusCode::OK, Json(ApiResponse::success(result))),
        Err(e) => {
            let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(ApiResponse::<AnchorResult>::error(e.error_code(), e.to_string())))
        }
    }
}

async fn retry_batch_impl(state: &AppState, id: Uuid) -> Result<AnchorResult> {
    // Get the failed batch
    let batch = sqlx::query!(
        r#"
        SELECT id, merkle_root, event_count, status::text as "status!"
        FROM anchor_batches
        WHERE id = $1
        "#,
        id,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| GuardRailError::NotFound(format!("Batch {} not found", id)))?;
    
    if batch.status != "FAILED" {
        return Err(GuardRailError::InvalidInput("Can only retry failed batches".to_string()));
    }
    
    // Update status to anchoring
    sqlx::query!(
        "UPDATE anchor_batches SET status = 'ANCHORING'::anchor_status WHERE id = $1",
        id,
    )
    .execute(&state.db)
    .await?;
    
    let mut ethereum_tx_hash: Option<String> = None;
    let mut ethereum_block: Option<i64> = None;
    let mut solana_tx_signature: Option<String> = None;
    let mut solana_slot: Option<i64> = None;
    let mut failed = false;
    
    // Retry Ethereum
    if state.config.ethereum_enabled {
        let eth = state.ethereum.read().await;
        if let Some(ethereum) = eth.as_ref() {
            match anchor_to_ethereum(ethereum, &batch.merkle_root, &id, batch.event_count as u32).await {
                Ok((tx_hash, block)) => {
                    ethereum_tx_hash = Some(tx_hash);
                    ethereum_block = Some(block);
                }
                Err(e) => {
                    tracing::error!("Retry failed for Ethereum: {}", e);
                    failed = true;
                }
            }
        }
    }
    
    // Retry Solana
    if state.config.solana_enabled && !failed {
        let sol = state.solana.read().await;
        if let Some(solana) = sol.as_ref() {
            match anchor_to_solana(solana, &batch.merkle_root, &id, batch.event_count as u32).await {
                Ok((sig, slot)) => {
                    solana_tx_signature = Some(sig);
                    solana_slot = Some(slot);
                }
                Err(e) => {
                    tracing::error!("Retry failed for Solana: {}", e);
                    failed = true;
                }
            }
        }
    }
    
    let status = if failed { AnchorStatus::Failed } else { AnchorStatus::Confirmed };
    let anchored_at = if !failed { Some(chrono::Utc::now()) } else { None };
    
    sqlx::query!(
        r#"
        UPDATE anchor_batches
        SET status = $2::anchor_status,
            ethereum_tx_hash = COALESCE($3, ethereum_tx_hash),
            ethereum_block = COALESCE($4, ethereum_block),
            solana_tx_signature = COALESCE($5, solana_tx_signature),
            solana_slot = COALESCE($6, solana_slot),
            anchored_at = COALESCE($7, anchored_at)
        WHERE id = $1
        "#,
        id,
        status.to_string(),
        ethereum_tx_hash,
        ethereum_block,
        solana_tx_signature,
        solana_slot,
        anchored_at,
    )
    .execute(&state.db)
    .await?;
    
    Ok(AnchorResult {
        batch_id: id,
        merkle_root: batch.merkle_root,
        event_count: batch.event_count,
        ethereum_tx_hash,
        solana_tx_signature,
        status,
    })
}

// ============================================================================
// Background Scheduler
// ============================================================================

async fn run_scheduler(state: Arc<AppState>) {
    let interval_secs = state.config.anchor_interval_secs;
    
    loop {
        tokio::time::sleep(Duration::from_secs(interval_secs)).await;
        
        tracing::info!("Running scheduled anchor job");
        
        match create_and_anchor_batch(&state).await {
            Ok(Some(result)) => {
                tracing::info!(
                    "Anchored batch {} with {} events (status: {:?})",
                    result.batch_id,
                    result.event_count,
                    result.status
                );
            }
            Ok(None) => {
                tracing::debug!("No events to anchor");
            }
            Err(e) => {
                tracing::error!("Scheduled anchor failed: {}", e);
            }
        }
    }
}

// ============================================================================
// Router
// ============================================================================

fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        // Health and stats
        .route("/health", get(health))
        .route("/api/v1/anchors/stats", get(get_stats))
        // Batch management
        .route("/api/v1/anchors", get(list_batches))
        .route("/api/v1/anchors/:id", get(get_batch))
        // Manual operations
        .route("/api/v1/anchors/trigger", post(trigger_anchor))
        .route("/api/v1/anchors/:id/retry", post(retry_batch))
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
                .unwrap_or_else(|_| "chain_anchor=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    // Load environment variables
    dotenvy::dotenv().ok();

    // Database connection
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");

    let db = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    tracing::info!("Connected to database");

    // Load config
    let config = AnchorConfig {
        batch_size: std::env::var("ANCHOR_BATCH_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1000),
        anchor_interval_secs: std::env::var("ANCHOR_INTERVAL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(3600),
        ethereum_enabled: std::env::var("ETHEREUM_ENABLED")
            .map(|s| s == "true")
            .unwrap_or(false),
        solana_enabled: std::env::var("SOLANA_ENABLED")
            .map(|s| s == "true")
            .unwrap_or(false),
        ethereum_rpc_url: std::env::var("ETHEREUM_RPC_URL").ok(),
        ethereum_contract_address: std::env::var("ETHEREUM_CONTRACT_ADDRESS").ok(),
        solana_rpc_url: std::env::var("SOLANA_RPC_URL").ok(),
        solana_program_id: std::env::var("SOLANA_PROGRAM_ID").ok(),
    };

    // Initialize Ethereum (if enabled)
    let ethereum = if config.ethereum_enabled {
        if let (Some(rpc_url), Some(contract_addr), Ok(private_key)) = (
            &config.ethereum_rpc_url,
            &config.ethereum_contract_address,
            std::env::var("ETHEREUM_PRIVATE_KEY"),
        ) {
            let provider = Provider::<Http>::try_from(rpc_url.as_str())?;
            let wallet: LocalWallet = private_key.parse()?;
            let contract_address: Address = contract_addr.parse()?;
            
            tracing::info!("Ethereum anchor enabled: {}", contract_addr);
            
            Some(EthereumAnchor {
                provider,
                contract_address,
                wallet,
            })
        } else {
            tracing::warn!("Ethereum enabled but missing configuration");
            None
        }
    } else {
        None
    };

    // Initialize Solana (if enabled)
    let solana = if config.solana_enabled {
        if let (Some(rpc_url), Some(program_id_str), Ok(private_key)) = (
            &config.solana_rpc_url,
            &config.solana_program_id,
            std::env::var("SOLANA_PRIVATE_KEY"),
        ) {
            let client = RpcClient::new_with_commitment(rpc_url.clone(), CommitmentConfig::confirmed());
            let program_id: Pubkey = program_id_str.parse()
                .map_err(|e| anyhow::anyhow!("Invalid Solana program ID: {}", e))?;
            
            // Parse private key (base58 encoded)
            let payer = Keypair::from_base58_string(&private_key);
            
            tracing::info!("Solana anchor enabled: {}", program_id);
            
            Some(SolanaAnchor {
                client,
                program_id,
                payer,
            })
        } else {
            tracing::warn!("Solana enabled but missing configuration");
            None
        }
    } else {
        None
    };

    // Create app state
    let state = Arc::new(AppState {
        db,
        config: Arc::new(config),
        ethereum: Arc::new(RwLock::new(ethereum)),
        solana: Arc::new(RwLock::new(solana)),
    });

    // Start background scheduler
    let scheduler_state = state.clone();
    tokio::spawn(async move {
        run_scheduler(scheduler_state).await;
    });

    // Create router
    let app = create_router(state);

    // Start server
    let port = std::env::var("PORT").unwrap_or_else(|_| "3004".to_string());
    let addr = format!("0.0.0.0:{}", port);
    
    tracing::info!("Chain Anchor listening on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}
