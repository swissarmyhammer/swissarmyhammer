---
assignees:
- claude-code
depends_on:
- 01KKRRBR4K3JHZS2WZRD13V06X
position_column: done
position_ordinal: ffffffffa580
title: Update commit skill to run detected formatters
---
## What

The commit skill (`builtin/skills/commit/SKILL.md`) was already updated to include formatting steps in its Process section, but we should verify the `detected-projects` partial it includes actually works end-to-end now that project-type partials have formatting sections.

This card is about verifying the full chain: commit skill → `{% include \"_partials/detected-projects\" %}` → agent calls `detect projects` → gets rendered guidelines with formatting sections → runs the right formatters.

**Files:**
- `builtin/skills/commit/SKILL.md` — already updated, verify it renders correctly
- `builtin/_partials/detected-projects.md` — verify it renders through Liquid
- `builtin/_partials/project-types/*.md` — already have formatting sections (done earlier)

## Acceptance Criteria
- [ ] `commit` skill renders with `detected-projects` partial resolved
- [ ] Project-type partials with formatting sections are accessible through the rendering chain
- [ ] Manual test: invoke `/commit` skill and verify output includes formatting guidance

## Tests
- [ ] `cargo nextest run -p swissarmyhammer-prompts` (existing rendering tests cover this)
- [ ] Manual verification that `/commit` skill output includes project-type formatting info