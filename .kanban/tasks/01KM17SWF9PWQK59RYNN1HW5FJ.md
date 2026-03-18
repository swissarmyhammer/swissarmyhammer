---
assignees:
- claude-code
position_column: todo
position_ordinal: 8b80
title: '[NIT] card/SKILL.md: schema examples in skill tool don''t include arguments field'
---
## What

`swissarmyhammer-skills/src/schema.rs` generates the MCP schema examples for the `skill` tool. The "Activate a skill by name" example shows:

```json
{"op": "use skill", "name": "plan"}
```

Now that `arguments` is a supported optional field (used by `/card` and potentially all skills), the examples should include at least one case demonstrating it:

```json
{"op": "use skill", "name": "card", "arguments": "fix the login bug"}
```

Without this, LLMs using the tool schema may not discover that `arguments` is accepted.

File:
- `swissarmyhammer-skills/src/schema.rs` (`generate_skill_examples()`, line 20–41)

## Acceptance Criteria
- [ ] At least one example in `generate_skill_examples()` demonstrates passing `arguments`.

## Tests
- [ ] `cargo test -p swissarmyhammer-skills` passes after the change. #review-finding #nit