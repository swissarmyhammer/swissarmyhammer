# Remove ALL #[allow(dead_code)] Attributes and Delete Dead Code

## Problem
The codebase contains **103+ instances** of `#[allow(dead_code)]` across 39 files, violating our coding standards. According to `builtin/prompts/coding_standards.md.liquid:134`:

> "Never #[allow(dead_code)], delete it -- we have source control these days"

Dead code should be deleted, not hidden with compiler directive suppression.

## Evidence of Violations

### **Complete List of #[allow(dead_code)] Locations:**

#### swissarmyhammer-shell
- `src/hardening.rs:152` - 1 occurrence

#### swissarmyhammer-workflow  
- `src/parser.rs:774` - 1 occurrence
- `src/storage.rs:131` - 1 occurrence (with comment)
- `src/action_parser.rs:26` - 1 occurrence (with comment "Used in tests")
- `src/action_parser.rs:72` - 1 occurrence (with comment "Used in tests")
- `src/action_parser.rs:84` - 1 occurrence (with comment "Used in tests")
- `src/test_helpers.rs:24, 42, 74, 118` - 4 occurrences
- `src/actions_tests/common.rs:7, 28` - 2 occurrences
- `src/actions.rs:1387, 1390, 1393, 1396, 1402, 1414` - 6 occurrences

#### swissarmyhammer-cli
- `src/cli.rs:493` - 1 occurrence
- `src/completions.rs:9` - 1 occurrence  
- `src/signal_handler.rs:4` - 1 occurrence
- `src/validate.rs:400` - 1 occurrence (with comment)
- `src/validate.rs:896` - 1 occurrence (with comment "Used by test infrastructure")
- `src/error.rs:14, 32, 43, 146` - 4 occurrences
- `src/commands/doctor/types.rs:69, 174, 192, 201` - 4 occurrences
- `src/schema_conversion.rs:19, 53, 56` - 3 occurrences
- `src/test.rs:329, 367` - 2 occurrences
- `src/parameter_cli.rs:67` - 1 occurrence
- `src/schema_validation.rs:13, 72, 88, 96, 99` - 5 occurrences
- `src/dynamic_cli.rs:92` - 1 occurrence (with comment)
- `src/mcp_integration.rs:158, 170, 176, 193` - 4 occurrences
- `tests/abort_final_integration_tests.rs:35, 41` - 2 occurrences
- `tests/test_utils.rs:24, 33, 86, 101, 198, 207, 276, 301, 622` - 9 occurrences
- `tests/in_process_test_utils.rs:13, 15, 23` - 3 occurrences

#### swissarmyhammer-config
- `tests/environment_variable_tests.rs:25` - 1 occurrence
- `tests/environment_variable_tests_fixed.rs:15, 59, 68` - 3 occurrences

#### swissarmyhammer-tools
- `src/mcp/tools/outline/generate/mod.rs:283` - 1 occurrence

#### swissarmyhammer-outline
- `src/file_discovery.rs:274` - 1 occurrence

#### swissarmyhammer (main crate)
- `src/template.rs:87` - 1 occurrence (with comment)
- `src/search/indexer.rs:631` - 1 occurrence
- `src/workflow/parser.rs:772` - 1 occurrence
- `src/workflow/storage.rs:131` - 1 occurrence (with comment)
- `src/workflow/actions.rs:1380, 1383, 1386, 1389, 1395, 1407` - 6 occurrences
- `src/workflow/test_helpers.rs:24, 42, 74, 118` - 4 occurrences
- `src/workflow/actions_tests/common.rs:7, 28` - 2 occurrences
- `src/workflow/agents/llama_agent_executor.rs:1254` - 1 occurrence
- `tests/flexible_branching_performance.rs:155` - 1 occurrence

#### Total: **103+ #[allow(dead_code)] violations**

## Implementation Plan

### Phase 1: Analyze Each Dead Code Instance
- [ ] Review each `#[allow(dead_code)]` location
- [ ] Determine if the code is actually dead or incorrectly flagged
- [ ] For actually dead code: delete it entirely
- [ ] For incorrectly flagged code: find why it's not being detected as used

### Phase 2: Remove Dead Code (Delete Strategy)
- [ ] **swissarmyhammer-shell**: 1 location to clean up
- [ ] **swissarmyhammer-workflow**: 15+ locations to clean up  
- [ ] **swissarmyhammer-cli**: 38+ locations to clean up
- [ ] **swissarmyhammer-config**: 4+ locations to clean up
- [ ] **swissarmyhammer-tools**: 1+ location to clean up
- [ ] **swissarmyhammer-outline**: 1+ location to clean up
- [ ] **swissarmyhammer (main)**: 20+ locations to clean up

### Phase 3: Fix Incorrectly Flagged Code
For code that's actually used but flagged as dead:
- [ ] Check if it's only used in tests - move to test modules if needed
- [ ] Check if it's used conditionally - restructure if needed
- [ ] Check if it's used via macros or reflection - document usage
- [ ] Check if it's pub but unused - make private or delete

### Phase 4: Crate-by-Crate Cleanup

#### Priority 1: Critical Crates
- [ ] **swissarmyhammer-workflow** - 15+ violations (highest count)
- [ ] **swissarmyhammer-cli** - 38+ violations (highest count)
- [ ] **swissarmyhammer (main)** - 20+ violations

#### Priority 2: Domain Crates  
- [ ] **swissarmyhammer-shell** - 1 violation
- [ ] **swissarmyhammer-outline** - 1 violation
- [ ] **swissarmyhammer-tools** - 1 violation
- [ ] **swissarmyhammer-config** - 4+ violations

### Phase 5: Prevention
- [ ] Add clippy lint to prevent future `#[allow(dead_code)]`
- [ ] Set up CI to catch dead code allowances
- [ ] Document the no-dead-code policy clearly
- [ ] Add automated checking in build process

## Success Criteria

**This issue is complete when:**

```bash
# Should return ZERO results when done:
rg "#\[allow\(dead_code\)\]|#\[.*allow.*dead_code.*\]" /Users/wballard/github/sah/

# Verification:
echo "Dead code allowances: $(rg '#\[allow\(dead_code\)\]' /Users/wballard/github/sah/ | wc -l)"
```

**Target**: 0 occurrences of `#[allow(dead_code)]`  
**Current**: 103+ occurrences

## Approach Strategy

### For Each #[allow(dead_code)]:
1. **Remove the attribute**
2. **Run `cargo build`**  
3. **If compilation fails**:
   - If truly unused → **DELETE the code**
   - If used but not detected → **Fix the usage detection**
   - If only used in tests → **Move to test module or mark with #[cfg(test)]**

### Categories of Dead Code:
- **Truly Dead**: Delete entirely
- **Test-Only**: Move to test modules  
- **Conditionally Used**: Fix with proper cfg attributes
- **Public API**: Decide if it should be private or deleted
- **Future Features**: Delete (we have source control)

## Risk Mitigation
- Work incrementally, one file at a time
- Test after each dead code removal
- Keep git commits granular for easy rollback
- Verify no functionality is lost
- Check that tests still pass after cleanup

## Benefits
- **Cleaner Codebase**: No dead code hiding in the codebase
- **Smaller Binaries**: Dead code doesn't get compiled into final artifacts
- **Better Maintenance**: No confusion about what code is actually used
- **Follows Standards**: Adheres to stated coding standards
- **Easier Refactoring**: Clear visibility into what code is actually needed

## Notes
This cleanup is required by our coding standards and will improve codebase maintainability. Dead code creates confusion and maintenance overhead - it should be deleted rather than hidden with compiler directives.

The principle is: **If the code isn't needed, delete it. We have source control to recover it if ever actually needed.**