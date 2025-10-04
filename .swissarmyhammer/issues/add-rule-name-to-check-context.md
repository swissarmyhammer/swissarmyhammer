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
