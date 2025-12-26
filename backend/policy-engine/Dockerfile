# Policy Engine Dockerfile
FROM rust:1.75-slim-bookworm AS builder
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
COPY . .
RUN cargo build --release --bin policy-engine

FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*

# Create a non-privileged user
RUN useradd -m -u 1000 -U -s /bin/sh -d /app guardrail

COPY --from=builder /app/target/release/policy-engine /app/
COPY --from=builder /app/policy-engine/policies /app/policies

# Change ownership of the application binary and policies
RUN chown -R guardrail:guardrail /app

# Switch to the non-privileged user
USER guardrail

EXPOSE 3002
CMD ["/app/policy-engine"]
