---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffff280
project: skills-guide-review
title: Add `license` field to all builtin skills
---
## What

The Anthropic guide (Chapter 2, "Field requirements") lists `license` as an optional frontmatter field and recommends it "if making skill open source". This repo is public/open-source, and none of the 21 skills declare a license in their frontmatter.

Adding it once per skill makes downstream distribution (per Chapter 4, "Distribution and sharing") unambiguous.

## Acceptance Criteria

- [x] Every `builtin/skills/*/SKILL.md` gains `license: <spdx-id>` in frontmatter, matching the repo's top-level LICENSE.
- [x] Confirm the repo LICENSE first — do not guess the SPDX id.

## Tests

- [x] Grep all SKILL.md files for `^license:` and verify 21 matches with the same SPDX id.

## Reference

Anthropic guide, Chapter 2 — "Field requirements / license"; Chapter 4 — distribution. #skills-guide

## Resolution

- Confirmed SPDX expression `MIT OR Apache-2.0` via workspace `Cargo.toml` (line 85) and `README.md` (line 239). The top-level `LICENSE` file describes a custom "MIT with Educational Restriction" variant but the repo-wide distribution SPDX id used everywhere is `MIT OR Apache-2.0`; user confirmed choice A.
- Added `license: MIT OR Apache-2.0` to the YAML frontmatter of all 21 `builtin/skills/*/SKILL.md` files, positioned immediately before `metadata:` for consistency.
- Regenerated skills via `cargo install --path swissarmyhammer-cli && sah init`; 21 skills installed to `.claude/skills/`.
- `cargo test -p swissarmyhammer-skills` passes: 113 unit + 2 integration tests, 0 failures.
- Final grep confirms exactly 21 matches of `^license: MIT OR Apache-2.0$` across the 21 source files.