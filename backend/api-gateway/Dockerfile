# API Gateway Dockerfile
FROM rust:1.75-slim-bookworm AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY shared ./shared
COPY api-gateway ./api-gateway

# Create dummy files for other services
RUN mkdir -p identity-service/src policy-engine/src movement-ledger/src chain-anchor/src && \
    echo "fn main() {}" > identity-service/src/main.rs && \
    echo "fn main() {}" > policy-engine/src/main.rs && \
    echo "fn main() {}" > movement-ledger/src/main.rs && \
    echo "fn main() {}" > chain-anchor/src/main.rs && \
    cp api-gateway/Cargo.toml identity-service/ && \
    cp api-gateway/Cargo.toml policy-engine/ && \
    cp api-gateway/Cargo.toml movement-ledger/ && \
    cp api-gateway/Cargo.toml chain-anchor/

RUN cargo build --release --bin api-gateway

FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*

# Create a non-privileged user
RUN useradd -m -u 1000 -U -s /bin/sh -d /app guardrail

COPY --from=builder /app/target/release/api-gateway /app/

# Change ownership of the application binary
RUN chown -R guardrail:guardrail /app

# Switch to the non-privileged user
USER guardrail

EXPOSE 3000
CMD ["/app/api-gateway"]
