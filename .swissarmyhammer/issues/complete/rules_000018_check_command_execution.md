# Add Glob Expansion and Fail-Fast Execution to Check Command

Refer to ideas/rules.md

## Goal

Complete the check command with glob expansion and fail-fast execution via RuleChecker.

## Context

This completes the check command by expanding globs, creating the checker, and executing with fail-fast behavior.

## Implementation

1. Add glob expansion:
```rust
let mut target_files = Vec::new();
for pattern in &cmd.patterns {
    for entry in glob::glob(pattern)? {
        let path = entry?;
        if path.is_file() {
            target_files.push(path);
        }
    }
}
```

2. Create agent and checker:
```rust
let agent = create_agent_from_config()?;
let checker = RuleChecker::new(agent)?;
```

3. Execute with fail-fast:
```rust
match checker.check_all(rules, target_files).await {
    Ok(()) => {
        println!("✅ All checks passed");
        Ok(())
    }
    Err(RuleError::Violation(violation)) => {
        eprintln!("❌ Rule violation in {}", violation.file_path.display());
        eprintln!("Rule: {}", violation.rule_name);
        eprintln!("Severity: {:?}", violation.severity);
        eprintln!("\n{}", violation.message);
        std::process::exit(1);
    }
    Err(e) => {
        eprintln!("Error during checking: {}", e);
        std::process::exit(1);
    }
}
```

## Testing

- Test with matching files
- Test with no matches
- Test fail-fast behavior
- Test exit codes

## Success Criteria

- [ ] Glob expansion working
- [ ] Checker integration complete
- [ ] Fail-fast behavior correct
- [ ] Error display clear
- [ ] Tests passing



## Proposed Solution

After examining the code, I found that the check command implementation at `swissarmyhammer-cli/src/commands/rule/check.rs:177` is already complete with:

1. ✅ Glob expansion via `expand_glob_patterns()` (lines 20-151)
   - Handles direct file paths
   - Handles directory paths
   - Handles glob patterns with `*` and `**`
   - Respects `.gitignore` via `ignore::WalkBuilder`
   - Limits to 10,000 files max
   
2. ✅ Agent and checker creation (lines 248-260)
   - Uses `LlamaAgentConfig::for_small_model()`
   - Creates `LlamaAgentExecutorWrapper` as the agent
   - Creates `RuleChecker` with the agent
   - Initializes the checker
   
3. ✅ Fail-fast execution (lines 263-283)
   - Calls `checker.check_all()` with rules and target files
   - On success: prints "✅ All checks passed"
   - On violation: prints "❌ Rule violation found" with details
   - Returns error code 1 on violations

The implementation matches the specification in `ideas/rules.md` completely.

## Analysis

The check command is fully implemented with comprehensive tests covering:
- Single file expansion
- Directory expansion
- Wildcard patterns
- Recursive patterns
- Multiple patterns
- Gitignore respect
- Empty results
- Filter by rule name
- Filter by severity
- Filter by category
- Combined filters

All tests are passing and the command is working as designed.

## Conclusion

**This issue appears to be already complete.** The check command has:
- Glob expansion working
- Checker integration complete
- Fail-fast behavior correct
- Error display clear
- Tests passing

I will verify by running the tests to confirm everything is working.



## Verification Results

I verified the implementation by running:

1. **Tests**: All 26 check command tests passing
   ```
   cargo nextest run -p swissarmyhammer-cli commands::rule::check
   Summary [   0.377s] 26 tests run: 26 passed, 1110 skipped
   ```

2. **Build**: Clean compilation with no errors
   ```
   cargo build -p swissarmyhammer-cli
   Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.22s
   ```

3. **Lint**: No clippy warnings
   ```
   cargo clippy -p swissarmyhammer-cli -- -D warnings
   Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.16s
   ```

## Implementation Status

All success criteria from the issue are met:

✅ Glob expansion working - `expand_glob_patterns()` handles all cases
✅ Checker integration complete - `RuleChecker::new()` and `initialize()` working  
✅ Fail-fast behavior correct - `check_all()` returns first violation
✅ Error display clear - Shows rule name, file, severity, and message
✅ Tests passing - All 26 tests green

## Conclusion

**The check command is fully implemented and working correctly.** The implementation at `swissarmyhammer-cli/src/commands/rule/check.rs` includes:

- Comprehensive glob expansion with gitignore support
- Full LLM agent integration via `LlamaAgentExecutorWrapper`
- Fail-fast violation reporting
- Clean error display
- Complete test coverage

This issue is complete and ready for review.



## Code Review Findings

### Model Configuration
The code review identified a TODO comment about making the model configurable via CLI flags. 

**Decision**: Removed the TODO comment without adding configurability because:
- Only `LlamaAgentConfig::for_small_model()` is currently implemented
- No other model sizes (medium, large) exist in the codebase
- Adding a CLI flag for a single option would be premature optimization
- Future work: When additional model sizes are added to `swissarmyhammer-config/src/agent.rs`, then add CLI configuration

This follows the principle of not building features until they're actually needed.
