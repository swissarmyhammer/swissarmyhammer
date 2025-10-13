# Implement Rule Check Command Core Logic

Refer to ideas/rules.md

## Goal

Implement the core logic for `sah rule check [glob...]` command.

## Context

The check command is the main entry point for running rules. It validates rules, loads files, and executes checks with fail-fast behavior.

## Implementation

1. In `check.rs`, define `CheckCommand` struct:
```rust
pub struct CheckCommand {
    pub patterns: Vec<String>,         // Glob patterns
    pub rule: Option<Vec<String>>,     // Filter by rule names
    pub severity: Option<Severity>,    // Filter by severity
    pub category: Option<String>,      // Filter by category
}
```

2. Implement `execute_check_command()`:
   - Load all rules via RuleResolver
   - Validate all rules first (fail if any invalid)
   - Apply filters (rule names, severity, category)
   - Display what will be checked
   
3. Add validation phase before checking:
```rust
println!("Validating rules...");
for rule in &rules {
    rule.validate()?;
}
println!("✓ All {} rules are valid\n", rules.len());
```

## Testing

- Test with no rules
- Test with filters
- Test validation phase
- Test with invalid rules

## Success Criteria

- [ ] CheckCommand struct defined
- [ ] Command parsing works
- [ ] Rule loading and filtering works
- [ ] Validation phase implemented
- [ ] Tests passing



## Proposed Solution

Based on analysis of the existing code, I will implement the core logic for the `sah rule check` command by:

1. **Update `CheckCommand` struct in cli.rs:**
   - Replace `files: Vec<String>` with `patterns: Vec<String>` (glob patterns)
   - Add `severity: Option<Severity>` filter
   - Add `category: Option<String>` filter  
   - Keep `rule_name: Option<String>` but rename to `rule: Option<Vec<String>>` for multiple rule filtering
   - Remove `fix` field (not part of core check logic)

2. **Implement `execute_check_command()` in check.rs:**
   - Phase 1: Load all rules via `RuleResolver` and `RuleLibrary`
   - Phase 2: Validate all rules first (fail if any invalid)
   - Phase 3: Apply filters (rule names, severity, category)
   - Phase 4: Expand glob patterns to file paths
   - Phase 5: Create `RuleChecker` with agent
   - Phase 6: Run `check_all()` with fail-fast behavior
   - Display appropriate messages and exit with proper code

3. **Integration with existing infrastructure:**
   - Use `RuleResolver::new()` and `load_all_rules()` from `swissarmyhammer-rules`
   - Use `RuleLibrary` for rule management
   - Use `RuleChecker::new()` for checking logic
   - Use `LlamaAgentExecutorWrapper` for agent execution
   - Handle `RuleError::Violation` for fail-fast behavior

4. **Testing approach:**
   - Update existing tests to match new CheckCommand structure
   - Add tests for glob pattern expansion
   - Add tests for filtering logic
   - Add tests for validation phase
   - Add integration tests with mock agent (if possible)



## Implementation Notes

### Changes Made

1. **Updated CheckCommand struct** (cli.rs:35-40):
   - Changed `files: Vec<String>` to `patterns: Vec<String>` for glob pattern support
   - Changed `rule_name: Option<String>` to `rule: Option<Vec<String>>` to support multiple rule filtering
   - Added `severity: Option<String>` for severity-based filtering
   - Added `category: Option<String>` for category-based filtering
   - Removed `fix: bool` field (not part of core check logic)

2. **Implemented execute_check_command()** (check.rs:15-119):
   - Phase 1: Load all rules via `RuleResolver::new()` and `load_all_rules()`
   - Phase 2: Validate all rules with `rule.validate()` before checking (fail-fast)
   - Phase 3: Apply filters for rule names, severity, and category
   - Phase 4: Expand patterns to file paths (simple file/directory expansion for now)
   - Phase 5: Create `RuleChecker` with `LlamaAgentExecutorWrapper`
   - Phase 6: Run `check_all()` with fail-fast behavior on violations

3. **Updated all tests** to use new CheckCommand structure:
   - cli.rs: Updated parsing tests
   - check.rs: Added tests for no rules and no files scenarios
   - mod.rs: Updated integration tests

### Technical Decisions

- Used `walkdir::WalkDir` for simple path expansion instead of full glob support
- Kept validation phase separate from checking phase for clarity
- Used quiet mode in tests to suppress output
- Properly handle both violation errors and other errors differently

### Test Results

All 96 tests pass successfully in the rules command module.



## Code Review Implementation Notes

Completed all code review items from CODE_REVIEW.md:

### 1. ✅ Formatting Issues
- Ran `cargo fmt --package swissarmyhammer-cli` to fix all formatting violations
- Verified with `cargo fmt --check`

### 2. ✅ Proper Glob Pattern Implementation  
- Replaced `walkdir` with proper glob pattern matching using `glob` crate
- Added `ignore` crate for `.gitignore` support via `WalkBuilder`
- Implemented `expand_glob_patterns()` helper function with:
  - Support for direct file paths
  - Support for directory walking with gitignore
  - Support for wildcard patterns like `*.rs`
  - Support for recursive patterns like `**/*.rs`
  - Proper relative and absolute path handling
- Files: swissarmyhammer-cli/src/commands/rule/check.rs:19-152

### 3. ✅ Comprehensive Tests
Added extensive test coverage:
- `test_expand_glob_patterns_single_file` - Direct file path handling
- `test_expand_glob_patterns_directory` - Directory walking
- `test_expand_glob_patterns_wildcard` - Wildcard pattern matching
- `test_expand_glob_patterns_recursive` - Recursive `**` patterns
- `test_expand_glob_patterns_multiple_patterns` - Multiple pattern support
- `test_expand_glob_patterns_respects_gitignore` - Gitignore integration
- `test_expand_glob_patterns_empty_on_no_match` - Empty result handling
- `test_execute_check_command_filter_by_severity` - Severity filtering
- `test_execute_check_command_filter_by_category` - Category filtering
- `test_execute_check_command_filter_by_rule_name` - Rule name filtering
- `test_execute_check_command_combined_filters` - Combined filter application

All 1135 tests pass (26 in check module).

### 4. ✅ Agent Configuration
- Replaced `LlamaAgentConfig::for_testing()` with `LlamaAgentConfig::for_small_model()`
- Added TODO comment to make this configurable via CLI flags or config file
- File: swissarmyhammer-cli/src/commands/rule/check.rs:248-251

### 5. ✅ Comprehensive Documentation
- Added module-level rustdoc comments explaining the command
- Added detailed function documentation with:
  - Phase-by-phase execution explanation
  - Parameter descriptions
  - Return value documentation
  - Usage examples with bash commands
- File: swissarmyhammer-cli/src/commands/rule/check.rs:154-177

### 6. ✅ Improved Error Handling
- Replaced string prefix matching with pattern-based detection
- Check for violation patterns: "violated in" and "(severity:"
- Provides clear error messages distinguishing violations from other errors
- File: swissarmyhammer-cli/src/commands/rule/check.rs:262-283

### 7. ✅ Dependency Management
- Added `glob = { workspace = true }` to Cargo.toml
- Added `ignore = { workspace = true }` to Cargo.toml
- Both crates were already in workspace dependencies

### 8. ✅ Code Quality
- All tests pass: `cargo nextest run --package swissarmyhammer-cli`
- All clippy checks pass: `cargo clippy --package swissarmyhammer-cli -- -D warnings`
- Code is properly formatted: `cargo fmt --check`

### Design Decisions

1. **Glob Pattern Matching**: Used `ignore::WalkBuilder` for directory walking because it provides native `.gitignore` support, which is essential for avoiding checking generated or ignored files.

2. **Test Approach**: Tests that need glob pattern matching change to temp directory using `std::env::set_current_dir()` to enable relative pattern testing. The gitignore test initializes a git repo to ensure `.gitignore` files are respected.

3. **Error Handling**: Since `SwissArmyHammerError` wraps `RuleError` and doesn't support downcasting, used pattern-based detection of violation errors by checking for the standard violation display format.

4. **Agent Config**: Using `for_small_model()` provides a reasonable default that's more suitable for production use than `for_testing()`, while keeping the door open for future configurability.

### Success Criteria Status

- ✅ CheckCommand struct defined with glob pattern support
- ✅ Command parsing works with all filters
- ✅ Rule loading and filtering works correctly
- ✅ Validation phase implemented with fail-fast behavior
- ✅ All tests passing (1135/1135)
- ✅ Code quality checks pass (clippy, fmt)
- ✅ Comprehensive test coverage added
- ✅ Documentation complete and thorough
