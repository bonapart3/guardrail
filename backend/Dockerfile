# GuardRail Backend Services Dockerfile
# Multi-stage build for Rust services

# Stage 1: Build
FROM rust:1.75-slim-bookworm AS builder

WORKDIR /app

# Install dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY shared ./shared
COPY api-gateway ./api-gateway
COPY identity-service ./identity-service
COPY policy-engine ./policy-engine
COPY movement-ledger ./movement-ledger
COPY chain-anchor ./chain-anchor

# Build all services
RUN cargo build --release

# Stage 2: Runtime
FROM debian:bookworm-slim AS runtime

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Copy built binaries
COPY --from=builder /app/target/release/api-gateway /app/
COPY --from=builder /app/target/release/identity-service /app/
COPY --from=builder /app/target/release/policy-engine /app/
COPY --from=builder /app/target/release/movement-ledger /app/
COPY --from=builder /app/target/release/chain-anchor /app/

# Copy policy files
COPY policy-engine/policies /app/policies

# Default to api-gateway
CMD ["/app/api-gateway"]
