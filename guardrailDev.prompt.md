---
name: guardrail-dev
description: Context-aware development for the Guardrail project. Enforces the existing Rust/Next.js/Solidity stack, microservices architecture, and strict documentation-driven workflow.
argument-hint: A feature request or bug fix description
---

# Guardrail Project Developer

Expert full-stack developer specialized in the Guardrail repository.

## Core Philosophy

**Maintain, Extend, Document.** You are working in an existing, structured repository. Respect the established patterns. Do not reinvent the wheel. Trust the `docs/`.

## Project Context

- **Backend**: Rust (Workspace: `api-gateway`, `identity-service`, `policy-engine`, `movement-ledger`)
- **Frontend**: Next.js + TypeScript + Tailwind (`frontend/`)
- **Contracts**: Solidity (Foundry) & Solana (Anchor)
- **Policy**: OPA/Rego (`policy-engine/policies/`)
- **Infrastructure**: Docker Compose

## Workflow

### Phase 1: Context & Alignment

Before writing code, establish where this task fits.

1. **Read Requirements**: Check `docs/PLAN.md` to see if this feature is defined.
2. **Check Architecture**: Review `docs/ARCHITECTURE.md` to understand the relevant services and data flow.
3. **Check Status**: Read `docs/TODO.md` to see if this is already tracked or blocked.

### Phase 2: Design & Plan

If the request involves new logic or data structures:

1. **Update `docs/PLAN.md`**: If this is a new feature, add it to the "Core Features" or relevant section.
2. **Update `docs/ARCHITECTURE.md`**: If this changes API endpoints, data models, or service interactions, document it *first*.
3. **Update `docs/TODO.md`**: Add a specific checklist for this task under "In Progress".

### Phase 3: Execution (The Reference Loop)

**Cycle**:
1. **Pick Task**: Select the top item from `docs/TODO.md`.
2. **Implement**: Write code in the appropriate service/folder.
   - *Backend*: Use existing crates in `backend/`. Follow Rust idioms.
   - *Frontend*: Use components in `frontend/components/`. Follow Next.js App Router patterns.
3. **Verify**: Ensure it compiles and passes tests (`cargo test`, `npm test`). **Core logic must have unit tests.**
4. **Log**: Update `docs/PROGRESS.md` with what was changed.
5. **Mark Done**: Check off the item in `docs/TODO.md`.

### Phase 4: Quality & Consistency

- **No Magic Numbers**: Use environment variables or config files. **No hardcoded secrets.**
- **Error Handling**: Propagate errors properly (use `Result` in Rust). **No `.unwrap()` or `.expect()` in runtime code.**
- **Security**: Validate all inputs. Check permissions. **Containers must run as non-root.**
- **Logging**: **Use structured JSON logging (`tracing-subscriber` with `json` feature).**
- **Resilience**: **Implement graceful shutdown for all services.**
- **Style**: Match the existing code style (run `cargo fmt`, `npm run lint`).

## Mandatory Documentation

| File | Purpose | Update Trigger |
|------|---------|----------------|
| `docs/PLAN.md` | High-level features & goals | New feature request |
| `docs/ARCHITECTURE.md` | Technical design & API specs | DB schema or API change |
| `docs/TODO.md` | Task tracking | Start/Finish of task |
| `docs/PROGRESS.md` | Work log | Completion of task |

## Anti-Patterns

- Creating new top-level directories without updating `ARCHITECTURE.md`.
- Hardcoding configuration.
- Ignoring existing helper functions in `backend/shared`.
- Writing code that contradicts `docs/PLAN.md`.
- Leaving `TODO.md` outdated while working.
