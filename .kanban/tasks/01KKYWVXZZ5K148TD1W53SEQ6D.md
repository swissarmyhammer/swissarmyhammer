---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffcd80
title: Inject package version into skill template context
---
## What

The skill template rendering in `swissarmyhammer-tools/src/mcp/tools/skill/use_op.rs:57` creates an empty `TemplateContext::new()`. This means `{{version}}` in skill SKILL.md files would render as empty string. Populate it with `env!(\"CARGO_PKG_VERSION\")` so skills can use `{{version}}` in their frontmatter.

Similarly, agent template rendering in `claude-agent/src/agent.rs:445` creates `TemplateContext::new()`. Populate that too.

### Files to modify
- `swissarmyhammer-tools/src/mcp/tools/skill/use_op.rs` — line 57, set `version` on the template context
- `claude-agent/src/agent.rs` — line 445, set `version` on the template context

### Approach
Use the same pattern as `avp-common/src/validator/parser.rs:51` which injects `crate::VERSION`. Each crate already has `pub const VERSION: &str = env!(\"CARGO_PKG_VERSION\")` — use that or add it if missing.

## Acceptance Criteria
- [ ] `{{version}}` in a skill's SKILL.md resolves to the workspace package version (e.g., `0.9.2`)
- [ ] `{{version}}` in an agent's AGENT.md resolves to the workspace package version
- [ ] Existing `{% include %}` rendering still works (no regression)

## Tests
- [ ] Add unit test in `swissarmyhammer-tools` confirming skill template context includes `version`
- [ ] Add unit test in `claude-agent` confirming agent template context includes `version`
- [ ] Run `cargo nextest run -p swissarmyhammer-tools -p claude-agent` — all pass