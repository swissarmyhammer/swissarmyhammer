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

- [ ] `.check` prompt exists in builtin prompts
- [ ] Prompt has all required parameters defined
- [ ] Manual test renders correctly
- [ ] Prompt output format is clear and parseable
