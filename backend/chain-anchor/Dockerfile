# Chain Anchor Dockerfile
FROM rust:1.75-slim-bookworm AS builder
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
COPY . .
RUN cargo build --release --bin chain-anchor

FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*

# Create a non-privileged user
RUN useradd -m -u 1000 -U -s /bin/sh -d /app guardrail

COPY --from=builder /app/target/release/chain-anchor /app/

# Change ownership of the application binary
RUN chown -R guardrail:guardrail /app

# Switch to the non-privileged user
USER guardrail

EXPOSE 3004
CMD ["/app/chain-anchor"]
