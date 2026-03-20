---
assignees:
- claude-code
depends_on:
- 01KKYWVXZZ5K148TD1W53SEQ6D
position_column: done
position_ordinal: ffffffffc580
title: Replace hardcoded versions in skill and agent frontmatter with {{version}}
---
## What

All builtin skills have hardcoded `version:` in their YAML frontmatter (e.g., `version: "2.0"`, `version: "3.0"`). Agents have no version at all. Replace all of these with `version: "{{version}}"` so they resolve to the package version at render time — the same pattern validators already use.

### Files to modify (skills — `builtin/skills/*/SKILL.md`)
- `builtin/skills/coverage/SKILL.md` — `version: "3.0"` → `version: "{{version}}"`
- `builtin/skills/implement-loop/SKILL.md` — `version: "1.0"` → `version: "{{version}}"`
- `builtin/skills/test-loop/SKILL.md` — `version: "1.0"` → `version: "{{version}}"`
- `builtin/skills/plan/SKILL.md` — `version: "2.0"` → `version: "{{version}}"`
- `builtin/skills/code-context/SKILL.md` — `version: "1.0"` → `version: "{{version}}"`
- `builtin/skills/commit/SKILL.md` — `version: "1.0"` → `version: "{{version}}"`
- `builtin/skills/deduplicate/SKILL.md` — `version: "3.0"` → `version: "{{version}}"`
- `builtin/skills/test/SKILL.md` — `version: "3.0"` → `version: "{{version}}"`
- `builtin/skills/shell/SKILL.md` — `version: "1.0"` → `version: "{{version}}"`
- `builtin/skills/implement/SKILL.md` — `version: "3.0"` → `version: "{{version}}"`
- `builtin/skills/double-check/SKILL.md` — `version: "1.0"` → `version: "{{version}}"`
- `builtin/skills/review/SKILL.md` — `version: "3.0"` → `version: "{{version}}"`
- `builtin/skills/kanban/SKILL.md` — `version: "1.2"` → `version: "{{version}}"`
- `builtin/skills/lsp/SKILL.md` — `version: "1.0"` → `version: "{{version}}"`

### Files to modify (agents — `.agents/*/AGENT.md`)
Add `version: "{{version}}"` to the metadata section of each agent's frontmatter:
- `.agents/default/AGENT.md`
- `.agents/committer/AGENT.md`
- `.agents/explore/AGENT.md`
- `.agents/general-purpose/AGENT.md`
- `.agents/implementer/AGENT.md`
- `.agents/plan/AGENT.md`
- `.agents/planner/AGENT.md`
- `.agents/reviewer/AGENT.md`
- `.agents/test/AGENT.md`
- `.agents/tester/AGENT.md`

### Also update the test in `swissarmyhammer-skills/src/skill_loader.rs`
The two test cases have `version: "1.0"` in their fixtures — update to `version: "{{version}}"` or leave as-is since they test parsing, not rendering.

## Acceptance Criteria
- [ ] No skill has a hardcoded version number in its frontmatter
- [ ] Every agent AGENT.md includes `version: "{{version}}"` in frontmatter metadata
- [ ] Validators continue to use `{{version}}` (no change needed — already correct)
- [ ] All SKILL.md and AGENT.md files use the same `{{version}}` pattern

## Tests
- [ ] `cargo nextest run -p swissarmyhammer-skills` — all pass (frontmatter parsing still works with template syntax)
- [ ] `cargo nextest run -p swissarmyhammer-tools` — all pass
- [ ] Grep for hardcoded `version: "X.Y"` in builtin skills returns zero matches