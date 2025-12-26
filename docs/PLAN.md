# GuardRail - Project Plan

> Risk and control layer for AI and automated money-moving in crypto: identity + policy + movement history for high-risk users and agents.

## Problem Statement

Exchanges, trading desks, on/off-ramps, and RWA/CeDeFi/fintech platforms face growing pressure to:

1. **Prove who initiated** large or sensitive actions and under what policy
2. **Allow AI/scripted agents** to operate without unlimited fraud/compliance risk
3. **Produce clean, regulator-friendly, cryptographically verifiable logs** of user and agent activity

Existing solutions either:

- Focus on KYC alone (who you are) without programmable policies and movement trails
- Are internal/custom and not reusable or standardized across chains and platforms

## Target Users

### Primary Customers (Initial Focus)

- Centralized and hybrid exchanges / prime brokers
- Prop-trading desks and crypto market-makers using bots/agents
- RWA / CeDeFi platforms, crypto banks, and fintechs providing yield, credit, or tokenized assets

### End Users

- Risk and compliance teams (dashboard users)
- Developers integrating via SDK
- Operations teams managing approvals

## Core Features (MVP)

### Identity & Credential Service

- [ ] Represent humans, AI/agents/bots, organizations, and roles
- [ ] Store core profiles (IDs, KYC provider refs, risk scores)
- [ ] Bind cryptographic keys/identifiers (wallet addresses, agent keys, device IDs)
- [ ] Store credential references (KYC status, jurisdiction flags, sanctions status)
- [ ] Pluggable adapters for external KYC/AML providers

### Policy Engine

- [ ] Policy definition via OPA/Rego DSL
- [ ] Real-time rules evaluation on every sensitive action
- [ ] Policy versioning and history (linked to movement events for audit)
- [ ] Decision outcomes: ALLOW, DENY, REQUIRE_APPROVAL
- [ ] Test harness for simulating policies against historical data

### Movement / Audit Ledger

- [ ] Append-only event store with immutable semantics
- [ ] Every event captures: actor, policy context, action details, decision, cryptographic commitments
- [ ] Hash-chaining for tamper evidence
- [ ] Query layer indexed by identity, policy, action type
- [ ] CQRS separation for read/write optimization

### Chain Anchoring

- [ ] Periodic Merkle tree commitments to Ethereum L2
- [ ] Periodic Merkle tree commitments to Solana
- [ ] Smart contracts/programs storing commitments with metadata
- [ ] Proof reconstruction APIs

### Admin Console

- [ ] Policy builder (wizard + advanced Rego editor)
- [ ] Timeline view of high-risk events and approvals
- [ ] Investigator view: search by identity, wallet, or agent
- [ ] Compliance export (PDF/CSV/JSON with cryptographic proofs)

### SDKs

- [ ] TypeScript/Node SDK with `checkAction()` interface
- [ ] Python SDK with equivalent functionality
- [ ] Webhook/streaming support for real-time enforcement

## Data Model

### Core Entities

```
Identity
├── id (UUID)
├── type (HUMAN | AGENT | ORGANIZATION)
├── profile (name, metadata)
├── credentials[] (KYC refs, risk scores, jurisdiction)
├── keys[] (wallet addresses, signing keys)
└── roles[] (permission sets)

Policy
├── id (UUID)
├── version (semver)
├── rego_source (OPA policy code)
├── metadata (name, description, active)
└── created_at, updated_at

PolicyDecision
├── id (UUID)
├── policy_id + version
├── identity_id
├── action (type, amount, asset, target)
├── context (input data)
├── decision (ALLOW | DENY | REQUIRE_APPROVAL)
├── reasons[]
└── timestamp

MovementEvent
├── id (UUID)
├── sequence_number
├── actor_id (identity or agent)
├── policy_decision_id
├── action_details (JSON)
├── previous_hash
├── event_hash
├── anchor_batch_id (nullable)
└── timestamp

AnchorBatch
├── id (UUID)
├── merkle_root
├── event_range (start_seq, end_seq)
├── ethereum_tx_hash (nullable)
├── solana_tx_hash (nullable)
├── anchored_at
└── metadata
```

## Integrations

### Authentication & Authorization

- JWT-based API authentication
- RBAC for console access
- API key management for SDK integrations

### External Services

- Pluggable KYC/AML provider adapters (Chainalysis, Elliptic, etc.)
- Webhook endpoints for customer system integration
- Kafka/WebSocket streaming for real-time events

### Blockchain

- Ethereum L2 (Arbitrum/Base) for anchoring
- Solana for anchoring
- Read: verify transaction origin, wallet ownership
- Write: anchor Merkle commitments

## Deployment Target

### Production Architecture

- Containerized microservices (Docker)
- Kubernetes orchestration
- PostgreSQL (managed) for persistent storage
- Redis for caching and pub/sub
- Multi-region capable

### Development

- Docker Compose for local development
- Hot reload for all services
- Local PostgreSQL + Redis

---

## Technology Stack

### Backend Services

| Service | Language | Framework | Justification |
|---------|----------|-----------|---------------|
| Identity Service | Rust | Axum | Memory safety critical for credential handling, type safety |
| Policy Engine | Rust + OPA | Axum + OPA | OPA is CNCF-graduated industry standard, evaluates thousands of policies/sec |
| Movement Ledger | Rust | Axum | Append-only event sourcing requires correctness guarantees |
| Chain Anchor | Rust | Axum | Native Solana ecosystem (Solana is Rust-based), Ethereum via ethers-rs |
| API Gateway | Rust | Axum | Unified stack, consistent error handling, high throughput |

**Why Rust over Go for this project:**

- Memory safety without GC pauses — critical for real-time policy decisions
- Native Solana ecosystem (Solana programs are Rust)
- `regorus` crate provides native Rego evaluation (no OPA sidecar needed)
- Type system catches policy/identity mismatches at compile time
- Industry trend: fintech increasingly adopting Rust for compliance-critical systems

### Policy Engine

| Component | Choice | Justification |
|-----------|--------|---------------|
| Policy Language | Rego (OPA) | CNCF graduated, industry standard, declarative, auditable |
| Evaluator | `regorus` crate | Native Rust Rego evaluator, no sidecar, thousands of evals/sec |
| Policy Storage | PostgreSQL | Versioned policies with migration history |

### Frontend Console

| Component | Choice | Justification |
|-----------|--------|---------------|
| Framework | Next.js 14 (App Router) | Enterprise standard, SSR for compliance dashboards |
| Language | TypeScript | Type safety across API contracts |
| UI Library | shadcn/ui + Radix | Accessible, customizable, enterprise-grade components |
| Styling | Tailwind CSS | Rapid iteration, consistent design system |
| Charts | Recharts | Policy decision analytics, event timelines |
| Tables | TanStack Table | High-performance data grids for audit logs |
| Forms | React Hook Form + Zod | Policy builder, identity management |
| State | Zustand + TanStack Query | Server state caching, minimal boilerplate |

### Database & Storage

| Component | Choice | Justification |
|-----------|--------|---------------|
| Primary DB | PostgreSQL (Supabase) | Managed, built-in dashboard, scales vertically |
| Event Store | PostgreSQL (append-only tables) | ACID guarantees for audit trail |
| Cache | Redis (Upstash) | Policy decision caching, pub/sub for real-time |
| File Storage | Supabase Storage | Compliance exports, policy backups |

### Infrastructure & Hosting

| Component | Choice | Justification |
|-----------|--------|---------------|
| Backend Services | Railway or Fly.io | Container-native, auto-scaling, cost-effective |
| Database | Supabase | Managed PostgreSQL, built-in auth primitives |
| Redis | Upstash | Serverless Redis, pay-per-request |
| Frontend | Vercel | Native Next.js optimization, edge functions |
| Monitoring | Axiom or Grafana Cloud | Log aggregation, metrics |

### Blockchain Integration

| Chain | Approach | Libraries |
|-------|----------|-----------|
| Ethereum L2 (Base/Arbitrum) | Merkle commitment contracts | `ethers-rs`, `alloy` |
| Solana | Anchor program for commitments | `anchor-lang`, `solana-sdk` |

### SDKs

| SDK | Language | Distribution |
|-----|----------|--------------|
| TypeScript | TypeScript | npm package |
| Python | Python 3.10+ | PyPI package |

---

## Console Features (Full from Day 1)

### Dashboard

- Real-time policy decision feed
- Key metrics: decisions/hour, approval rate, blocked actions
- Risk score distribution charts
- Recent high-risk events

### Identity Management

- Create/edit humans, agents, organizations
- Credential binding (wallets, keys)
- KYC status tracking
- Risk score management
- Role assignment

### Policy Builder

- Visual policy wizard for common rules
- Advanced Rego editor with syntax highlighting
- Policy simulation/testing against sample data
- Version history with diff view
- Activate/deactivate policies

### Movement Ledger / Audit Trail

- Searchable event timeline
- Filter by identity, action type, decision, date range
- Event detail view with full context
- Proof verification (show Merkle path to on-chain anchor)
- Export to PDF/CSV/JSON with cryptographic attestation

### Approval Workflow

- Pending approvals queue
- Approve/reject with comments
- Escalation rules
- Notification preferences

### Settings & Configuration

- Organization settings
- API key management
- Webhook configuration
- Chain anchoring settings (frequency, chains)
- User management (RBAC)

---

## Customer Model

**Target**: External customers (exchanges, trading desks, RWA platforms)
**Design Partner #1**: Tyler / Veridicus ecosystem
**Pricing Model** (future):

- Base subscription by org size
- Usage: identities/agents under governance, policy checks, events
- Premium: advanced analytics, custom deployments

---

## Success Metrics

- Policy evaluation latency < 10ms p99
- Event ingestion throughput > 10k events/sec
- Anchor batch frequency: configurable (default: hourly)
- Console page load < 2s
- SDK integration time < 1 dayput |

### Policy Engine Details

- **OPA (Open Policy Agent)**: CNCF-graduated, evaluates thousands of policies/second
- **Rego**: Purpose-built declarative policy language for complex hierarchical data
- **Deployment**: OPA as sidecar or embedded via `opa-wasm` for lowest latency

### Frontend

| Component | Technology | Justification |
|-----------|------------|---------------|
| Framework | Next.js 14 (App Router) | SSR for SEO, RSC for performance, enterprise ecosystem |
| Language | TypeScript | Type safety, SDK alignment |
| UI Library | shadcn/ui + Tailwind | Accessible, customizable, professional aesthetic |
| State | Zustand + TanStack Query | Lightweight, server state separation |
| Charts | Recharts | Timeline visualizations, audit dashboards |
| Auth | NextAuth.js | Enterprise SSO, JWT handling |

### Database & Storage

| Component | Technology | Justification |
|-----------|------------|---------------|
| Primary DB | PostgreSQL 15+ | ACID, JSON support, mature tooling |
| Event Store | PostgreSQL (append-only tables) | Event sourcing with hash chains |
| Cache | Redis | Policy decision caching, pub/sub for real-time |
| Search | PostgreSQL Full-Text (initially) | Scales to Elasticsearch later if needed |

### Blockchain Integration

| Chain | Technology | Purpose |
|-------|------------|---------|
| Ethereum L2 | Arbitrum via ethers-rs | Low-cost Merkle root anchoring |
| Solana | Anchor framework | Native Rust, fast finality anchoring |

### Infrastructure & Deployment

| Component | Provider | Justification |
|-----------|----------|---------------|
| Backend Hosting | Fly.io | Rust containers, multi-region, auto-scaling |
| Frontend Hosting | Vercel | Next.js native, edge functions, preview deploys |
| Database | Supabase | Managed Postgres, connection pooling, backups |
| Cache | Upstash | Serverless Redis, pay-per-request |
| Secrets | Doppler or Infisical | Centralized secrets management |
| CI/CD | GitHub Actions | Standard, free for public repos |
| Monitoring | Axiom + Sentry | Logs, traces, error tracking |

---

## Go-to-Market

### Initial Wedge

- 2-3 design partners: one exchange/brokerage, one RWA/DeFi platform, one prop-desk with bots
- Pilot focused on large withdrawals OR agent trading permissions
- Co-design policies and dashboards with partners

### Pricing Model

- Base subscription by organization size and environment count
- Usage component: identities/agents under governance, policy checks volume
- Premium: advanced analytics, custom data residency, dedicated support

---

## Success Metrics

### Technical

- Policy evaluation latency < 10ms p99
- Event ingestion throughput > 10,000 events/sec
- Zero data loss (append-only with replication)
- Anchor batches within 1 hour of events

### Business

- 3 design partners onboarded in Phase 1
- < 1 day integration time with SDK
- Compliance audit prep time reduced by 50%+
