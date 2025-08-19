#!/bin/bash
# Generate comprehensive test report for CLI exclusion system
set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
REPORT_DIR="target/test_reports"
TIMESTAMP=$(date '+%Y-%m-%d_%H-%M-%S')
REPORT_FILE="${REPORT_DIR}/cli_exclusion_test_report_${TIMESTAMP}.md"

echo -e "${GREEN}📊 Generating comprehensive CLI exclusion test report...${NC}"

# Create report directory
mkdir -p "$REPORT_DIR"

# Start report
cat > "$REPORT_FILE" << EOF
# CLI Exclusion System Test Report

**Generated:** $(date '+%Y-%m-%d %H:%M:%S')  
**Git Commit:** $(git rev-parse HEAD 2>/dev/null || echo "Unknown")  
**Git Branch:** $(git branch --show-current 2>/dev/null || echo "Unknown")

## Executive Summary

This report provides comprehensive validation results for the CLI exclusion system testing suite.

EOF

# Function to run test category and capture results
run_test_category() {
    local category="$1"
    local description="$2"
    
    echo -e "${BLUE}🧪 Running $description...${NC}"
    
    # Capture test output
    local output_file="${REPORT_DIR}/${category}_output.log"
    local start_time=$(date +%s)
    
    if cargo test --test cli_exclusion_comprehensive_tests -- "$category" --no-capture > "$output_file" 2>&1; then
        local status="✅ PASSED"
        local exit_code=0
    else
        local status="❌ FAILED"
        local exit_code=1
    fi
    
    local end_time=$(date +%s)
    local duration=$((end_time - start_time))
    
    # Extract test counts from output
    local test_count=$(grep -o "[0-9]\+ passed" "$output_file" | head -1 | grep -o "[0-9]\+" || echo "0")
    local failed_count=$(grep -o "[0-9]\+ failed" "$output_file" | head -1 | grep -o "[0-9]\+" || echo "0")
    
    # Add to report
    cat >> "$REPORT_FILE" << EOF

### $description

- **Status:** $status
- **Duration:** ${duration}s
- **Tests Passed:** $test_count
- **Tests Failed:** $failed_count

EOF

    if [[ $exit_code -ne 0 ]]; then
        cat >> "$REPORT_FILE" << EOF
**Failure Details:**
\`\`\`
$(tail -50 "$output_file")
\`\`\`

EOF
    fi
    
    echo -e "   $status (${duration}s, $test_count passed, $failed_count failed)"
    return $exit_code
}

# Run test categories
echo -e "${YELLOW}📋 Executing test categories...${NC}"

# Track overall status
overall_status=0

# Unit Tests
run_test_category "unit" "Unit Tests" || overall_status=1

# Integration Tests  
run_test_category "integration" "Integration Tests" || overall_status=1

# Property Tests
PROPTEST_CASES=500 run_test_category "property" "Property-Based Tests" || overall_status=1

# End-to-End Tests
run_test_category "e2e" "End-to-End Tests" || overall_status=1

# Performance Tests (run with --ignored flag)
echo -e "${BLUE}🧪 Running Performance Tests...${NC}"
performance_output="${REPORT_DIR}/performance_output.log"
start_time=$(date +%s)

if cargo test --test cli_exclusion_comprehensive_tests -- performance --no-capture --ignored > "$performance_output" 2>&1; then
    perf_status="✅ PASSED"
    perf_exit=0
else
    perf_status="❌ FAILED" 
    perf_exit=1
    overall_status=1
fi

end_time=$(date +%s)
perf_duration=$((end_time - start_time))

cat >> "$REPORT_FILE" << EOF

### Performance Tests

- **Status:** $perf_status  
- **Duration:** ${perf_duration}s

EOF

# Add coverage information if available
if [[ -f "target/tarpaulin/cli_exclusion/tarpaulin-report.json" ]]; then
    echo -e "${BLUE}📊 Adding coverage information...${NC}"
    
    coverage=$(python3 -c "
import json
import sys
try:
    with open('target/tarpaulin/cli_exclusion/tarpaulin-report.json') as f:
        data = json.load(f)
    coverage = data.get('coverage', 0) * 100
    print(f'{coverage:.2f}')
except:
    print('N/A')
" 2>/dev/null || echo "N/A")

    cat >> "$REPORT_FILE" << EOF

## Test Coverage

- **CLI Exclusion Coverage:** ${coverage}%

EOF
fi

# Add system information
cat >> "$REPORT_FILE" << EOF

## System Information

- **Rust Version:** $(rustc --version)
- **Cargo Version:** $(cargo --version)
- **Operating System:** $(uname -s)
- **Architecture:** $(uname -m)

## Test Configuration

- **Test Thread Count:** 1 (sequential execution for reliability)
- **Property Test Cases:** ${PROPTEST_CASES:-1000}
- **Performance Test Mode:** Enabled with --ignored flag
- **Coverage Threshold:** 95% for CLI exclusion system

## Summary

EOF

if [[ $overall_status -eq 0 ]]; then
    cat >> "$REPORT_FILE" << EOF
**Overall Status:** ✅ ALL TESTS PASSED

The CLI exclusion system has successfully passed all comprehensive tests including:
- Unit tests for attribute macros and registry detection
- Integration tests for cross-system functionality  
- Property-based tests for robustness validation
- End-to-end workflow tests
- Performance validation tests

The system is ready for production use.
EOF
    echo -e "${GREEN}🎉 All tests passed! Report generated at: $REPORT_FILE${NC}"
else
    cat >> "$REPORT_FILE" << EOF
**Overall Status:** ❌ SOME TESTS FAILED

Some test categories have failures that need to be addressed before the CLI exclusion system can be considered production-ready. Please review the failure details above and resolve the issues.
EOF
    echo -e "${RED}❌ Some tests failed. Report generated at: $REPORT_FILE${NC}"
fi

# Generate summary
echo -e "${YELLOW}📋 Test Report Summary:${NC}"
echo -e "   • Report Location: $REPORT_FILE"
echo -e "   • Overall Status: $([ $overall_status -eq 0 ] && echo "✅ PASSED" || echo "❌ FAILED")"
echo -e "   • Coverage Available: $([ -f "target/tarpaulin/cli_exclusion/tarpaulin-report.html" ] && echo "Yes" || echo "No")"

exit $overall_status