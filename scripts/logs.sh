#!/bin/bash

# GuardRail Log Viewer

SERVICE=${1:-all}

case "$SERVICE" in
    api|gateway)
        docker compose logs -f api-gateway
        ;;
    identity)
        docker compose logs -f identity-service
        ;;
    policy)
        docker compose logs -f policy-engine
        ;;
    ledger|movement)
        docker compose logs -f movement-ledger
        ;;
    anchor|chain)
        docker compose logs -f chain-anchor
        ;;
    frontend|console)
        docker compose logs -f frontend
        ;;
    postgres|db)
        docker compose logs -f postgres
        ;;
    redis)
        docker compose logs -f redis
        ;;
    all)
        docker compose logs -f
        ;;
    *)
        echo "Usage: $0 [service]"
        echo ""
        echo "Services:"
        echo "  api, gateway    - API Gateway"
        echo "  identity        - Identity Service"
        echo "  policy          - Policy Engine"
        echo "  ledger          - Movement Ledger"
        echo "  anchor          - Chain Anchor"
        echo "  frontend        - Console Frontend"
        echo "  postgres, db    - PostgreSQL"
        echo "  redis           - Redis"
        echo "  all             - All services (default)"
        exit 1
        ;;
esac
