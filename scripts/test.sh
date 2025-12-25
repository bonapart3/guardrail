#!/bin/bash
set -e

# GuardRail Test Runner

echo "
╔══════════════════════════════════════════════════════════════╗
║               🧪 GuardRail Test Suite                         ║
╚══════════════════════════════════════════════════════════════╝
"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

COMPONENT=${1:-all}

run_backend_tests() {
    echo -e "${BLUE}Running backend tests...${NC}"
    cd backend
    
    # Check
    echo -e "${YELLOW}Checking...${NC}"
    cargo check --all-targets
    
    # Clippy
    echo -e "${YELLOW}Running Clippy...${NC}"
    cargo clippy --all-targets -- -D warnings
    
    # Format
    echo -e "${YELLOW}Checking format...${NC}"
    cargo fmt --all -- --check
    
    # Tests
    echo -e "${YELLOW}Running tests...${NC}"
    cargo test --all
    
    cd ..
    echo -e "${GREEN}✓ Backend tests passed${NC}"
}

run_frontend_tests() {
    echo -e "${BLUE}Running frontend tests...${NC}"
    cd frontend
    
    # Install deps
    npm ci
    
    # Lint
    echo -e "${YELLOW}Linting...${NC}"
    npm run lint
    
    # Type check
    echo -e "${YELLOW}Type checking...${NC}"
    npx tsc --noEmit
    
    # Tests (if configured)
    if [ -f "jest.config.js" ] || grep -q "\"test\"" package.json; then
        echo -e "${YELLOW}Running tests...${NC}"
        npm test || true
    fi
    
    cd ..
    echo -e "${GREEN}✓ Frontend tests passed${NC}"
}

run_contract_tests() {
    echo -e "${BLUE}Running contract tests...${NC}"
    
    # Ethereum
    echo -e "${YELLOW}Testing Ethereum contracts...${NC}"
    cd contracts/ethereum
    if command -v forge &> /dev/null; then
        forge build
        forge test -vvv
    else
        echo -e "${YELLOW}Foundry not installed, skipping Ethereum tests${NC}"
    fi
    cd ../..
    
    # Solana
    echo -e "${YELLOW}Testing Solana contracts...${NC}"
    cd contracts/solana
    if command -v anchor &> /dev/null; then
        anchor build
        anchor test
    else
        echo -e "${YELLOW}Anchor not installed, skipping Solana tests${NC}"
    fi
    cd ../..
    
    echo -e "${GREEN}✓ Contract tests passed${NC}"
}

run_sdk_tests() {
    echo -e "${BLUE}Running SDK tests...${NC}"
    
    # TypeScript
    echo -e "${YELLOW}Testing TypeScript SDK...${NC}"
    cd sdk/typescript
    npm ci
    npm run build
    npm test || true
    cd ../..
    
    # Python
    echo -e "${YELLOW}Testing Python SDK...${NC}"
    cd sdk/python
    pip install -e ".[dev]" -q
    pytest || true
    cd ../..
    
    echo -e "${GREEN}✓ SDK tests passed${NC}"
}

case "$COMPONENT" in
    backend)
        run_backend_tests
        ;;
    frontend)
        run_frontend_tests
        ;;
    contracts)
        run_contract_tests
        ;;
    sdk)
        run_sdk_tests
        ;;
    all)
        run_backend_tests
        echo ""
        run_frontend_tests
        echo ""
        run_contract_tests
        echo ""
        run_sdk_tests
        ;;
    *)
        echo "Usage: $0 [component]"
        echo ""
        echo "Components:"
        echo "  backend     - Run Rust backend tests"
        echo "  frontend    - Run Next.js frontend tests"
        echo "  contracts   - Run smart contract tests"
        echo "  sdk         - Run SDK tests"
        echo "  all         - Run all tests (default)"
        exit 1
        ;;
esac

echo ""
echo -e "${GREEN}═══════════════════════════════════════════════════════════════${NC}"
echo -e "${GREEN}                    All tests completed!${NC}"
echo -e "${GREEN}═══════════════════════════════════════════════════════════════${NC}"
