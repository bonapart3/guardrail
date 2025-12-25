#!/bin/bash
set -e

echo "Stopping GuardRail services..."

# Stop Docker services
docker compose down

# Kill any local processes
pkill -f "identity-service" 2>/dev/null || true
pkill -f "policy-engine" 2>/dev/null || true
pkill -f "movement-ledger" 2>/dev/null || true
pkill -f "chain-anchor" 2>/dev/null || true
pkill -f "api-gateway" 2>/dev/null || true
pkill -f "next-server" 2>/dev/null || true

echo "✓ All services stopped"
