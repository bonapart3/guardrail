// GuardRail API Types

export type IdentityType = 'HUMAN' | 'AGENT' | 'ORGANIZATION';
export type KeyType = 'WALLET_ADDRESS' | 'SIGNING_KEY' | 'API_KEY' | 'DEVICE_ID';
export type CredentialType = 'KYC_LEVEL' | 'RISK_SCORE' | 'JURISDICTION' | 'SANCTIONS_STATUS' | 'ACCREDITED_INVESTOR' | 'CUSTOM';
export type Decision = 'ALLOW' | 'DENY' | 'REQUIRE_APPROVAL';
export type EventType = 
  | 'POLICY_DECISION'
  | 'IDENTITY_CREATED'
  | 'IDENTITY_UPDATED'
  | 'KEY_ATTACHED'
  | 'KEY_DETACHED'
  | 'CREDENTIAL_ADDED'
  | 'CREDENTIAL_UPDATED'
  | 'APPROVAL_REQUESTED'
  | 'APPROVAL_GRANTED'
  | 'APPROVAL_REJECTED'
  | 'POLICY_CREATED'
  | 'POLICY_UPDATED'
  | 'ANCHOR_BATCH_CREATED'
  | 'SYSTEM_EVENT';
export type ApprovalStatus = 'PENDING' | 'APPROVED' | 'REJECTED' | 'EXPIRED';
export type AnchorStatus = 'PENDING' | 'ANCHORING' | 'CONFIRMED' | 'FAILED';

// Identity Types
export interface Identity {
  id: string;
  identity_type: IdentityType;
  external_id?: string;
  display_name: string;
  metadata: Record<string, unknown>;
  organization_id?: string;
  is_active: boolean;
  created_at: string;
  updated_at: string;
}

export interface IdentityKey {
  id: string;
  identity_id: string;
  key_type: KeyType;
  public_key: string;
  chain?: string;
  label?: string;
  is_primary: boolean;
  verified_at?: string;
  created_at: string;
}

export interface Credential {
  id: string;
  identity_id: string;
  credential_type: CredentialType;
  provider: string;
  value: Record<string, unknown>;
  expires_at?: string;
  verified_at?: string;
  created_at: string;
  updated_at: string;
}

// Policy Types
export interface Policy {
  id: string;
  name: string;
  description?: string;
  version: string;
  rego_source: string;
  is_active: boolean;
  created_by?: string;
  created_at: string;
  updated_at: string;
}

export interface PolicyDecision {
  decision_id: string;
  decision: Decision;
  reasons: string[];
  required_approvers: string[];
  policy_id: string;
  policy_version: string;
  evaluated_at: string;
}

// Event Types
export interface MovementEvent {
  id: string;
  sequence_number: number;
  event_type: EventType;
  actor_id: string;
  policy_decision_id?: string;
  payload: Record<string, unknown>;
  previous_hash: string;
  event_hash: string;
  anchor_batch_id?: string;
  created_at: string;
}

export interface EventWithProof {
  event: MovementEvent;
  proof?: MerkleProof;
}

export interface MerkleProof {
  event_hash: string;
  siblings: ProofSibling[];
  root: string;
  anchor_batch_id: string;
  ethereum_tx_hash?: string;
  solana_tx_signature?: string;
}

export interface ProofSibling {
  hash: string;
  position: 'left' | 'right';
}

// Approval Types
export interface Approval {
  id: string;
  decision_id: string;
  identity_id: string;
  action: Record<string, unknown>;
  required_role: string;
  status: ApprovalStatus;
  approved_by?: string;
  approved_at?: string;
  rejection_reason?: string;
  expires_at: string;
  created_at: string;
}

// Anchor Types
export interface AnchorBatch {
  id: string;
  merkle_root: string;
  start_sequence: number;
  end_sequence: number;
  event_count: number;
  ethereum_tx_hash?: string;
  ethereum_block?: number;
  solana_tx_signature?: string;
  solana_slot?: number;
  status: AnchorStatus;
  created_at: string;
  anchored_at?: string;
}

// API Response Types
export interface ApiResponse<T> {
  success: boolean;
  data?: T;
  error?: ApiError;
}

export interface ApiError {
  code: string;
  message: string;
  details?: Record<string, unknown>;
}

export interface PaginatedResponse<T> {
  items: T[];
  total: number;
  page: number;
  per_page: number;
  total_pages: number;
}

// Auth Types
export interface User {
  id: string;
  email: string;
  role: string;
}

export interface LoginResponse {
  token: string;
  expires_at: string;
  user: User;
}

// Dashboard Stats
export interface DashboardStats {
  total_identities: number;
  total_policies: number;
  total_events: number;
  pending_approvals: number;
  decisions_today: number;
  approval_rate: number;
  anchored_events: number;
}

export interface EventTypeCount {
  event_type: string;
  count: number;
}

export interface LedgerStats {
  total_events: number;
  events_by_type: EventTypeCount[];
  unanchored_events: number;
  last_anchor_time?: string;
}

// Request Types
export interface CreateIdentityRequest {
  identity_type: IdentityType;
  external_id?: string;
  display_name: string;
  metadata?: Record<string, unknown>;
  organization_id?: string;
}

export interface CreatePolicyRequest {
  name: string;
  description?: string;
  rego_source: string;
}

export interface CheckActionRequest {
  identity_id: string;
  action: {
    action_type: string;
    amount?: string;
    asset?: string;
    source_address?: string;
    target_address?: string;
    metadata?: Record<string, unknown>;
  };
  context: {
    ip_address?: string;
    device_id?: string;
    user_agent?: string;
    geo_location?: string;
    timestamp: string;
    session_id?: string;
    metadata?: Record<string, unknown>;
  };
}
