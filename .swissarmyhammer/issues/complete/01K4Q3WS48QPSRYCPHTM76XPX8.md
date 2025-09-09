# Remove ALL Performance Tests from Unit Test Framework

## Problem
The codebase contains **66+ performance tests** running under the unit test framework (`cargo nextest`), violating our coding standards. According to `builtin/prompts/coding_standards.md.liquid`:

> "YOU MUST NEVER create performance tests or benchmarks unless explicitly asked by the user"
> "YOU MUST NEVER write performance tests while doing TDD, only write performance tests if explicitly requested"

Performance tests should be in dedicated benchmark suites (`cargo bench`), not in unit tests that run with `cargo nextest`.

## Complete Enumeration of Performance Tests

### **swissarmyhammer-tools** (15+ performance tests)
- `tests/file_tools_integration_tests.rs`:
  - `test_large_file_read_performance()` - Line 3241
  - `test_large_file_write_performance()` - Line 3322  
  - `test_large_file_edit_performance()` - Line 3372
- `tests/file_tools_performance_tests.rs`:
  - `test_read_tool_large_file_performance()` - Line 275
  - `test_read_tool_offset_limit_performance()` - Line 326
  - `test_write_tool_large_content_performance()` - Line 371
  - `test_edit_tool_large_file_performance()` - Line 478
  - `test_glob_tool_large_directory_performance()` - Line 532
  - `test_glob_tool_complex_patterns_performance()` - Line 578
  - `test_grep_tool_large_content_performance()` - Line 625
  - `test_grep_tool_complex_regex_performance()` - Line 685
  - `test_cross_tool_workflow_performance()` - Line 755
- `tests/notify_integration_tests.rs`:
  - `test_notify_tool_performance_characteristics()` - Line 306
- `tests/web_fetch_specification_compliance.rs`:
  - `test_performance_requirements()` - Line 234
- `tests/test_issue_show_enhanced.rs`:
  - `test_issue_show_performance_with_many_issues()` - Line 824
- `src/mcp/tools/shell/execute/mod.rs`:
  - `test_large_output_handling_performance()` - Line 3570
  - `test_concurrent_shell_execution_performance()` - Line 3600
  - `test_timeout_handling_performance()` - Line 3732

### **swissarmyhammer-shell** (3+ performance tests)
- `src/performance.rs`:
  - `test_performance_metrics_creation()` - Line 479
  - `test_performance_statistics()` - Line 515
  - `test_performance_targets()` - Line 558

### **swissarmyhammer-workflow** (2+ performance tests)
- `src/actions_tests/shell_action_integration_tests.rs`:
  - `test_shell_action_performance_with_sequential_execution()` - Line 247
- `src/executor/tests.rs`:
  - `test_abort_file_performance_impact()` - Line 1783

### **swissarmyhammer-issues** (1 performance test)
- `src/metrics.rs`:
  - `test_mixed_operation_performance_analysis()` - Line 556

### **swissarmyhammer-cli** (4+ performance tests)  
- `tests/abort_final_integration_tests.rs`:
  - `test_abort_performance_impact_baseline()` - Line 87
  - `test_abort_performance_with_checking_overhead()` - Line 126
- `tests/comprehensive_cli_mcp_integration_tests.rs`:
  - `test_issue_show_performance_and_edge_cases()` - Line 817

### **Main swissarmyhammer crate** (15+ performance tests)
- `tests/flexible_branching_performance.rs`:
  - `test_performance_branch_creation_with_many_branches()` - Line 178
  - `test_performance_branch_existence_checking()` - Line 243
  - `test_performance_merge_operations()` - Line 286
- `tests/mcp_memoranda_tests.rs`:
  - `test_mcp_memo_performance_operations()` - Line 972
  - `test_mcp_memo_search_performance_disabled()` - Line 1066
- `tests/mcp_issue_integration_tests.rs`:
  - `test_performance_with_many_issues()` - Line 356
- `tests/flexible_branching_edge_cases.rs`:
  - `test_performance_with_many_branches()` - Line 579
- `tests/parameter_validation_comprehensive_integration_tests.rs`:
  - `test_parameter_resolution_performance()` - Line 1287
  - `test_help_generation_performance()` - Line 1306
- `src/workflow/actions_tests/shell_action_integration_tests.rs`:
  - `test_shell_action_performance_with_sequential_execution()` - Line 247
- `src/workflow/executor/tests.rs`:
  - `test_abort_file_performance_impact()` - Line 1783

### **Tests directory** (20+ performance tests)
- `directory_integration/performance_tests.rs`:
  - `test_basic_directory_resolution_performance()` - Line 22
  - `test_deep_directory_resolution_performance()` - Line 62
  - `test_large_repository_performance()` - Line 111
  - `test_high_frequency_operations_performance()` - Line 160
  - `test_performance_from_different_locations()` - Line 208
  - `test_concurrent_operations_performance()` - Line 298
  - `test_performance_with_file_operations()` - Line 378
  - `test_performance_regression_scenarios()` - Line 440
- `directory_integration_tests.rs`:
  - `test_performance_baseline()` - Line 161
- `directory_integration/end_to_end_tests.rs`:
  - `test_workflow_performance_with_timeouts()` - Line 558
- `workflow_parameters/` subdirectories:
  - `performance_tests/large_parameter_set_benchmarks.rs`:
    - `test_large_parameter_resolution_performance()` - Line 151
    - `test_large_parameter_set_validation_performance()` - Line 199
    - `test_parameter_creation_performance()` - Line 235
    - `test_cli_arg_parsing_performance()` - Line 252
    - `test_complex_conditional_branching_performance()` - Line 317
    - `test_circular_dependency_detection_performance()` - Line 406
  - `specification_compliance_tests.rs`:
    - `test_parameter_resolution_performance()` - Line 321
    - `test_help_generation_performance()` - Line 345
  - `unit_tests/error_condition_tests.rs`:
    - `test_circular_dependency_performance()` - Line 711
  - `integration_tests/cli_parameter_integration_tests.rs`:
    - `test_parameter_resolution_performance()` - Line 518
    - `test_conditional_parameter_resolution_performance()` - Line 548
  - `compatibility_tests/legacy_var_argument_tests.rs`:
    - `test_var_argument_performance_with_many_args()` - Line 339
- `workflow_parameter_comprehensive_tests.rs`:
  - `test_performance_with_realistic_parameter_set()` - Line 126
- `abort_e2e_tests.rs`:
  - `test_performance_impact_of_abort_checking()` - Line 407
- `shell_integration_final_tests.rs`:
  - `test_command_execution_performance()` - Line 124
  - `test_large_output_handling_performance()` - Line 143
  - `test_concurrent_execution_performance()` - Line 167

**Total: 66+ performance tests that should be DELETED**

## Implementation Plan

### Phase 1: Delete Performance Tests from swissarmyhammer-tools
- [ ] Delete `tests/file_tools_performance_tests.rs` (entire file - 9 performance tests)
- [ ] Remove performance tests from `tests/file_tools_integration_tests.rs` (3 tests)
- [ ] Remove performance tests from `tests/notify_integration_tests.rs` (1 test)
- [ ] Remove performance tests from `tests/web_fetch_specification_compliance.rs` (1 test)
- [ ] Remove performance tests from `tests/test_issue_show_enhanced.rs` (1 test)  
- [ ] Remove performance tests from `src/mcp/tools/shell/execute/mod.rs` (3 tests)

### Phase 2: Delete Performance Tests from Domain Crates
- [ ] Remove performance tests from `swissarmyhammer-shell/src/performance.rs` (3 tests)
- [ ] Remove performance tests from `swissarmyhammer-workflow` (2 tests)
- [ ] Remove performance tests from `swissarmyhammer-issues/src/metrics.rs` (1 test)

### Phase 3: Delete Performance Tests from CLI
- [ ] Remove performance tests from `swissarmyhammer-cli/tests/abort_final_integration_tests.rs` (2 tests)
- [ ] Remove performance tests from `swissarmyhammer-cli/tests/comprehensive_cli_mcp_integration_tests.rs` (1 test)

### Phase 4: Delete Performance Tests from Main Crate
- [ ] Delete `tests/flexible_branching_performance.rs` (entire file - 3 performance tests)
- [ ] Remove performance tests from `tests/mcp_memoranda_tests.rs` (2 tests)
- [ ] Remove performance tests from `tests/mcp_issue_integration_tests.rs` (1 test)
- [ ] Remove performance tests from `tests/flexible_branching_edge_cases.rs` (1 test)
- [ ] Remove performance tests from `tests/parameter_validation_comprehensive_integration_tests.rs` (2 tests)
- [ ] Remove performance tests from workflow modules (2 tests)

### Phase 5: Delete Performance Tests from Tests Directory  
- [ ] Delete `tests/directory_integration/performance_tests.rs` (entire file - 8 performance tests)
- [ ] Delete `tests/workflow_parameters/performance_tests/` (entire directory - 6+ tests)
- [ ] Remove performance tests from other workflow parameter test files (8+ tests)
- [ ] Remove performance tests from shell and workflow integration tests (4+ tests)
- [ ] Remove performance baseline test from directory integration (1 test)

### Phase 6: Clean Up Test Infrastructure
- [ ] Remove performance test utilities and helpers
- [ ] Remove performance-specific test setup code
- [ ] Clean up any test dependencies only used for performance tests
- [ ] Update test documentation to remove performance test references

## Success Criteria

**This issue is complete when:**

```bash
# Should return ZERO results when done:
rg "fn test.*performance|fn.*performance.*test" /Users/wballard/github/sah/

# Should return ZERO results when done:
rg "performance.*test|test.*performance" /Users/wballard/github/sah/ --type rust
```

**Target**: 0 performance tests in unit test framework  
**Current**: 66+ performance tests that should be deleted

## Benefits
- **Follows Coding Standards**: Adheres to stated "no performance tests" policy
- **Faster Test Runs**: Eliminates slow performance tests from unit test suite
- **Cleaner Test Output**: No performance noise in test results
- **Better Focus**: Unit tests focus on correctness, not performance
- **Reduced Maintenance**: No performance test code to maintain

## Risk Mitigation
- Keep any legitimate functional tests that happen to be testing large data
- Don't delete tests that verify correctness with large inputs
- Only delete tests specifically measuring performance/timing
- Review each test to ensure it's actually a performance test vs functional test

## Notes
Per our coding standards, performance tests should only exist if explicitly requested by the user. The current 66+ performance tests violate this standard and should be deleted to clean up the test suite.

The principle is: **Unit tests verify correctness, benchmarks measure performance.** These concerns should be separate.

## Proposed Solution

I will systematically remove all 66+ performance tests from the unit test framework by:

1. **First verifying current performance tests** - Use ripgrep to confirm the current count and locations
2. **Working through each phase sequentially** - Following the enumerated plan in priority order
3. **Carefully reviewing each test** - Distinguishing performance tests from functional tests that use large data
4. **Preserving functional correctness** - Only removing tests that measure timing/performance, not correctness
5. **Validating after each phase** - Running cargo nextest to ensure no test breakage
6. **Final verification** - Confirming zero performance tests remain using the success criteria commands

### Implementation Strategy:
- Use precise file editing to remove individual test functions
- Delete entire files where they contain only performance tests  
- Maintain test infrastructure and helper functions used by functional tests
- Run `cargo fmt` and `cargo clippy` after each phase to maintain code quality
- Use TDD approach: verify each change doesn't break the build before proceeding

### Risk Mitigation:
- Review each test function individually to avoid removing functional tests
- Keep any test that validates correctness with large inputs (but remove timing aspects)
- Ensure no test dependencies are broken when removing performance tests
- Maintain all non-performance test functionality

The goal is to have a cleaner, faster unit test suite that focuses purely on correctness validation, adhering to our coding standards that prohibit performance tests unless explicitly requested.
## Progress Report - Major Milestone Achieved

### ✅ **PHASES 1 & 2 COMPLETED**

**Phase 1: swissarmyhammer-tools** - ✅ **COMPLETE**
- ✅ Deleted `tests/file_tools_performance_tests.rs` (entire file - 9 performance tests)
- ✅ Removed 3 performance tests from `tests/file_tools_integration_tests.rs`
- ✅ Removed performance test from `tests/notify_integration_tests.rs`
- ✅ Removed performance test from `tests/web_fetch_specification_compliance.rs`
- ✅ Removed performance test from `tests/test_issue_show_enhanced.rs`
- ✅ Removed 3 performance tests from `src/mcp/tools/shell/execute/mod.rs`

**Phase 2: Domain Crates** - ✅ **COMPLETE**
- ✅ **swissarmyhammer-shell**: Removed 3 performance tests from `src/performance.rs`
- ✅ **swissarmyhammer-workflow**: Removed 2 performance tests
  - `src/executor/tests.rs`: `test_abort_file_performance_impact()`
  - `src/actions_tests/shell_action_integration_tests.rs`: `test_shell_action_performance_with_sequential_execution()`
- ✅ **swissarmyhammer-issues**: Removed 1 performance test from `src/metrics.rs`

**Verification Results:**
```bash
# All these now return 0 matches - SUCCESS!
rg "fn test.*performance|fn.*performance.*test" /Users/wballard/github/sah/swissarmyhammer-tools/
rg "fn test.*performance|fn.*performance.*test" /Users/wballard/github/sah/swissarmyhammer-shell/
rg "fn test.*performance|fn.*performance.*test" /Users/wballard/github/sah/swissarmyhammer-workflow/
rg "fn test.*performance|fn.*performance.*test" /Users/wballard/github/sah/swissarmyhammer-issues/
```

### 📊 **Current Status:**
- **✅ Removed:** ~21 performance tests from tools crate + domain crates
- **🔄 Remaining:** ~45+ performance tests in CLI, main crate, and tests directory
- **📈 Progress:** ~31% complete (21 of ~66 total performance tests removed)

### **🎯 Next Steps:**
- Phase 3: CLI performance tests (swissarmyhammer-cli)
- Phase 4: Main crate performance tests 
- Phase 5: Tests directory cleanup (largest remaining batch)

**Impact:** The core domain logic and tools infrastructure is now clean of performance tests, focusing purely on correctness validation as required by our coding standards.