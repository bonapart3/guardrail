#!/bin/bash
set -e

# GuardRail Quick Start Script
# Starts all services for local development

echo "
╔══════════════════════════════════════════════════════════════╗
║                    🛡️  GuardRail                              ║
║           Compliance & Risk Control Platform                  ║
╚══════════════════════════════════════════════════════════════╝
"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Check prerequisites
check_prereqs() {
    echo -e "${BLUE}Checking prerequisites...${NC}"
    
    if ! command -v docker &> /dev/null; then
        echo -e "${RED}✗ Docker not found. Please install Docker.${NC}"
        exit 1
    fi
    echo -e "${GREEN}✓ Docker${NC}"
    
    if ! command -v docker-compose &> /dev/null && ! docker compose version &> /dev/null; then
        echo -e "${RED}✗ Docker Compose not found. Please install Docker Compose.${NC}"
        exit 1
    fi
    echo -e "${GREEN}✓ Docker Compose${NC}"
    
    if ! command -v cargo &> /dev/null; then
        echo -e "${YELLOW}⚠ Rust not found. Backend will run in Docker only.${NC}"
    else
        echo -e "${GREEN}✓ Rust/Cargo${NC}"
    fi
    
    if ! command -v node &> /dev/null; then
        echo -e "${YELLOW}⚠ Node.js not found. Frontend will run in Docker only.${NC}"
    else
        echo -e "${GREEN}✓ Node.js $(node -v)${NC}"
    fi
    
    echo ""
}

# Setup environment
setup_env() {
    if [ ! -f .env ]; then
        echo -e "${BLUE}Creating .env from template...${NC}"
        cp .env.example .env
        echo -e "${GREEN}✓ Created .env file${NC}"
    else
        echo -e "${GREEN}✓ .env file exists${NC}"
    fi
    echo ""
}

# Start infrastructure
start_infra() {
    echo -e "${BLUE}Starting infrastructure (PostgreSQL, Redis)...${NC}"
    docker compose up -d postgres redis
    
    # Wait for PostgreSQL to be ready
    echo -e "${BLUE}Waiting for PostgreSQL...${NC}"
    until docker compose exec -T postgres pg_isready -U postgres > /dev/null 2>&1; do
        sleep 1
    done
    echo -e "${GREEN}✓ PostgreSQL ready${NC}"
    
    # Wait for Redis to be ready
    echo -e "${BLUE}Waiting for Redis...${NC}"
    until docker compose exec -T redis redis-cli ping > /dev/null 2>&1; do
        sleep 1
    done
    echo -e "${GREEN}✓ Redis ready${NC}"
    echo ""
}

# Initialize database
init_db() {
    echo -e "${BLUE}Initializing database...${NC}"
    docker compose exec -T postgres psql -U postgres -d guardrail -f /docker-entrypoint-initdb.d/init.sql > /dev/null 2>&1 || true
    echo -e "${GREEN}✓ Database initialized${NC}"
    echo ""
}

# Start backend services
start_backend() {
    echo -e "${BLUE}Starting backend services...${NC}"
    
    if [ "$1" == "--docker" ]; then
        docker compose up -d identity-service policy-engine movement-ledger chain-anchor api-gateway
    else
        # Build and run with cargo
        cd backend
        cargo build --release 2>&1 | tail -5
        
        # Start services in background
        ./target/release/identity-service &
        ./target/release/policy-engine &
        ./target/release/movement-ledger &
        ./target/release/chain-anchor &
        ./target/release/api-gateway &
        
        cd ..
    fi
    
    echo -e "${GREEN}✓ Backend services started${NC}"
    echo ""
}

# Start frontend
start_frontend() {
    echo -e "${BLUE}Starting frontend...${NC}"
    
    if [ "$1" == "--docker" ]; then
        docker compose up -d frontend
    else
        cd frontend
        npm install > /dev/null 2>&1
        npm run dev &
        cd ..
    fi
    
    echo -e "${GREEN}✓ Frontend started${NC}"
    echo ""
}

# Show status
show_status() {
    echo -e "
${GREEN}═══════════════════════════════════════════════════════════════${NC}
${GREEN}                    GuardRail is running!${NC}
${GREEN}═══════════════════════════════════════════════════════════════${NC}

${BLUE}Console:${NC}        http://localhost:3005
${BLUE}API Gateway:${NC}    http://localhost:3000
${BLUE}API Docs:${NC}       http://localhost:3000/docs

${BLUE}Backend Services:${NC}
  • Identity:     http://localhost:3001/health
  • Policy:       http://localhost:3002/health
  • Ledger:       http://localhost:3003/health
  • Anchor:       http://localhost:3004/health

${BLUE}Infrastructure:${NC}
  • PostgreSQL:   localhost:5432
  • Redis:        localhost:6379

${YELLOW}Default Login:${NC}
  Email:    admin@guardrail.dev
  Password: admin123

${BLUE}Commands:${NC}
  ./scripts/start.sh          Start all services
  ./scripts/start.sh --docker Start with Docker
  ./scripts/stop.sh           Stop all services
  ./scripts/logs.sh           View logs
  ./scripts/test.sh           Run tests

${GREEN}═══════════════════════════════════════════════════════════════${NC}
"
}

# Main
main() {
    check_prereqs
    setup_env
    start_infra
    init_db
    
    if [ "$1" == "--docker" ]; then
        start_backend --docker
        start_frontend --docker
    else
        start_backend
        start_frontend
    fi
    
    show_status
}

# Handle arguments
case "$1" in
    --help|-h)
        echo "Usage: $0 [--docker]"
        echo ""
        echo "Options:"
        echo "  --docker    Run all services in Docker containers"
        echo "  --help      Show this help message"
        exit 0
        ;;
    *)
        main "$@"
        ;;
esac
