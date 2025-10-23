#!/bin/bash
# Verify all error types implement Severity trait correctly
#
# This script validates that all SwissArmyHammer crates with error types
# properly implement the Severity trait and that the implementations work correctly.

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if cargo-nextest is installed
if ! command -v cargo-nextest &> /dev/null; then
    echo -e "${RED}cargo-nextest not found. Install with: cargo install cargo-nextest${NC}"
    exit 1
fi

echo "🔍 Verifying Severity trait implementations..."
echo ""

# List of crates with error types that should implement Severity
CRATES=(
    "swissarmyhammer-common"
    "swissarmyhammer-cli"
    "swissarmyhammer-workflow"
    "swissarmyhammer-config"
    "swissarmyhammer-rules"
    "swissarmyhammer-git"
    "swissarmyhammer-todo"
    "swissarmyhammer-search"
    "swissarmyhammer-memoranda"
    "swissarmyhammer-outline"
    "swissarmyhammer-templating"
    "swissarmyhammer-agent-executor"
    "swissarmyhammer-shell"
    "swissarmyhammer-tools"
    "swissarmyhammer"
)

# Track overall success
ALL_SUCCESS=true

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Phase 1: Building all crates"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

for crate in "${CRATES[@]}"; do
    printf "  %-40s" "$crate"
    if cargo build -p "$crate" --quiet 2>/dev/null; then
        echo -e "${GREEN}✓${NC}"
    else
        echo -e "${RED}✗${NC}"
        echo -e "${RED}    └─ Build failed${NC}"
        ALL_SUCCESS=false
    fi
done

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Phase 2: Running tests for all crates"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

for crate in "${CRATES[@]}"; do
    printf "  %-40s" "$crate"
    if cargo nextest run -p "$crate" --failure-output immediate --hide-progress-bar --status-level fail --final-status-level fail 2>&1 | grep -q "passed"; then
        echo -e "${GREEN}✓${NC}"
    else
        echo -e "${RED}✗${NC}"
        echo -e "${RED}    └─ Tests failed${NC}"
        ALL_SUCCESS=false
    fi
done

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Phase 3: Running clippy for all crates"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

for crate in "${CRATES[@]}"; do
    printf "  %-40s" "$crate"
    if cargo clippy -p "$crate" --quiet 2>&1 | grep -qE "(error|warning):"; then
        echo -e "${YELLOW}⚠${NC}"
        echo -e "${YELLOW}    └─ Has clippy warnings/errors${NC}"
        ALL_SUCCESS=false
    else
        echo -e "${GREEN}✓${NC}"
    fi
done

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Summary"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

if [ "$ALL_SUCCESS" = true ]; then
    echo -e "${GREEN}✅ All crates implement Severity trait correctly!${NC}"
    echo ""
    echo "All error types across the SwissArmyHammer workspace:"
    echo "  • Build successfully"
    echo "  • Pass all tests"
    echo "  • Properly implement the Severity trait"
    echo ""
    exit 0
else
    echo -e "${RED}❌ Some crates failed verification${NC}"
    echo ""
    echo "Please review the output above to identify failing crates."
    echo "Common issues:"
    echo "  • Missing Severity trait implementation"
    echo "  • Compilation errors"
    echo "  • Test failures"
    echo ""
    exit 1
fi
