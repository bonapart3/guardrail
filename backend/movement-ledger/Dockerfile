# Movement Ledger Dockerfile
FROM rust:1.80-slim-bookworm AS builder
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY shared ./shared
COPY identity-service ./identity-service
COPY api-gateway ./api-gateway
COPY policy-engine ./policy-engine
COPY movement-ledger ./movement-ledger
COPY chain-anchor ./chain-anchor

# Build only movement-ledger
RUN cargo build --release --bin movement-ledger

FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*

# Create a non-privileged user
RUN useradd -m -u 1000 -U -s /bin/sh -d /app guardrail

COPY --from=builder /app/target/release/movement-ledger /app/

# Change ownership of the application binary
RUN chown -R guardrail:guardrail /app

# Switch to the non-privileged user
USER guardrail

EXPOSE 3003
CMD ["/app/movement-ledger"]
