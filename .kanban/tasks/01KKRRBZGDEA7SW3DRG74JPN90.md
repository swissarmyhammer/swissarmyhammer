---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffa480
title: Clean up duplicate strip_frontmatter in test files
---
## What

Two test files in `swissarmyhammer-prompts/tests/` have their own local `strip_frontmatter()` implementations instead of using the canonical one from `markdowndown::frontmatter::strip_frontmatter`.

**Files:**
- `swissarmyhammer-prompts/tests/detected_projects_inclusion_test.rs` — local `strip_frontmatter()` at line ~46
- `swissarmyhammer-prompts/tests/skills_rendering_test.rs` — local `strip_frontmatter()` at line ~47

**Approach:** Replace the local functions with `use markdowndown::frontmatter::strip_frontmatter;`. May need to add `markdowndown` as a dev-dependency of `swissarmyhammer-prompts` if not already present.

## Acceptance Criteria
- [ ] No local `strip_frontmatter()` in either test file
- [ ] Both test files use `markdowndown::frontmatter::strip_frontmatter`
- [ ] Tests still pass

## Tests
- [ ] `cargo nextest run -p swissarmyhammer-prompts`