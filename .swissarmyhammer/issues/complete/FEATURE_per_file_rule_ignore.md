# Per-File Rule Ignore Comments

## Summary
Add support for ignoring rules on a per-file basis using inline comments. This allows developers to selectively disable rules in specific files where they have legitimate reasons to bypass certain checks.

## Proposed Syntax
```
<comment-syntax> sah rule ignore <rule-name-glob>
```

### Examples Across Languages
```javascript
// sah rule ignore no-console
// sah rule ignore no-* (ignore all rules starting with "no-")
```

```python
# sah rule ignore no-print-statements
```

```rust
// sah rule ignore no-unwrap
/* sah rule ignore no-panic */
```

```html
<!-- sah rule ignore no-inline-styles -->
```

```css
/* sah rule ignore no-important */
```

## Implementation Considerations

### Detection Strategy
- Scan each file for lines matching the pattern: `sah rule ignore <glob>`
- Use regex to extract ignore directives regardless of comment syntax
- Suggested regex pattern: `(?:\/\/|#|\/\*|<!--|--)\s*sah\s+rule\s+ignore\s+([^\s*>]+)`
- Build a per-file ignore list before running rule checks

### Glob Matching
- Support wildcards for flexible rule matching: `no-*`, `*-unwrap`, `security-*`
- Allow multiple ignores per file with separate comments
- Consider whether to support comma-separated lists: `// sah rule ignore no-console, no-debugger`

### Scope Options
**Option 1: File-level (Recommended)**
- Place comment anywhere in file (conventionally at top)
- Applies to entire file
- Simple to implement and understand

**Option 2: Line-level**
- Place comment on or immediately before the violating line
- Only ignores rule for that specific line
- More granular but complex to implement

**Option 3: Block-level**
```javascript
// sah rule ignore-start no-console
console.log("debug");
console.log("more debug");
// sah rule ignore-end
```
- Start/end markers for sections
- Most flexible but highest complexity

### Recommendation: Start with File-level (Option 1)
- Simplest to implement and understand
- Covers most use cases
- Can expand to line/block level in future iteration if needed

## Alternative Syntax Considerations

### Alternate Names
- `sah-ignore` (shorter)
- `sah-disable` (clearer intent)
- `sah-skip-rule` (explicit)

### Alternate Formats
```
// @sah ignore: no-console
// sah:ignore[no-console]
// pragma sah ignore no-console
```

Recommendation: Stick with `sah rule ignore` as it's clear and self-documenting

## Edge Cases to Handle
1. **Invalid glob patterns**: Log warning, don't ignore any rules
2. **Nonexistent rule names**: Log warning to help catch typos
3. **Conflicting ignores**: If file and project config both specify ignores, merge them
4. **Comment detection**: Handle edge cases like commented-out code containing the ignore pattern
5. **Multi-line comments**: Ensure detection works in `/* */` style blocks

## User Experience

### Reporting
When a rule is ignored:
- Optionally report in output: `ℹ️ no-console ignored in src/debug.js (file directive)`
- Include ignore count in summary: `Found 5 violations (3 ignored by directives)`

### Documentation Needs
- Add examples to README
- Document glob patterns supported
- Explain when to use ignores vs fixing violations
- Best practices: prefer fixing over ignoring

## Testing Requirements
- Test ignore detection across multiple comment syntaxes
- Test glob pattern matching (wildcards, exact matches)
- Test multiple ignore directives in same file
- Test invalid patterns and error handling
- Test reporting output includes ignore information

## Future Enhancements
- Line-level and block-level ignores
- Reason/justification field: `// sah rule ignore no-unwrap: validation already performed`
- Expiring ignores: `// sah rule ignore no-console until:2025-12-31`
- IDE integration to suggest adding ignores


## Proposed Solution

After analyzing the codebase, I'll implement file-level ignore directives by modifying the rule checking pipeline:

### Architecture

1. **Ignore Directive Parser** (`swissarmyhammer-rules/src/ignore.rs`)
   - Extract ignore directives from file content using regex
   - Support multiple comment syntaxes: `//`, `#`, `/*`, `<!--`
   - Parse glob patterns for flexible rule matching
   - Return a set of ignored rule names/patterns per file

2. **Integration Point: `RuleChecker::check_file()`** (`swissarmyhammer-rules/src/checker.rs`)
   - Before executing LLM check, parse file for ignore directives
   - Match rule name against ignore patterns using glob matching
   - Skip check if rule is ignored, log the skip at debug level
   - Continue with normal checking if not ignored

3. **Glob Pattern Matching**
   - Use the `glob` crate (already a dependency) for pattern matching
   - Support: `no-*`, `*-unwrap`, `specific-rule-name`
   - Cache parsed patterns per file for efficiency (part of existing cache key)

### Implementation Steps

1. **Create `ignore.rs` module**
   - `parse_ignore_directives(content: &str) -> Vec<String>` - extracts patterns
   - `should_ignore_rule(rule_name: &str, patterns: &[String]) -> bool` - checks if rule matches
   - Regex pattern: `(?://|#|/\*|<!--)\s*sah\s+rule\s+ignore\s+([^\s*>]+)`

2. **Modify `checker.rs:check_file()`**
   - Call `parse_ignore_directives()` after reading file content
   - Check `should_ignore_rule()` before Stage 1 rendering
   - Return `Ok(())` early if ignored, logging: `Rule {rule_name} ignored in {path} (file directive)`
   - No cache storage for ignored rules (they're skipped before cache check)

3. **Cache Strategy**
   - Ignored rules skip all processing (no cache interaction)
   - This means if an ignore directive is removed, the rule will be checked on next run
   - Cache key already includes file content, so changes to ignores trigger re-check

4. **Testing Strategy**
   - Unit tests for `parse_ignore_directives()` with various comment syntaxes
   - Unit tests for glob pattern matching
   - Integration test: create test file with ignore directive, verify rule is skipped
   - Integration test: verify multiple ignore directives work
   - Integration test: verify invalid patterns are handled gracefully

### Example Flow

```
File: src/debug.rs
Content:
```rust
// sah rule ignore no-*
// sah rule ignore allow-unwrap

fn debug_function() {
    println!("Debug output");  // would violate no-println
}
```

1. User runs: `sah rule check --patterns "**/*.rs"`
2. `check_file()` reads `src/debug.rs`
3. Parser extracts: `["no-*", "allow-unwrap"]`
4. For rule `no-println`:
   - Check if "no-println" matches "no-*" → YES
   - Log: "Rule no-println ignored in src/debug.rs (file directive)"
   - Return `Ok(())` without LLM call
5. For rule `complexity-check`:
   - Check if "complexity-check" matches patterns → NO
   - Continue with normal check process

### Design Decisions

1. **File-level only (for now)**: Simplest implementation covering 90% of use cases
2. **Skip before LLM**: Saves cost and time by avoiding unnecessary agent execution
3. **No cache for ignores**: Keeps cache logic simple, ignore changes take effect immediately
4. **Debug-level logging**: Doesn't clutter output but available for troubleshooting
5. **Graceful handling**: Invalid glob patterns log warning but don't fail the check
6. **Early return**: Clean separation of concern - ignored rules never enter check logic

### Files to Create/Modify

- **NEW**: `swissarmyhammer-rules/src/ignore.rs`
- **MODIFY**: `swissarmyhammer-rules/src/lib.rs` (add ignore module)
- **MODIFY**: `swissarmyhammer-rules/src/checker.rs` (integrate ignore logic)
- **NEW**: `swissarmyhammer-rules/tests/ignore_integration_test.rs`




## Implementation Complete

### Summary

Successfully implemented file-level rule ignore directives with the syntax:
```
<comment-syntax> sah rule ignore <rule-name-glob>
```

### Files Created/Modified

1. **NEW**: `swissarmyhammer-rules/src/ignore.rs`
   - `parse_ignore_directives()` - Extracts ignore patterns from file content
   - `should_ignore_rule()` - Checks if a rule matches ignore patterns using glob matching
   - Uses simple string parsing (avoiding regex complexity with special characters)
   - Handles various comment syntaxes: `//`, `#`, `/*`, `<!--`
   - Supports glob patterns: `*`, `?` for flexible matching

2. **MODIFIED**: `swissarmyhammer-rules/src/lib.rs`
   - Added `pub mod ignore;` to expose ignore functionality

3. **MODIFIED**: `swissarmyhammer-rules/src/checker.rs:check_file()`
   - Integrated ignore logic after reading file content
   - Checks ignore directives before cache lookup
   - Returns early if rule is ignored (skips LLM call)
   - Logs at debug level: "Rule {name} ignored in {path} (file directive)"

4. **NEW**: `swissarmyhammer-rules/tests/ignore_integration_test.rs`
   - 11 comprehensive integration tests
   - Tests single rules, glob patterns, multiple patterns, different comment styles
   - Tests whitespace handling, pattern positions, case sensitivity

### Test Results

- **Unit tests**: 17/17 passed (ignore module)
- **Integration tests**: 11/11 passed
- **Full package tests**: 217/217 passed

### Key Design Decisions

1. **Simple String Parsing**: Used word-based parsing instead of regex to avoid complexity with special characters like `*` and `?` in glob patterns

2. **Early Return**: Ignored rules skip all processing including cache checks, saving LLM costs

3. **File-Level Scope**: Ignore directive anywhere in file applies to entire file (simplest approach)

4. **Glob Matching**: Used `glob` crate (existing dependency) for pattern matching

5. **Debug Logging**: Logged ignored rules at debug level to avoid cluttering output

6. **No Cache Interaction**: Ignored rules skip cache entirely, ensuring immediate effect when directives change

### Example Usage

```rust
// sah rule ignore no-unwrap
fn debug_function() {
    let value = Some(1).unwrap(); // Won't be checked by no-unwrap rule
}
```

```rust
// sah rule ignore no-*
// Ignores: no-unwrap, no-panic, no-todo, etc.
```

```python
# sah rule ignore test-*
# Ignores all rules starting with "test-"
```

### Implementation Notes for Future Reference

- Parser handles comment syntax attached to "sah" (e.g., `//sah` vs `// sah`)
- Split by whitespace and look for word sequence: ends_with("sah"), "rule", "ignore", <pattern>
- First non-whitespace token after "ignore" is the pattern
- Glob matching handles `*` (any sequence), `?` (single char)
- Invalid glob patterns log warning but don't fail the check




## Code Review Fixes Completed

### Changes Made
1. Fixed clippy lint errors in `swissarmyhammer-rules/tests/ignore_integration_test.rs`:
   - Line 278: Changed `vec![]` to array `[]` for test_cases in test_ignore_directive_whitespace_handling
   - Line 312: Changed `vec![]` to array `[]` for positions in test_ignore_directive_position_in_file
   
2. Ran `cargo fmt --all` to ensure consistent formatting across all files

3. Verified all changes with `cargo clippy` - no warnings or errors

4. Verified all tests pass:
   - All 217 tests in swissarmyhammer-rules package pass
   - Integration tests for ignore directives all pass
   - Unit tests for ignore functionality all pass

### Testing Results
```
cargo nextest run --package swissarmyhammer-rules
Summary: 217 tests run: 217 passed, 0 skipped
```

### Code Quality
- All clippy lints resolved
- All code properly formatted with rustfmt  
- No compilation warnings
- All existing tests pass
- Feature is complete and ready for use

### Note
There is an unrelated test failure in swissarmyhammer-cli package (test_abort_file_cleanup_between_command_runs) that is not related to the per-file rule ignore feature changes.
