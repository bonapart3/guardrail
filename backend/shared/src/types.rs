//! Core domain types for GuardRail

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use utoipa::ToSchema;

// ============================================================================
// Identity Types
// ============================================================================

/// Type of identity in the system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "identity_type", rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IdentityType {
    Human,
    Agent,
    Organization,
}

/// An identity represents a user, agent, or organization in the system
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct Identity {
    pub id: Uuid,
    pub identity_type: IdentityType,
    pub external_id: Option<String>,
    pub display_name: String,
    pub metadata: serde_json::Value,
    pub organization_id: Option<Uuid>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request to create a new identity
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateIdentityRequest {
    pub identity_type: IdentityType,
    pub external_id: Option<String>,
    pub display_name: String,
    pub metadata: Option<serde_json::Value>,
    pub organization_id: Option<Uuid>,
}

/// A cryptographic key or wallet address bound to an identity
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct IdentityKey {
    pub id: Uuid,
    pub identity_id: Uuid,
    pub key_type: KeyType,
    pub public_key: String,
    pub chain: Option<String>,
    pub label: Option<String>,
    pub is_primary: bool,
    pub verified_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "key_type", rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum KeyType {
    WalletAddress,
    SigningKey,
    ApiKey,
    DeviceId,
}

/// A credential attached to an identity (KYC status, risk score, etc.)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Credential {
    pub id: Uuid,
    pub identity_id: Uuid,
    pub credential_type: CredentialType,
    pub provider: String,
    pub value: serde_json::Value,
    pub expires_at: Option<DateTime<Utc>>,
    pub verified_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "credential_type", rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CredentialType {
    KycLevel,
    RiskScore,
    Jurisdiction,
    SanctionsStatus,
    AccreditedInvestor,
    Custom,
}

// ============================================================================
// Policy Types
// ============================================================================

/// A policy definition with Rego source
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct Policy {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub version: String,
    pub rego_source: String,
    pub is_active: bool,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request to create or update a policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePolicyRequest {
    pub name: String,
    pub description: Option<String>,
    pub rego_source: String,
}

/// An action to be checked against policies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub action_type: ActionType,
    pub amount: Option<String>,
    pub asset: Option<String>,
    pub source_address: Option<String>,
    pub target_address: Option<String>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ActionType {
    Withdrawal,
    Deposit,
    Transfer,
    Swap,
    Trade,
    ApiCall,
    ConfigChange,
    Custom,
}

/// Context for policy evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionContext {
    pub ip_address: Option<String>,
    pub device_id: Option<String>,
    pub user_agent: Option<String>,
    pub geo_location: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub session_id: Option<String>,
    pub metadata: serde_json::Value,
}

/// Request to check an action against policies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckActionRequest {
    pub identity_id: Uuid,
    pub action: Action,
    pub context: ActionContext,
}

/// Result of a policy check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyDecision {
    pub decision_id: Uuid,
    pub decision: Decision,
    pub reasons: Vec<String>,
    pub required_approvers: Vec<String>,
    pub policy_id: Uuid,
    pub policy_version: String,
    pub evaluated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "decision", rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Decision {
    Allow,
    Deny,
    RequireApproval,
}

// ============================================================================
// Movement / Event Types
// ============================================================================

/// An immutable event in the movement ledger
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct MovementEvent {
    pub id: Uuid,
    pub sequence_number: i64,
    pub event_type: EventType,
    pub actor_id: Uuid,
    pub policy_decision_id: Option<Uuid>,
    pub payload: serde_json::Value,
    pub previous_hash: String,
    pub event_hash: String,
    pub anchor_batch_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "event_type", rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EventType {
    PolicyDecision,
    IdentityCreated,
    IdentityUpdated,
    KeyAttached,
    KeyDetached,
    CredentialAdded,
    CredentialUpdated,
    ApprovalRequested,
    ApprovalGranted,
    ApprovalRejected,
    PolicyCreated,
    PolicyUpdated,
    AnchorBatchCreated,
    SystemEvent,
}

// ============================================================================
// Approval Types
// ============================================================================

/// A pending approval request
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Approval {
    pub id: Uuid,
    pub decision_id: Uuid,
    pub identity_id: Uuid,
    pub action: serde_json::Value,
    pub required_role: String,
    pub status: ApprovalStatus,
    pub approved_by: Option<Uuid>,
    pub approved_at: Option<DateTime<Utc>>,
    pub rejection_reason: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "approval_status", rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Rejected,
    Expired,
}

// ============================================================================
// Anchor Types
// ============================================================================

/// A batch of events anchored to blockchain(s)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AnchorBatch {
    pub id: Uuid,
    pub merkle_root: String,
    pub start_sequence: i64,
    pub end_sequence: i64,
    pub event_count: i32,
    pub ethereum_tx_hash: Option<String>,
    pub ethereum_block: Option<i64>,
    pub solana_tx_signature: Option<String>,
    pub solana_slot: Option<i64>,
    pub status: AnchorStatus,
    pub created_at: DateTime<Utc>,
    pub anchored_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "anchor_status", rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AnchorStatus {
    Pending,
    Anchoring,
    Confirmed,
    Failed,
}

// ============================================================================
// API Response Types
// ============================================================================

/// Standard API response wrapper
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<ApiError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(ApiError {
                code: code.into(),
                message: message.into(),
                details: None,
            }),
        }
    }
}

/// Paginated response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub total: i64,
    pub page: i32,
    pub per_page: i32,
    pub total_pages: i32,
}

impl<T> PaginatedResponse<T> {
    pub fn new(items: Vec<T>, total: i64, page: i32, per_page: i32) -> Self {
        let total_pages = ((total as f64) / (per_page as f64)).ceil() as i32;
        Self {
            items,
            total,
            page,
            per_page,
            total_pages,
        }
    }
}
