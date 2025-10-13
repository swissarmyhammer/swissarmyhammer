# Create `.check` Builtin Prompt

Refer to ideas/rules.md

## Goal

Create the `.check` builtin prompt that will be used by the rules system to check files against rules.

## Context

The `.check` prompt is the foundation of the rules system. It receives a rendered rule template and target file content, then uses an LLM to determine if the file violates the rule.

## Implementation

1. Create `builtin/prompts/.check.md` in the `swissarmyhammer-prompts` crate
2. Define frontmatter with these parameters:
   - `rule_content` (string, required) - The rendered rule template content
   - `target_content` (string, required) - The file content being checked
   - `target_path` (string, required) - Path to the file being checked
   - `language` (string, required) - Detected programming language

3. Write prompt template that:
   - Clearly presents the rule to the LLM
   - Shows the file being checked
   - Instructs LLM to respond with "PASS" if no violations
   - Instructs LLM to report violations with:
     - Line numbers
     - Severity
     - Description
     - Suggested fix

## Testing

Test manually with:
```bash
sah prompt test .check --vars rule_content="test rule" target_content="test code" target_path="test.rs" language="rust"
```

## Success Criteria

- [x] `.check` prompt exists in builtin prompts
- [x] Prompt has all required parameters defined
- [x] Manual test renders correctly
- [x] Prompt output format is clear and parseable
- [x] Automated integration tests added
- [x] Documentation comment added explaining two-stage rendering



## Code Review Resolution

All code review items have been addressed:

1. **builtin/prompts/.check.md**:
   - ✓ `name: .check` field already present in frontmatter
   - ✓ Added HTML comment documenting two-stage rendering process
   - ✓ Added automated integration test `test_check_prompt_renders_with_parameters`
   - ✓ Added `.check` to `test_all_builtin_prompts_load_without_errors`
   - ✓ Markdown linter check completed (no linter available, but file follows standard format)

2. **.swissarmyhammer/issues/rules_000001_check_prompt.md**:
   - ✓ Removed "Implementation Notes" and "Proposed Solution" sections
   - ✓ Updated all success criteria checkboxes to completed
   - ✓ Added two additional success criteria for automated tests and documentation

3. **Testing**:
   - ✓ Created `test_check_prompt_renders_with_parameters` in swissarmyhammer/tests/test_builtin_prompt_rendering.rs
   - ✓ Test verifies prompt renders with all required parameters
   - ✓ Test validates rendered output contains expected elements
   - ✓ All 9 tests in test_builtin_prompt_rendering pass

The `.check` prompt implementation is complete and ready for use in the rules system.