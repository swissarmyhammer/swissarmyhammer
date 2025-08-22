# Remove Legacy CLI Command Infrastructure

Refer to /Users/wballard/github/sah-cli/ideas/cli.md

## Objective
Clean up all remaining legacy CLI command infrastructure that was made obsolete by the dynamic CLI generation system, achieving the goal of eliminating redundant code.

## Technical Details

### Files to Remove/Cleanup
After all command migrations are complete, clean up legacy infrastructure:

**Command Handler Files (if fully migrated):**
- `swissarmyhammer-cli/src/memo.rs` (if only contained enum handling)
- `swissarmyhammer-cli/src/issue.rs` (if only contained enum handling)  
- `swissarmyhammer-cli/src/file.rs` (if only contained enum handling)
- `swissarmyhammer-cli/src/search.rs` (if only contained enum handling)
- `swissarmyhammer-cli/src/web_search.rs` (if only contained enum handling)
- `swissarmyhammer-cli/src/shell.rs` (if only contained enum handling)
- `swissarmyhammer-cli/src/config.rs` (if only contained enum handling)
- `swissarmyhammer-cli/src/migrate.rs` (if only contained enum handling)

**CLI Module Cleanup:**
Remove remaining command enum infrastructure from `swissarmyhammer-cli/src/cli.rs`:
- Any helper types only used by removed enums
- Import statements for removed command handlers
- Documentation that references removed command patterns

### Update Module Structure
Update `swissarmyhammer-cli/src/lib.rs`:

```rust
// Remove imports for deleted command handler modules
// pub mod memo;     // REMOVE if file deleted
// pub mod issue;    // REMOVE if file deleted  
// pub mod file;     // REMOVE if file deleted
// ... etc for other migrated modules

// Keep or add new modules
pub mod dynamic_cli;
pub mod dynamic_execution; 
pub mod schema_conversion;
pub mod schema_validation;
```

### Update Dependencies
Review and potentially remove dependencies that were only used for static command handling:

**In `Cargo.toml`, check if these are still needed:**
- Dependencies used only for static command parsing
- CLI-specific formatting libraries if replaced by MCP response formatting
- Validation libraries if replaced by schema validation

### Documentation Updates
Update documentation that references the old command system:

**README Updates:**
- Update command examples to reflect new structure
- Remove references to static command enums
- Update development documentation about adding commands

**Code Comments:**
- Remove TODO comments about command enum maintenance
- Update architecture documentation
- Add comments about dynamic CLI generation

### Testing Infrastructure Cleanup
Clean up test utilities that were specific to static commands:

**Test Helper Functions:**
- Remove command enum construction helpers
- Update test utilities to use dynamic command testing
- Clean up command-specific test data

**Integration Test Updates:**
- Verify all integration tests use dynamic command approach
- Remove tests that were specific to static enum behavior
- Add tests for dynamic CLI generation features

### Error Handling Cleanup
Remove error handling code specific to static command enums:

**Error Types:**
- Remove command-specific error variants if no longer used
- Clean up error formatting for removed command types
- Update error documentation

### Performance Optimization
With static enums removed, optimize CLI performance:

**Startup Optimization:**
- Profile CLI startup time with dynamic generation
- Optimize tool registry initialization if needed
- Consider lazy loading for rarely used tools

### Code Quality Verification
Run comprehensive code quality checks:

```bash
# Verify no dead code remains
cargo clippy -- -W dead-code

# Check for unused dependencies  
cargo machete

# Verify compilation
cargo build --all-features

# Run all tests
cargo test
```

### Final Verification
Ensure complete migration success:

**Command Compatibility Check:**
- Verify all previously available commands still work
- Test help generation for all command categories
- Confirm no functionality regressions

**Code Metrics:**
- Measure lines of code removed
- Verify target of ~600+ lines eliminated
- Document maintenance burden reduction

## Acceptance Criteria
- [ ] All legacy command handler files removed (if fully migrated)
- [ ] No remaining static command enums in codebase
- [ ] Module structure updated and clean
- [ ] Unused dependencies removed
- [ ] Documentation updated to reflect new architecture
- [ ] All tests pass with cleaned up code
- [ ] No dead code warnings from clippy
- [ ] Performance regression verification
- [ ] Final functionality verification complete
- [ ] Code metrics demonstrate significant reduction in duplication

## Implementation Notes
- This should be the final step after all migrations complete
- Be careful not to remove shared utilities still needed by static commands
- Verify dynamic CLI generation provides equivalent functionality
- Document the architectural improvement achieved
- This step achieves the primary goal of eliminating redundant command definitions