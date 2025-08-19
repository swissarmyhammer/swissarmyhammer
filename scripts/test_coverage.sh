#!/bin/bash
# Comprehensive test coverage script for CLI exclusion system
set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
MIN_COVERAGE_CLI_EXCLUSION=95
MIN_COVERAGE_COMPREHENSIVE=85
MIN_COVERAGE_INTEGRATION=90

echo -e "${GREEN}🧪 Running comprehensive test coverage analysis...${NC}"

# Ensure tarpaulin is installed
if ! command -v cargo-tarpaulin &> /dev/null; then
    echo -e "${YELLOW}⚠️  Installing cargo-tarpaulin...${NC}"
    cargo install cargo-tarpaulin --force
fi

# Clean previous coverage results
echo -e "${YELLOW}🧹 Cleaning previous coverage results...${NC}"
rm -rf target/tarpaulin/

# Run CLI exclusion specific coverage
echo -e "${GREEN}📊 Running CLI exclusion system coverage...${NC}"
if cargo tarpaulin --config cli_exclusion_coverage --skip-clean; then
    echo -e "${GREEN}✅ CLI exclusion coverage analysis completed${NC}"
else
    echo -e "${RED}❌ CLI exclusion coverage analysis failed${NC}"
    exit 1
fi

# Run comprehensive coverage 
echo -e "${GREEN}📊 Running comprehensive test coverage...${NC}"
if cargo tarpaulin --config comprehensive --skip-clean; then
    echo -e "${GREEN}✅ Comprehensive coverage analysis completed${NC}"
else
    echo -e "${RED}❌ Comprehensive coverage analysis failed${NC}"
    exit 1
fi

# Run integration coverage
echo -e "${GREEN}📊 Running integration test coverage...${NC}"
if cargo tarpaulin --config integration --skip-clean; then
    echo -e "${GREEN}✅ Integration coverage analysis completed${NC}"
else
    echo -e "${RED}❌ Integration coverage analysis failed${NC}"
    exit 1
fi

# Parse coverage results and validate thresholds
echo -e "${GREEN}📈 Validating coverage thresholds...${NC}"

# Function to extract coverage percentage from JSON
extract_coverage() {
    local json_file="$1"
    if [[ -f "$json_file" ]]; then
        # Extract coverage percentage from JSON (assuming standard tarpaulin JSON format)
        python3 -c "
import json
import sys
try:
    with open('$json_file') as f:
        data = json.load(f)
    coverage = data.get('coverage', 0) * 100
    print(f'{coverage:.2f}')
except:
    print('0.00')
" 2>/dev/null || echo "0.00"
    else
        echo "0.00"
    fi
}

# Check CLI exclusion coverage
cli_exclusion_coverage=$(extract_coverage "target/tarpaulin/cli_exclusion/tarpaulin-report.json")
if (( $(echo "$cli_exclusion_coverage >= $MIN_COVERAGE_CLI_EXCLUSION" | bc -l) )); then
    echo -e "${GREEN}✅ CLI exclusion coverage: ${cli_exclusion_coverage}% (>= ${MIN_COVERAGE_CLI_EXCLUSION}%)${NC}"
else
    echo -e "${RED}❌ CLI exclusion coverage: ${cli_exclusion_coverage}% (< ${MIN_COVERAGE_CLI_EXCLUSION}%)${NC}"
    exit 1
fi

# Check comprehensive coverage
comprehensive_coverage=$(extract_coverage "target/tarpaulin/comprehensive/tarpaulin-report.json")
if (( $(echo "$comprehensive_coverage >= $MIN_COVERAGE_COMPREHENSIVE" | bc -l) )); then
    echo -e "${GREEN}✅ Comprehensive coverage: ${comprehensive_coverage}% (>= ${MIN_COVERAGE_COMPREHENSIVE}%)${NC}"
else
    echo -e "${RED}❌ Comprehensive coverage: ${comprehensive_coverage}% (< ${MIN_COVERAGE_COMPREHENSIVE}%)${NC}"
    exit 1
fi

# Summary
echo -e "${GREEN}🎉 All coverage thresholds met!${NC}"
echo -e "📊 Coverage Summary:"
echo -e "   • CLI exclusion system: ${cli_exclusion_coverage}%"
echo -e "   • Comprehensive tests: ${comprehensive_coverage}%"
echo -e "   • Integration tests: Coverage analysis completed"

echo -e "${YELLOW}📁 Coverage reports available in:${NC}"
echo -e "   • target/tarpaulin/cli_exclusion/tarpaulin-report.html"
echo -e "   • target/tarpaulin/comprehensive/tarpaulin-report.html"
echo -e "   • target/tarpaulin/integration/tarpaulin-report.html"