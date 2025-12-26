# GuardRail - Architecture

## System Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              CUSTOMER SYSTEMS                                │
│  (Exchanges, Trading Desks, RWA Platforms)                                  │
└─────────────────────┬───────────────────────────────────────────────────────┘
                      │ SDK / API
                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                            API GATEWAY (Rust/Axum)                          │
│  • Authentication (JWT/API Keys)  • Rate limiting  • Request routing        │
└────────┬─────────────────┬─────────────────┬─────────────────┬──────────────┘
         │                 │                 │                 │
         ▼                 ▼                 ▼                 ▼
┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐
│  IDENTITY   │  │   POLICY    │  │  MOVEMENT   │  │      CHAIN ANCHOR       │
│   SERVICE   │  │   ENGINE    │  │   LEDGER    │  │        SERVICE          │
└──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └───────────┬─────────────┘
       └────────────────┴────────────────┴─────────────────────┘
                                  │
       ┌──────────────────────────┼──────────────────────────┐
       ▼                          ▼                          ▼
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   PostgreSQL    │    │     Redis       │    │   Blockchain    │
│   (Supabase)    │    │   (Upstash)     │    │  ETH L2 + SOL   │
└─────────────────┘    └─────────────────┘    └─────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│                         ADMIN CONSOLE (Next.js)                             │
│  Dashboard │ Identities │ Policies │ Audit Log │ Approvals │ Settings      │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Directory Structure

```
guardrail/
├── docs/                    # Documentation (source of truth)
├── backend/
│   ├── api-gateway/         # Request routing, auth, rate limiting
│   ├── identity-service/    # Identity and credential management
│   ├── policy-engine/       # OPA/Rego policy evaluation
│   ├── movement-ledger/     # Event sourcing and audit trail
│   ├── chain-anchor/        # Blockchain anchoring service
│   └── shared/              # Shared types and utilities
├── frontend/                # Next.js admin console
├── contracts/
│   ├── ethereum/            # Solidity anchor contract
│   └── solana/              # Anchor program
├── sdks/
│   ├── typescript/          # TypeScript SDK
│   └── python/              # Python SDK
├── scripts/                 # Dev utilities
└── infrastructure/          # Docker, deployment configs
```

## Security & Deployment

- **Container Security**: All services run as non-privileged `guardrail` user.
- **Secrets Management**: Secrets are injected via environment variables (no hardcoded credentials).
- **Graceful Shutdown**: Services handle SIGTERM for safe termination.
- **Health Checks**: All services expose `/health` endpoint.

## API Design

### Core Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/v1/identities` | Create identity |
| GET | `/api/v1/identities` | List identities |
| GET | `/api/v1/identities/:id` | Get identity |
| PUT | `/api/v1/identities/:id` | Update identity |
| POST | `/api/v1/policies` | Create policy |
| GET | `/api/v1/policies` | List policies |
| POST | `/api/v1/policies/:id/simulate` | Test policy |
| **POST** | **`/api/v1/check`** | **Evaluate action** |
| GET | `/api/v1/events` | List audit events |
| GET | `/api/v1/events/:id/proof` | Get Merkle proof |
| GET | `/api/v1/approvals` | List pending approvals |
| POST | `/api/v1/approvals/:id/approve` | Approve action |

### Check Action (Primary SDK Interface)

**Request:**

```json
{
  "identity_id": "uuid",
  "action": {
    "type": "WITHDRAWAL",
    "amount": "50000",
    "asset": "USDC",
    "destination": "0x...",
    "chain": "ethereum"
  },
  "context": { "ip_address": "...", "timestamp": "..." }
}
```

**Response:**

```json
{
  "decision": "REQUIRE_APPROVAL",
  "reasons": ["Amount exceeds daily limit", "New destination"],
  "approval_requirements": { "required_roles": ["risk_officer"] },
  "decision_id": "uuid"
}
```

## Data Flow

### Policy Check Flow

1. Customer System → `POST /api/v1/check`
2. API Gateway authenticates
3. Identity Service retrieves identity + credentials
4. Policy Engine evaluates Rego policies
5. Movement Ledger records decision (hash-chained)
6. If `REQUIRE_APPROVAL`: create pending approval, notify
7. Return decision

### Anchor Flow (Hourly)

1. Query uncommitted events
2. Build Merkle tree from event hashes
3. Submit root to Ethereum L2 + Solana
4. Store tx hashes, mark events anchored

## Security Model

### Authentication

- Console: JWT (Supabase Auth)
- API: API Key + Secret (hashed with Argon2)
- Internal: mTLS

### RBAC Roles

- `super_admin`: Full access
- `risk_officer`: Approvals, all events
- `compliance`: Read-only audit, exports
- `developer`: API keys, policy testing
- `viewer`: Read-only dashboard

## Event Schema

```rust
struct MovementEvent {
    id: Uuid,
    sequence_number: i64,
    event_type: EventType,
    actor_id: Uuid,
    action: ActionDetails,
    previous_hash: String,    // Hash chain
    event_hash: String,
    anchor_batch_id: Option<Uuid>,
    created_at: DateTime<Utc>,
}
```

## Performance Targets

| Metric | Target |
|--------|--------|
| Policy check latency | < 10ms p50, < 50ms p99 |
| Event write throughput | > 10,000/sec |
| Console page load | < 2s |
| API availability | 99.9% |
