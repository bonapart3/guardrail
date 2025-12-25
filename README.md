# GuardRail

**Risk and Control Layer for AI/Automated Crypto Money Movement**

GuardRail provides enterprise-grade compliance infrastructure for exchanges, trading desks, RWA platforms, and prop shops operating in the crypto space. It combines identity management, policy-based access control, and immutable audit trails with on-chain anchoring.

## 🎯 Core Features

### Three Pillars

1. **Identity Layer** - Unified identity management for humans, AI agents, and organizations with credential attestations
2. **Policy Engine** - OPA/Rego-based policy evaluation with real-time decision making
3. **Movement Ledger** - Append-only, hash-chained audit trail with blockchain anchoring

### Key Capabilities

- **Policy-Based Control**: Define complex rules in Rego (same as Kubernetes, Netflix, Uber)
- **Multi-Signer Approvals**: Route high-risk actions to human reviewers
- **Tamper-Evident Logs**: Hash-chained events with Merkle proofs
- **Dual-Chain Anchoring**: Commit audit roots to Ethereum L2 + Solana
- **Agent Guardrails**: Special policies for AI/automated trading agents
- **Real-Time Decisions**: Sub-10ms policy evaluation at scale

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         API Gateway (3000)                          │
│                    JWT/API Key Auth • Rate Limiting                 │
└────────────────────────────────┬────────────────────────────────────┘
                                 │
        ┌────────────────────────┼────────────────────────┐
        ▼                        ▼                        ▼
┌───────────────┐      ┌─────────────────┐      ┌─────────────────┐
│   Identity    │      │  Policy Engine  │      │ Movement Ledger │
│  Service (1)  │      │      (2)        │      │      (3)        │
│               │      │                 │      │                 │
│ • Identities  │◄────►│ • Rego Eval     │◄────►│ • Event Store   │
│ • Keys/Wallets│      │ • Policy CRUD   │      │ • Hash Chain    │
│ • Credentials │      │ • Simulation    │      │ • Merkle Proofs │
└───────────────┘      └─────────────────┘      └────────┬────────┘
                                                         │
                                                         ▼
                                               ┌─────────────────┐
                                               │  Chain Anchor   │
                                               │      (4)        │
                                               │                 │
                                               │ • Batch Commits │
                                               │ • Ethereum L2   │
                                               │ • Solana        │
                                               └─────────────────┘
```

## 🚀 Quick Start

### Prerequisites

- Docker & Docker Compose
- Rust 1.75+ (for local backend development)
- Node.js 20+ (for local frontend development)

### One-Command Start

```bash
# Clone and enter project
cd guardrail

# Start everything with one command
./scripts/start.sh

# Or use Docker for all services
./scripts/start.sh --docker
```

### Manual Setup

```bash
# Copy environment file
cp .env.example .env

# Start infrastructure (Postgres, Redis)
docker-compose up -d postgres redis

# Run database migrations
psql $DATABASE_URL < scripts/init.sql

# Start backend (in separate terminals or use docker-compose)
cd backend
cargo run --bin api-gateway
cargo run --bin identity-service
cargo run --bin policy-engine
cargo run --bin movement-ledger
cargo run --bin chain-anchor

# Start frontend
cd ../frontend && npm install && npm run dev
```

### Useful Commands

```bash
./scripts/start.sh          # Start all services
./scripts/start.sh --docker # Start with Docker
./scripts/stop.sh           # Stop all services
./scripts/logs.sh           # View all logs
./scripts/logs.sh policy    # View policy engine logs
```

### Default Credentials

- **Email**: admin@guardrail.dev
- **Password**: admin123

## 📁 Project Structure

```
guardrail/
├── backend/
│   ├── shared/              # Common types, errors, utilities
│   ├── api-gateway/         # Auth, routing, rate limiting
│   ├── identity-service/    # Identity CRUD, keys, credentials
│   ├── policy-engine/       # Rego evaluation, policy management
│   ├── movement-ledger/     # Event sourcing, hash chains
│   └── chain-anchor/        # Blockchain anchoring
├── frontend/
│   ├── app/                 # Next.js 14 app router
│   ├── components/          # React components
│   └── lib/                 # API client, utilities
├── contracts/
│   ├── ethereum/            # Solidity contracts (Base L2)
│   └── solana/              # Anchor programs
├── sdk/
│   ├── typescript/          # TypeScript SDK
│   └── python/              # Python SDK
├── infrastructure/
│   └── docker-compose.yml   # Local dev environment
├── scripts/
│   └── init.sql             # Database schema
└── docs/
    ├── PLAN.md              # Requirements & design
    ├── ARCHITECTURE.md      # System architecture
    └── TODO.md              # Task tracking
```

## 🔌 API Reference

### Check Action (Main Entry Point)

```bash
POST /api/v1/check
Authorization: Bearer <token>

{
  "identity_id": "550e8400-e29b-41d4-a716-446655440000",
  "action": {
    "action_type": "WITHDRAWAL",
    "amount": "50000",
    "asset": "USDC",
    "target_address": "0x742d35Cc6634C0532925a3b844Bc9e7595f..."
  },
  "context": {
    "ip_address": "192.168.1.1",
    "timestamp": "2024-12-25T12:00:00Z"
  }
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "decision_id": "a1b2c3d4-...",
    "decision": "REQUIRE_APPROVAL",
    "reasons": [
      "Amount exceeds tier limit ($10,000 for Tier 1)",
      "New destination address"
    ],
    "required_approvers": ["risk_officer"],
    "policy_id": "withdrawal-limits-v1",
    "policy_version": "1.0.0"
  }
}
```

## 📜 Sample Rego Policy

```rego
package guardrail

import future.keywords.if
import future.keywords.in
import future.keywords.contains

default deny := []
default require_approval := []

# Deny if no KYC
deny contains "KYC verification required" if {
    not has_kyc_credential
}

# Require approval for large amounts
require_approval contains "risk_officer" if {
    input.action.action_type == "WITHDRAWAL"
    to_number(input.action.amount) > 10000
}

has_kyc_credential if {
    input.identity.credentials[_].type == "KYC_LEVEL"
}
```

## 🔗 SDK Usage

### TypeScript

```typescript
import GuardRailClient from '@guardrail/sdk';

const client = new GuardRailClient({
  baseUrl: 'https://api.guardrail.dev',
  apiKey: 'gr_your_api_key',
});

const decision = await client.checkWithdrawal({
  identityId: 'user-123',
  amount: '50000',
  asset: 'USDC',
  destinationAddress: '0x...',
});
```

### Python

```python
from guardrail_sdk import GuardRailClient

client = GuardRailClient(
    base_url="https://api.guardrail.dev",
    api_key="gr_your_api_key",
)

decision = client.check_withdrawal(
    identity_id="user-123",
    amount="50000",
    asset="USDC",
    destination_address="0x...",
)
```

## 📊 Performance Targets

| Metric | Target |
|--------|--------|
| Policy check latency | < 10ms p50, < 50ms p99 |
| Event write throughput | > 10,000/sec |
| API availability | 99.9% |

## 📄 License

MIT License

---

Built with ❤️ for the crypto compliance community
