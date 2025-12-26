//! Error types for GuardRail

use thiserror::Error;

/// Main error type for GuardRail services
#[derive(Error, Debug)]
pub enum GuardRailError {
    // Database errors
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    // Serialization errors
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    // Authentication/Authorization errors
    #[error("Authentication failed: {0}")]
    Authentication(String),

    #[error("Authorization denied: {0}")]
    Authorization(String),

    #[error("Invalid token: {0}")]
    InvalidToken(String),

    #[error("Token expired")]
    TokenExpired,

    // Identity errors
    #[error("Identity not found: {0}")]
    IdentityNotFound(String),

    #[error("Identity already exists: {0}")]
    IdentityAlreadyExists(String),

    #[error("Key already bound to identity: {0}")]
    KeyAlreadyBound(String),

    // Policy errors
    #[error("Policy not found: {0}")]
    PolicyNotFound(String),

    #[error("Policy evaluation failed: {0}")]
    PolicyEvaluation(String),

    #[error("Invalid Rego syntax: {0}")]
    InvalidRego(String),

    // Event errors
    #[error("Event not found: {0}")]
    EventNotFound(String),

    #[error("Hash chain integrity violation at sequence {0}")]
    HashChainViolation(i64),

    // Approval errors
    #[error("Approval not found: {0}")]
    ApprovalNotFound(String),

    #[error("Approval already processed")]
    ApprovalAlreadyProcessed,

    #[error("Approval expired")]
    ApprovalExpired,

    // Anchor errors
    #[error("Anchor batch not found: {0}")]
    AnchorNotFound(String),

    #[error("Blockchain transaction failed: {0}")]
    BlockchainTransaction(String),

    #[error("Chain anchor error: {0}")]
    ChainAnchor(String),

    // Validation errors
    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Invalid input: {field} - {message}")]
    InvalidField { field: String, message: String },

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    // Crypto/ZK errors
    #[error("Cryptographic error: {0}")]
    CryptoError(String),

    // Auth errors
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    // Generic errors
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    // External service errors
    #[error("KYC provider error: {0}")]
    KycProvider(String),

    #[error("External service unavailable: {0}")]
    ExternalService(String),

    // Generic errors
    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Not implemented: {0}")]
    NotImplemented(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Resource conflict: {0}")]
    Conflict(String),
}

impl GuardRailError {
    /// Get the HTTP status code for this error
    pub fn status_code(&self) -> u16 {
        match self {
            Self::Authentication(_) | Self::InvalidToken(_) | Self::TokenExpired | Self::Unauthorized(_) => 401,
            Self::Authorization(_) => 403,
            Self::IdentityNotFound(_)
            | Self::PolicyNotFound(_)
            | Self::EventNotFound(_)
            | Self::ApprovalNotFound(_)
            | Self::AnchorNotFound(_)
            | Self::NotFound(_) => 404,
            Self::Validation(_) | Self::InvalidField { .. } | Self::InvalidInput(_) | Self::InvalidRego(_) | Self::CryptoError(_) => 400,
            Self::IdentityAlreadyExists(_)
            | Self::KeyAlreadyBound(_)
            | Self::ApprovalAlreadyProcessed
            | Self::Conflict(_) => 409,
            Self::ApprovalExpired => 410,
            Self::RateLimitExceeded => 429,
            Self::ExternalService(_) | Self::KycProvider(_) | Self::ServiceUnavailable(_) => 502,
            Self::NotImplemented(_) => 501,
            _ => 500,
        }
    }

    /// Get the error code for API responses
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::Database(_) => "DATABASE_ERROR",
            Self::Authentication(_) => "AUTHENTICATION_FAILED",
            Self::Authorization(_) => "AUTHORIZATION_DENIED",
            Self::InvalidToken(_) => "INVALID_TOKEN",
            Self::TokenExpired => "TOKEN_EXPIRED",
            Self::Unauthorized(_) => "UNAUTHORIZED",
            Self::IdentityNotFound(_) => "IDENTITY_NOT_FOUND",
            Self::IdentityAlreadyExists(_) => "IDENTITY_ALREADY_EXISTS",
            Self::KeyAlreadyBound(_) => "KEY_ALREADY_BOUND",
            Self::PolicyNotFound(_) => "POLICY_NOT_FOUND",
            Self::PolicyEvaluation(_) => "POLICY_EVALUATION_FAILED",
            Self::InvalidRego(_) => "INVALID_REGO",
            Self::EventNotFound(_) => "EVENT_NOT_FOUND",
            Self::HashChainViolation(_) => "HASH_CHAIN_VIOLATION",
            Self::ApprovalNotFound(_) => "APPROVAL_NOT_FOUND",
            Self::ApprovalAlreadyProcessed => "APPROVAL_ALREADY_PROCESSED",
            Self::ApprovalExpired => "APPROVAL_EXPIRED",
            Self::AnchorNotFound(_) => "ANCHOR_NOT_FOUND",
            Self::BlockchainTransaction(_) => "BLOCKCHAIN_TX_FAILED",
            Self::ChainAnchor(_) => "CHAIN_ANCHOR_ERROR",
            Self::Validation(_) => "VALIDATION_ERROR",
            Self::InvalidField { .. } => "INVALID_FIELD",
            Self::InvalidInput(_) => "INVALID_INPUT",
            Self::CryptoError(_) => "CRYPTO_ERROR",
            Self::KycProvider(_) => "KYC_PROVIDER_ERROR",
            Self::ExternalService(_) => "EXTERNAL_SERVICE_ERROR",
            Self::ServiceUnavailable(_) => "SERVICE_UNAVAILABLE",
            Self::NotFound(_) => "NOT_FOUND",
            Self::Internal(_) => "INTERNAL_ERROR",
            Self::NotImplemented(_) => "NOT_IMPLEMENTED",
            Self::RateLimitExceeded => "RATE_LIMIT_EXCEEDED",
            Self::Conflict(_) => "RESOURCE_CONFLICT",
            Self::Json(_) => "JSON_ERROR",
        }
    }
}

/// Result type alias for GuardRail operations
pub type Result<T> = std::result::Result<T, GuardRailError>;
