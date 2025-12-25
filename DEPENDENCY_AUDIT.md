# Dependency Audit Report

**Date:** December 25, 2025
**Project:** GuardRail Compliance Platform

---

## Executive Summary

This audit identified **21 security vulnerabilities** (5 critical, 7 high, 5 moderate, 4 high/critical in frontend), **1 highly suspicious package**, and several outdated dependencies across the project's JavaScript/TypeScript, Rust, and Python components.

### Priority Actions Required

| Priority | Issue | Impact |
|----------|-------|--------|
| **CRITICAL** | Remove `psql` package from root | Security vulnerabilities, wrong package |
| **CRITICAL** | Upgrade `next` to v14.2.35+ | 12 known CVEs including auth bypass |
| **HIGH** | Upgrade frontend dependencies | Multiple outdated packages |
| **MEDIUM** | Update Rust workspace dependencies | Version inconsistencies |
| **LOW** | Python SDK dependencies are current | No action needed |

---

## 1. Root package.json - CRITICAL ISSUE

### Current State
```json
{
  "dependencies": {
    "psql": "^0.0.1"
  }
}
```

### Problem
The `psql` npm package is **NOT a PostgreSQL client**. It's actually "Parallel MySQL queries" - a MySQL tool with:
- Only 1 version ever published (0.0.1)
- Last updated over a year ago
- **17 security vulnerabilities** in its dependency tree:
  - **5 Critical**: `form-data`, `js-yaml`, `underscore`
  - **7 High**: `hawk`, `hoek`, `mime`, `qs`
  - **5 Moderate**: `tunnel-agent`, `underscore.string`

### Recommendation
**Remove this package entirely.** If PostgreSQL client functionality is needed:
- For Node.js backend: Use `pg` (node-postgres) or `postgres` (Postgres.js)
- Current architecture uses Rust with `sqlx` for database access, so this dependency appears unnecessary

### Action
```bash
# Remove the suspicious package
rm package.json package-lock.json
# Or if root package.json is needed for other purposes:
# Edit to remove psql dependency
```

---

## 2. Frontend Dependencies (frontend/package.json)

### Critical Security Vulnerabilities

| Package | Current | Fixed Version | Severity | CVEs |
|---------|---------|---------------|----------|------|
| `next` | 14.2.0 | 14.2.35+ | **Critical** | 12 vulnerabilities including auth bypass (GHSA-f82v-jwr5-mffw), SSRF, cache poisoning |
| `eslint-config-next` | 14.2.0 | 14.2.35+ | High | Command injection via glob |

### Outdated Packages

| Package | Current | Latest | Breaking? | Recommendation |
|---------|---------|--------|-----------|----------------|
| `next` | 14.2.0 | 16.1.1 | Yes | Update to 14.2.35+ (patch), then plan v15 migration |
| `react` | 18.3.1 | 19.2.3 | Yes | Keep 18.x until Next.js 15 migration |
| `react-dom` | 18.3.1 | 19.2.3 | Yes | Keep 18.x until Next.js 15 migration |
| `zustand` | 4.5.2 | 5.0.9 | Yes | Update to 4.5.7, plan v5 migration |
| `zod` | 3.22.4 | 4.2.1 | Yes | Update to 3.25.76, plan v4 migration |
| `tailwind-merge` | 2.2.2 | 3.4.0 | Yes | Update to 2.6.0, plan v3 migration |
| `recharts` | 2.12.3 | 3.6.0 | Yes | Update to 2.15.4, plan v3 migration |
| `date-fns` | 3.6.0 | 4.1.0 | Yes | Keep 3.x for now |
| `next-themes` | 0.3.0 | 0.4.6 | Minor | Update to 0.4.6 |
| `lucide-react` | 0.363.0 | 0.562.0 | Minor | Update to latest |
| `@hookform/resolvers` | 3.3.4 | 5.2.2 | Yes | Update to 3.10.0, plan v5 migration |

### Recommended package.json Updates

```json
{
  "dependencies": {
    "next": "14.2.35",
    "react": "18.3.1",
    "react-dom": "18.3.1",
    "@radix-ui/react-avatar": "^1.1.11",
    "@radix-ui/react-dialog": "^1.1.15",
    "@radix-ui/react-dropdown-menu": "^2.1.16",
    "@radix-ui/react-icons": "^1.3.2",
    "@radix-ui/react-label": "^2.1.8",
    "@radix-ui/react-navigation-menu": "^1.2.14",
    "@radix-ui/react-popover": "^1.1.15",
    "@radix-ui/react-select": "^2.2.6",
    "@radix-ui/react-separator": "^1.1.8",
    "@radix-ui/react-slot": "^1.2.4",
    "@radix-ui/react-tabs": "^1.1.13",
    "@radix-ui/react-toast": "^1.2.15",
    "@radix-ui/react-tooltip": "^1.2.8",
    "@tanstack/react-query": "^5.90.12",
    "@tanstack/react-table": "^8.21.3",
    "class-variance-authority": "^0.7.1",
    "clsx": "^2.1.1",
    "cmdk": "^1.1.1",
    "date-fns": "^3.6.0",
    "lucide-react": "^0.562.0",
    "next-auth": "^4.24.13",
    "next-themes": "^0.4.6",
    "react-hook-form": "^7.69.0",
    "recharts": "^2.15.4",
    "tailwind-merge": "^2.6.0",
    "tailwindcss-animate": "^1.0.7",
    "zustand": "^4.5.7",
    "zod": "^3.25.76",
    "@hookform/resolvers": "^3.10.0",
    "@monaco-editor/react": "^4.7.0"
  },
  "devDependencies": {
    "@types/node": "^20.17.0",
    "@types/react": "^18.3.0",
    "@types/react-dom": "^18.3.0",
    "autoprefixer": "^10.4.20",
    "eslint": "^8.57.0",
    "eslint-config-next": "14.2.35",
    "postcss": "^8.4.49",
    "tailwindcss": "^3.4.17",
    "typescript": "^5.7.0"
  }
}
```

---

## 3. Rust Backend Dependencies

### Workspace Configuration Issues

There are **two workspace Cargo.toml files** with conflicting versions:

| Dependency | Root Cargo.toml | backend/Cargo.toml | Recommendation |
|------------|-----------------|---------------------|----------------|
| `sqlx` | 0.7 | 0.8 | Standardize on 0.8 |
| `tokio` | 1.35 | 1 | Use 1.35+ |
| `axum` | 0.7 (with macros) | 0.7 | Consistent |

### Outdated Crates

| Crate | Current | Latest | Notes |
|-------|---------|--------|-------|
| `solana-sdk` | 1.18 | 2.x | Major version available |
| `solana-client` | 1.18 | 2.x | Major version available |
| `anchor-client` | 0.29 | 0.30+ | Minor updates available |
| `reqwest` (orchestrator) | 0.11 | 0.12 | Used 0.12 elsewhere |

### Dependency Bloat Analysis

The `ethers` crate (Ethereum support) adds significant compile-time and binary size. Consider:
- If only Solana is supported initially, defer `ethers` dependency
- Use feature flags to make blockchain support optional

### Recommended Workspace Consolidation

Remove `backend/Cargo.toml` as a separate workspace and ensure all crates use the root workspace:

```toml
# Root Cargo.toml - update these versions
[workspace.dependencies]
sqlx = { version = "0.8", features = ["runtime-tokio", "postgres", "uuid", "chrono", "json"] }
tokio = { version = "1.42", features = ["full"] }
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
```

---

## 4. Solana Smart Contract

### Current State
```toml
[dependencies]
anchor-lang = "0.29.0"
```

### Recommendation
- `anchor-lang` 0.29.0 is reasonably current
- Consider updating to 0.30.x when stable for your use case
- Ensure Solana toolchain matches SDK versions

---

## 5. TypeScript SDK (sdk/typescript)

### Current State
Dependencies are dev-only and reasonably current:
- `typescript`: ^5.0.0 (latest is 5.7.x)
- `jest`: ^29.0.0 (current major)

### Missing Production Dependencies
The SDK has no runtime dependencies. If this SDK makes HTTP calls, consider adding:
```json
{
  "dependencies": {
    "undici": "^6.0.0"
  }
}
```

---

## 6. Python SDK (sdk/python)

### Current State
```toml
dependencies = [
    "httpx>=0.25.0",
]
```

### Assessment
- `httpx` is the correct modern choice for async HTTP
- Python version requirements (>=3.9) are appropriate
- Dev dependencies are current

### No Action Required

---

## 7. Unnecessary Bloat

### Identified Issues

1. **Root package.json `psql` dependency**
   - Brings in 10 transitive dependencies
   - Not used by any part of the application
   - **Remove entirely**

2. **Duplicate workspace configuration**
   - Both `/Cargo.toml` and `/backend/Cargo.toml` define workspaces
   - Creates confusion and version drift
   - **Consolidate to single workspace**

3. **`ethers` crate** (if Solana-only initially)
   - Large dependency tree
   - Consider making it a feature flag

4. **Potential unused Radix components**
   - 14 separate Radix UI imports
   - Audit actual usage and remove unused components

---

## 8. Action Plan

### Immediate (Security Critical)

```bash
# 1. Remove root package.json or fix it
cd /home/user/guardrail
rm package.json package-lock.json

# 2. Update frontend for security patches
cd frontend
npm install next@14.2.35 eslint-config-next@14.2.35
```

### Short-term (Within 1 week)

1. Update all Radix UI packages to latest minor versions
2. Update utility libraries (clsx, tailwind-merge, etc.)
3. Consolidate Rust workspace configuration
4. Standardize `sqlx` version across workspace

### Medium-term (Within 1 month)

1. Plan Next.js 15 migration (breaking changes with React 19)
2. Evaluate `zustand` v5 migration
3. Update Solana SDK to 2.x when ready
4. Implement feature flags for blockchain support

---

## 9. Security Vulnerability Summary

### Total Vulnerabilities Found

| Severity | Root package.json | Frontend | Total |
|----------|-------------------|----------|-------|
| Critical | 5 | 1 | **6** |
| High | 7 | 3 | **10** |
| Moderate | 5 | 0 | **5** |
| **Total** | **17** | **4** | **21** |

### CVE References (Frontend - Critical)

- **GHSA-f82v-jwr5-mffw**: Authorization Bypass in Next.js Middleware
- **GHSA-4342-x723-ch2f**: Next.js Improper Middleware Redirect (SSRF)
- **GHSA-gp8f-8m3g-qvj9**: Next.js Cache Poisoning
- **GHSA-7gfc-8cq8-jh5f**: Next.js authorization bypass vulnerability

---

## 10. Compliance Notes

For a compliance platform handling sensitive financial data:

1. **Dependency Provenance**: Consider using npm's `--before` flag or lock to specific SHAs
2. **SBOM Generation**: Implement Software Bill of Materials for audit trails
3. **Automated Scanning**: Add `npm audit` and `cargo audit` to CI/CD pipeline
4. **Dependabot/Renovate**: Enable automated dependency update PRs

---

*Generated by dependency audit on December 25, 2025*
