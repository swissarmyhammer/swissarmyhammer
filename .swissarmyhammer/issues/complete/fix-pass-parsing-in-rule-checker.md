# Fix PASS/VIOLATION Parsing in Rule Checker

## Problem
The rule checker is incorrectly treating PASS responses as violations. When the LLM returns a response starting with "PASS", the checker should recognize it as passing, but currently it's being treated as a violation.

## Evidence
```
Message: PASS

This is a Rust build script that generates code at compile time. It does not interact with databases or construct SQL queries. The code only:
- Reads environment variables
- Walks the filesystem to collect rule files
- Generates Rust source code as string literals
- Writes the generated code to a file

No SQL injection vulnerabilities are present.
```

This response clearly starts with "PASS" and includes an explanation, but it's being treated as a violation with WARN/ERROR level logging.

## Root Cause
In `checker.rs` around line 304:

```rust
let result_text = response.content.trim();
if result_text == "PASS" {
```

The code checks for exact equality with "PASS", but the LLM response is:
```
PASS

[explanation...]
```

So `result_text` is the full multi-line response, not just "PASS".

## LLM Response Format
The LLM returns one of two formats:
1. **PASS**: Response starts with "PASS" followed by optional explanation
2. **VIOLATION**: Response starts with "VIOLATION" followed by details

## Solution
Change the parsing logic to check if the response **starts with** "PASS":

```rust
let result_text = response.content.trim();
if result_text.starts_with("PASS") {
    // Pass case - log at info level
    tracing::info!("Check passed for {} against rule {}", target_path.display(), rule.name);
    Ok(())
} else {
    // Violation case (starts with "VIOLATION")
    // Create violation and fail-fast
}
```

## Location
File: `swissarmyhammer-rules/src/checker.rs`
Function: `RuleChecker::check_file()`
Lines: ~304-330

## Acceptance Criteria
- [ ] Responses starting with "PASS" are correctly recognized as passing
- [ ] No WARN/ERROR logs for passing checks (use INFO level instead)
- [ ] PASS responses can include explanations after the PASS keyword
- [ ] VIOLATION responses are correctly handled as failures
- [ ] Add test case with "PASS\n\nexplanation" format
- [ ] Add test case with "VIOLATION\n\ndetails" format



## Proposed Solution

After examining the code at `swissarmyhammer-rules/src/checker.rs:304`, I can confirm the root cause:

The code currently uses exact equality check:
```rust
if result_text == "PASS" {
```

But the LLM returns multi-line responses like:
```
PASS

This is a Rust build script...
```

### Implementation Plan

1. **Change parsing logic** (line ~304):
   - Replace `==` with `starts_with("PASS")`
   - Keep the same behavior for both PASS and VIOLATION cases
   
2. **Update logging**:
   - PASS: Use `tracing::info!` (currently using `tracing::debug!`)
   - VIOLATION: Keep existing `tracing::warn!` behavior

3. **Test-Driven Development**:
   - Write test for "PASS" exact match (baseline)
   - Write test for "PASS\n\nexplanation" format
   - Write test for "VIOLATION\n\ndetails" format
   - Verify all tests pass after implementation

### Code Changes

File: `swissarmyhammer-rules/src/checker.rs`

Line ~304-315, replace:
```rust
if result_text == "PASS" {
    tracing::debug!(
        "Check passed for {} against rule {}",
        target_path.display(),
        rule.name
    );
    Ok(())
}
```

With:
```rust
if result_text.starts_with("PASS") {
    tracing::info!(
        "Check passed for {} against rule {}",
        target_path.display(),
        rule.name
    );
    Ok(())
}
```

This single-line change from `==` to `starts_with` will handle both:
- Exact "PASS" responses
- "PASS\n\nexplanation" responses with optional details



## Implementation Notes

### Changes Made

1. **Modified parsing logic** in `swissarmyhammer-rules/src/checker.rs:305`:
   - Changed from `result_text == "PASS"` to `result_text.starts_with("PASS")`
   - This allows responses like "PASS\n\nexplanation" to be correctly recognized

2. **Updated logging level** in `swissarmyhammer-rules/src/checker.rs:306`:
   - Changed from `tracing::debug!` to `tracing::info!` for PASS cases
   - This ensures passing checks are visible at info level, not just debug

3. **Added comprehensive tests**:
   - `test_pass_response_parsing_exact`: Validates exact "PASS" matching
   - `test_pass_response_parsing_with_explanation`: Validates multi-line PASS responses
   - `test_violation_response_parsing`: Validates VIOLATION response handling

### Test Results

All tests pass (153 total):
- Existing tests: 150 ✓
- New tests: 3 ✓

### Code Quality

- `cargo fmt`: ✓ No formatting issues
- `cargo clippy`: ✓ No lint warnings
- `cargo nextest`: ✓ All tests passing

### Verification

The fix correctly handles both LLM response formats:
1. **PASS only**: `"PASS"` → Recognized as passing
2. **PASS with explanation**: `"PASS\n\nThis is..."` → Recognized as passing
3. **VIOLATION**: `"VIOLATION\n\nDetails..."` → Correctly treated as violation
