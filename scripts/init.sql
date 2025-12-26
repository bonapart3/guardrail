-- GuardRail Database Schema
-- Initialize all tables, types, and indexes

-- ============================================================================
-- Custom Types
-- ============================================================================

CREATE TYPE identity_type AS ENUM ('HUMAN', 'AGENT', 'ORGANIZATION');
CREATE TYPE key_type AS ENUM ('WALLET_ADDRESS', 'SIGNING_KEY', 'API_KEY', 'DEVICE_ID');
CREATE TYPE credential_type AS ENUM ('KYC_LEVEL', 'RISK_SCORE', 'JURISDICTION', 'SANCTIONS_STATUS', 'ACCREDITED_INVESTOR', 'CUSTOM');
CREATE TYPE decision AS ENUM ('ALLOW', 'DENY', 'REQUIRE_APPROVAL');
CREATE TYPE event_type AS ENUM (
    'POLICY_DECISION',
    'IDENTITY_CREATED',
    'IDENTITY_UPDATED',
    'KEY_ATTACHED',
    'KEY_DETACHED',
    'CREDENTIAL_ADDED',
    'CREDENTIAL_UPDATED',
    'APPROVAL_REQUESTED',
    'APPROVAL_GRANTED',
    'APPROVAL_REJECTED',
    'POLICY_CREATED',
    'POLICY_UPDATED',
    'ANCHOR_BATCH_CREATED',
    'SYSTEM_EVENT'
);
CREATE TYPE approval_status AS ENUM ('PENDING', 'APPROVED', 'REJECTED', 'EXPIRED');
CREATE TYPE anchor_status AS ENUM ('PENDING', 'ANCHORING', 'CONFIRMED', 'FAILED');

-- ============================================================================
-- Organizations (for multi-tenancy)
-- ============================================================================

CREATE TABLE organizations (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    slug VARCHAR(100) UNIQUE NOT NULL,
    metadata JSONB DEFAULT '{}',
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_organizations_slug ON organizations(slug);

-- ============================================================================
-- Identities
-- ============================================================================

CREATE TABLE identities (
    id UUID PRIMARY KEY,
    identity_type identity_type NOT NULL,
    external_id VARCHAR(255),
    display_name VARCHAR(255) NOT NULL,
    metadata JSONB DEFAULT '{}',
    organization_id UUID REFERENCES organizations(id),
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_identities_type ON identities(identity_type);
CREATE INDEX idx_identities_org ON identities(organization_id);
CREATE INDEX idx_identities_external ON identities(external_id);
CREATE INDEX idx_identities_display_name ON identities(display_name);
CREATE INDEX idx_identities_active ON identities(is_active) WHERE is_active = true;

-- ============================================================================
-- Identity Keys (wallets, signing keys, etc.)
-- ============================================================================

CREATE TABLE identity_keys (
    id UUID PRIMARY KEY,
    identity_id UUID NOT NULL REFERENCES identities(id) ON DELETE CASCADE,
    key_type key_type NOT NULL,
    public_key VARCHAR(500) NOT NULL,
    chain VARCHAR(50),
    label VARCHAR(255),
    is_primary BOOLEAN DEFAULT false,
    verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    
    UNIQUE(public_key, chain)
);

CREATE INDEX idx_identity_keys_identity ON identity_keys(identity_id);
CREATE INDEX idx_identity_keys_public_key ON identity_keys(public_key);
CREATE INDEX idx_identity_keys_chain ON identity_keys(chain);

-- ============================================================================
-- Credentials (KYC, risk scores, etc.)
-- ============================================================================

CREATE TABLE credentials (
    id UUID PRIMARY KEY,
    identity_id UUID NOT NULL REFERENCES identities(id) ON DELETE CASCADE,
    credential_type credential_type NOT NULL,
    provider VARCHAR(100) NOT NULL,
    value JSONB NOT NULL,
    expires_at TIMESTAMPTZ,
    verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_credentials_identity ON credentials(identity_id);
CREATE INDEX idx_credentials_type ON credentials(credential_type);
CREATE INDEX idx_credentials_expires ON credentials(expires_at) WHERE expires_at IS NOT NULL;

-- ============================================================================
-- Policies
-- ============================================================================

CREATE TABLE policies (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    version VARCHAR(20) NOT NULL,
    rego_source TEXT NOT NULL,
    is_active BOOLEAN DEFAULT true,
    created_by UUID REFERENCES identities(id),
    organization_id UUID REFERENCES organizations(id),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    
    UNIQUE(name, version, organization_id)
);

CREATE INDEX idx_policies_name ON policies(name);
CREATE INDEX idx_policies_active ON policies(is_active) WHERE is_active = true;
CREATE INDEX idx_policies_org ON policies(organization_id);

-- Policy version history
CREATE TABLE policy_versions (
    id UUID PRIMARY KEY,
    policy_id UUID NOT NULL REFERENCES policies(id) ON DELETE CASCADE,
    version VARCHAR(20) NOT NULL,
    rego_source TEXT NOT NULL,
    change_summary TEXT,
    created_by UUID REFERENCES identities(id),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_policy_versions_policy ON policy_versions(policy_id);

-- ============================================================================
-- Movement Events (append-only ledger)
-- ============================================================================

CREATE TABLE movement_events (
    id UUID PRIMARY KEY,
    sequence_number BIGSERIAL UNIQUE,
    event_type event_type NOT NULL,
    actor_id UUID NOT NULL REFERENCES identities(id),
    policy_decision_id UUID,
    payload JSONB NOT NULL,
    previous_hash VARCHAR(64) NOT NULL,
    event_hash VARCHAR(64) NOT NULL,
    anchor_batch_id UUID,
    organization_id UUID REFERENCES organizations(id),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Critical: ensure append-only semantics
CREATE INDEX idx_events_sequence ON movement_events(sequence_number);
CREATE INDEX idx_events_type ON movement_events(event_type);
CREATE INDEX idx_events_actor ON movement_events(actor_id);
CREATE INDEX idx_events_created ON movement_events(created_at);
CREATE INDEX idx_events_anchor ON movement_events(anchor_batch_id) WHERE anchor_batch_id IS NOT NULL;
CREATE INDEX idx_events_org ON movement_events(organization_id);
CREATE INDEX idx_events_hash ON movement_events(event_hash);

-- ============================================================================
-- Policy Decisions
-- ============================================================================

CREATE TABLE policy_decisions (
    id UUID PRIMARY KEY,
    identity_id UUID NOT NULL REFERENCES identities(id),
    policy_id UUID NOT NULL REFERENCES policies(id),
    policy_version VARCHAR(20) NOT NULL,
    action_type VARCHAR(50) NOT NULL,
    action_payload JSONB NOT NULL,
    context JSONB NOT NULL,
    decision decision NOT NULL,
    reasons TEXT[] DEFAULT '{}',
    required_approvers TEXT[] DEFAULT '{}',
    organization_id UUID REFERENCES organizations(id),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_decisions_identity ON policy_decisions(identity_id);
CREATE INDEX idx_decisions_policy ON policy_decisions(policy_id);
CREATE INDEX idx_decisions_decision ON policy_decisions(decision);
CREATE INDEX idx_decisions_created ON policy_decisions(created_at);
CREATE INDEX idx_decisions_org ON policy_decisions(organization_id);

-- ============================================================================
-- Approvals
-- ============================================================================

CREATE TABLE approvals (
    id UUID PRIMARY KEY,
    decision_id UUID NOT NULL REFERENCES policy_decisions(id),
    identity_id UUID NOT NULL REFERENCES identities(id),
    action JSONB NOT NULL,
    required_role VARCHAR(100) NOT NULL,
    status approval_status DEFAULT 'PENDING',
    approved_by UUID REFERENCES identities(id),
    approved_at TIMESTAMPTZ,
    rejection_reason TEXT,
    expires_at TIMESTAMPTZ NOT NULL,
    organization_id UUID REFERENCES organizations(id),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_approvals_status ON approvals(status);
CREATE INDEX idx_approvals_identity ON approvals(identity_id);
CREATE INDEX idx_approvals_expires ON approvals(expires_at);
CREATE INDEX idx_approvals_org ON approvals(organization_id);

-- ============================================================================
-- Anchor Batches
-- ============================================================================

CREATE TABLE anchor_batches (
    id UUID PRIMARY KEY,
    merkle_root VARCHAR(64) NOT NULL,
    start_sequence BIGINT NOT NULL,
    end_sequence BIGINT NOT NULL,
    event_count INTEGER NOT NULL,
    ethereum_tx_hash VARCHAR(66),
    ethereum_block BIGINT,
    solana_tx_signature VARCHAR(88),
    solana_slot BIGINT,
    status anchor_status DEFAULT 'PENDING',
    organization_id UUID REFERENCES organizations(id),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    anchored_at TIMESTAMPTZ
);

CREATE INDEX idx_anchors_status ON anchor_batches(status);
CREATE INDEX idx_anchors_merkle ON anchor_batches(merkle_root);
CREATE INDEX idx_anchors_sequence ON anchor_batches(start_sequence, end_sequence);
CREATE INDEX idx_anchors_org ON anchor_batches(organization_id);

-- ============================================================================
-- Users (for console access)
-- ============================================================================

CREATE TABLE users (
    id UUID PRIMARY KEY,
    email VARCHAR(255) UNIQUE NOT NULL,
    password_hash VARCHAR(255),
    identity_id UUID REFERENCES identities(id),
    role VARCHAR(50) NOT NULL DEFAULT 'VIEWER',
    organization_id UUID REFERENCES organizations(id),
    is_active BOOLEAN DEFAULT true,
    last_login_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_users_org ON users(organization_id);

-- ============================================================================
-- API Keys
-- ============================================================================

CREATE TABLE api_keys (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    key_hash VARCHAR(64) NOT NULL UNIQUE,
    key_prefix VARCHAR(10) NOT NULL,
    scopes TEXT[] DEFAULT '{}',
    organization_id UUID REFERENCES organizations(id),
    created_by UUID REFERENCES users(id),
    expires_at TIMESTAMPTZ,
    last_used_at TIMESTAMPTZ,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_api_keys_hash ON api_keys(key_hash);
CREATE INDEX idx_api_keys_org ON api_keys(organization_id);

-- ============================================================================
-- Webhook Configurations
-- ============================================================================

CREATE TABLE webhooks (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    url VARCHAR(2000) NOT NULL,
    secret VARCHAR(255) NOT NULL,
    events TEXT[] NOT NULL,
    is_active BOOLEAN DEFAULT true,
    organization_id UUID REFERENCES organizations(id),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_webhooks_org ON webhooks(organization_id);
CREATE INDEX idx_webhooks_active ON webhooks(is_active) WHERE is_active = true;

-- ============================================================================
-- Functions
-- ============================================================================

-- Function to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Apply updated_at trigger to relevant tables
CREATE TRIGGER update_organizations_updated_at BEFORE UPDATE ON organizations FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_identities_updated_at BEFORE UPDATE ON identities FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_credentials_updated_at BEFORE UPDATE ON credentials FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_policies_updated_at BEFORE UPDATE ON policies FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_users_updated_at BEFORE UPDATE ON users FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_webhooks_updated_at BEFORE UPDATE ON webhooks FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- ============================================================================
-- Seed Data (Development)
-- ============================================================================

-- Default organization
INSERT INTO organizations (id, name, slug, metadata)
VALUES ('00000000-0000-0000-0000-000000000001', 'GuardRail Demo', 'demo', '{"tier": "free"}');

-- System identity for internal operations
INSERT INTO identities (id, identity_type, display_name, metadata, organization_id)
VALUES ('00000000-0000-0000-0000-000000000001', 'AGENT', 'System', '{"internal": true}', '00000000-0000-0000-0000-000000000001');

-- Demo admin user
-- INSERT INTO users (id, email, password_hash, role, organization_id, identity_id)
-- VALUES (
--     '00000000-0000-0000-0000-000000000002',
--     'admin@guardrail.dev',
--     -- Password: 'admin123' (argon2 hash - generated by Rust argon2 0.5)
--     '$argon2id$v=19$m=19456,t=2,p=1$icuzQmP3EqQkPFKfs8rSFA$/MMA75O261m639TeJxI+r1TJF+U+Nf7wACjtyWXNJkk',
--     'ADMIN',
--     '00000000-0000-0000-0000-000000000001',
--     NULL
-- );
