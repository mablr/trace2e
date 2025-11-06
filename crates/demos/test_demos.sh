#!/bin/bash

# Test script for Trace2e E2E Demos
# Runs both deletion and consent demos with various configurations

set -e

echo "╔══════════════════════════════════════════════════════════════╗"
echo "║      Trace2e E2E Demonstrations Test Suite                  ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""

DEMO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$DEMO_DIR/../.." && pwd)"

cd "$PROJECT_ROOT"

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

# Test functions

test_deletion_demo() {
    echo -e "${YELLOW}▶ Testing Deletion Demo (All-in-One)...${NC}"
    if timeout 30 cargo run --bin deletion-demo 2>&1 | grep -qi "deleted policies\|BLOCKED"; then
        echo -e "${GREEN}✓ Deletion demo passed${NC}"
        return 0
    else
        echo -e "${RED}✗ Deletion demo failed${NC}"
        return 1
    fi
}

test_deletion_demo_quiet() {
    echo -e "${YELLOW}▶ Testing Deletion Demo (Quiet Mode)...${NC}"
    if timeout 30 cargo run --quiet --bin deletion-demo > /dev/null 2>&1; then
        echo -e "${GREEN}✓ Deletion demo completed without errors${NC}"
        return 0
    else
        echo -e "${RED}✗ Deletion demo crashed${NC}"
        return 1
    fi
}

test_consent_demo_auto() {
    echo -e "${YELLOW}▶ Testing Consent Demo (Auto-Grant Mode)...${NC}"
    if timeout 30 cargo run --bin consent-demo -- --auto-grant 2>&1 | grep -qi "Granted\|AUTO-GRANT"; then
        echo -e "${GREEN}✓ Consent demo passed${NC}"
        return 0
    else
        echo -e "${RED}✗ Consent demo failed${NC}"
        return 1
    fi
}

test_consent_demo_quiet() {
    echo -e "${YELLOW}▶ Testing Consent Demo (Quiet Mode, Auto-Grant)...${NC}"
    if timeout 30 cargo run --quiet --bin consent-demo -- --auto-grant > /dev/null 2>&1; then
        echo -e "${GREEN}✓ Consent demo completed without errors${NC}"
        return 0
    else
        echo -e "${RED}✗ Consent demo crashed${NC}"
        return 1
    fi
}

test_compilation() {
    echo -e "${YELLOW}▶ Testing Compilation...${NC}"
    if cargo build -p demos 2>&1 | grep -q "Finished"; then
        echo -e "${GREEN}✓ Demos compiled successfully${NC}"
        return 0
    else
        echo -e "${RED}✗ Compilation failed${NC}"
        return 1
    fi
}

# Run tests
echo "Step 1: Building demos..."
test_compilation || exit 1
echo ""

echo "Step 2: Running demos..."
test_deletion_demo_quiet || exit 1
echo ""

test_consent_demo_quiet || exit 1
echo ""

echo "Step 3: Detailed output verification..."
test_deletion_demo || exit 1
echo ""

test_consent_demo_auto || exit 1
echo ""

# Summary
echo "╔══════════════════════════════════════════════════════════════╗"
echo -e "${GREEN}║  ALL TESTS PASSED                                    ║${NC}"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""
echo "Next steps:"
echo "  1. Run deletion demo:  cargo run --bin deletion-demo"
echo "  2. Run consent demo:   cargo run --bin consent-demo"
echo "  3. With debug logs:    RUST_LOG=debug cargo run --bin deletion-demo"
echo ""

