# Add rule_name Parameter to Check Prompt Context

## Description
The `.check.md` prompt has been updated to include more content like file and rule information. We need to ensure that `rule_name` is passed as a parameter when rendering the check prompt.

## Current State
The check prompt rendering in `RuleChecker::check_file()` currently provides:
- `rule_content` (rendered rule template)
- `target_content` 
- `target_path`
- `language`

But does not provide `rule_name`.

## Required Changes
Add `rule_name` to the context when rendering the `.check` prompt in `checker.rs`:

```rust
check_context.set("rule_name".to_string(), rule.name.clone().into());
```

This should be added alongside the other context variables before rendering the `.check` prompt.

## Location
File: `swissarmyhammer-rules/src/checker.rs`
Function: `RuleChecker::check_file()`
Location: Around line 276 where `check_context` is built (STAGE 2)

## Acceptance Criteria
- [ ] `rule_name` is added to the `check_context` when rendering `.check` prompt
- [ ] The `.check.md` prompt can reference `{{rule_name}}` in its template
- [ ] Existing tests continue to pass
- [ ] Rule name appears in check prompt output when rendered



## Proposed Solution

After analyzing the code, I found:

1. The `.check.md` prompt template references `{{ rule_name }}` on line 45 for violation reporting
2. In `checker.rs:268-276`, the `check_context` is built with `rule_content`, `target_content`, `target_path`, and `language`
3. The `rule_name` is missing from this context

**Solution:**
Add one line to `checker.rs` at line 276 (after the language context is set):
```rust
check_context.set("rule_name".to_string(), rule.name.clone().into());
```

**Test Strategy:**
1. Create a test that verifies `rule_name` is included in the rendered `.check` prompt
2. Verify the rendered prompt contains the actual rule name when violations are reported
3. Ensure existing tests continue to pass

**Implementation Steps:**
1. Write a failing test that checks for `rule_name` in the check context
2. Add the `rule_name` to the context in `checker.rs:276`
3. Run tests to verify the fix
4. Run `cargo fmt` and `cargo clippy`



## Implementation Notes

Successfully implemented the fix using TDD:

1. **Test Created**: Added `test_check_prompt_includes_rule_name` in checker.rs:502-554
   - Test mimics the two-stage rendering process used in `check_file()`
   - Verifies that `rule_name` appears in the rendered `.check` prompt
   - Uses a specific test rule name "test-rule-name-123" to verify inclusion

2. **Fix Applied**: Added `rule_name` to check_context in checker.rs:277
   ```rust
   check_context.set("rule_name".to_string(), rule.name.clone().into());
   ```

3. **Test Results**:
   - All 150 tests in swissarmyhammer-rules pass
   - No clippy warnings
   - Code formatted correctly

## Changes Made

**File**: `/Users/wballard/github/sah/swissarmyhammer-rules/src/checker.rs`

**Line 277**: Added `rule_name` to the `check_context` during STAGE 2 rendering
**Lines 502-554**: Added comprehensive test for rule_name inclusion

## Verification

The `.check.md` prompt can now successfully reference `{{ rule_name }}` on line 45 when reporting violations, as the parameter is properly passed through the template context.
