# GuardRail Copilot Instructions

## Big Picture Architecture
GuardRail is a microservices-based compliance layer for crypto/AI money movements, with 5 Rust services (api-gateway, identity-service, policy-engine, movement-ledger, chain-anchor) coordinated via Axum routers. Data flows: API Gateway authenticates (JWT/API keys) and proxies to services; Identity binds keys/creds; Policy evaluates Rego rules; Ledger hash-chains events; Anchor commits Merkle roots to ETH L2/Solana. Why: Separation ensures auditability—e.g., tamper-proof logs for truth DAOs. Frontend: Next.js admin console with shadcn/ui components. Blockchain: Solidity (GuardRailAnchor.sol) + Anchor program. SDKs: TS/Python clients for integration. See ARCHITECTURE.md for diagrams, PLAN.md for entities.

## Critical Developer Workflows
- **Local Dev**: Run `./scripts/start.sh --docker` for full stack (Postgres/Redis/services). Stop with `./scripts/stop.sh`; logs via `./scripts/logs.sh [service]`. Env from .env.example (e.g., DATABASE_URL=postgres://user:pass@localhost:5432/guardrail).
- **Build/Test**: `cargo test` for units (e.g., policy eval in policy-engine/tests/); `cargo check -p [crate]` for quick validation. Integration: Run `integration_test.py`. CI/CD: GitHub Actions (.github/workflows/ci.yml) lints/tests/builds; deploy via deploy.yml (push to staging branch).
- **Debugging**: Tracing JSON logs (tracing-subscriber in main.rs files); health checks at /health per service. Graceful shutdown: tokio::signal in each main.rs for SIGTERM.
- **Deployment**: Staging: Fly.io for backend (`flyctl deploy`), Vercel for frontend (`vercel deploy`). Env schema: DATABASE_URL (postgres URI), REDIS_URL (Upstash), ETH_RPC_URL (Alchemy/Infura), SOLANA_RPC_URL. Monitoring: Scrape /health; add Sentry via env SENTRY_DSN.
- **Database Changes**: Update scripts/init.sql, run migrations with sqlx-cli.

## Project-Specific Conventions
- **Error Handling**: No .unwrap() in prod—use ? or match on Result (e.g., api-gateway/src/main.rs JWT parsing). Custom errors in shared/src/errors.rs.
- **Logging/Monitoring**: Structured JSON via tracing::info! macros; uniform across services with tracing-subscriber::fmt().set_json(). Event levels: debug for queries, error for failures. Health endpoints return service stats (e.g., chain-anchor batch status).
- **API Patterns**: Axum handlers with #[utoipa::path] for OpenAPI (api-gateway/src/main.rs). Responses wrapped in shared::ApiResponse<T>.
- **ZK Integration**: Bind proofs in zk_credential.rs (halo2/arkworks); extend policy-engine Rego for ZK eval (e.g., prove KYC without data leak). Mock flows: POST /identities/:id/zk-bind.
- **Blockchain Mocks**: Test anchors on Sepolia/Devnet; extend GuardRailAnchor.sol for 50% donation splits (e.g., to truth DAO address).
- **UI Patterns**: shadcn/ui for components (e.g., DataTable in identity list); Tailwind for styling. Monaco for Rego editor in policy-builder.
- **SDK Usage**: Clients auth via apiKey (hashed Argon2); e.g., TS: new GuardRailClient({baseUrl, apiKey}).checkAction(). Python similar with type hints.

## Integration Points & Communication
- **Inter-Service**: API Gateway proxies HTTP to localhost:3001-3004; shared crate for types/errors.
- **Externals**: Postgres (SQLx queries in services), Redis (caching/pubsub), ETH (ethers-rs in chain-anchor), Solana (solana-sdk). xAI Sim: Reqwest calls in policies for Grok API (e.g., semantic event search).
- **Common Tasks**:
  - Add Endpoint: Route in api-gateway/src/main.rs, handler in target service, utoipa docs.
  - Policy Test: POST /policies/:id/simulate with JSON input (policy-engine).
  - ZK Bootstrap: Add halo2 circuit in zk_credential.rs; bind via identity-service.
  - Rate Limiting: axum-middleware + governor in api-gateway (Redis-backed).

## Backlog Prioritization (Dep-Aware)
- Docs/Tests first (utoipa/OpenAPI, Playwright E2E, Foundry/Anchor contracts).
- Rate Limiting (security gate before UI).
- ZK-SNARKs (core for privacy; dep on shared types).
- UI Additions (roles/webhooks; dep on RBAC in identity).
- Performance (criterion benches post-ZK).

Reference: README.md quick-start, TODO.md backlog, PROGRESS.md sessions.</content>
<parameter name="filePath">c:\Users\tyler\Downloads\guardrail-complete\guardrail\.github\copilot-instructions.md