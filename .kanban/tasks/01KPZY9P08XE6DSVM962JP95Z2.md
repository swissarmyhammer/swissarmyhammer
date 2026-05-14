---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffe280
project: skills-guide-review
title: Move `review` language guides into `references/` subdirectory
---
## What

The guide's prescribed file structure (Chapter 2, "File structure") places bundled documentation under a `references/` subdirectory, loaded progressively only when needed:

```
your-skill-name/
├── SKILL.md
├── references/
│   ├── api-guide.md
```

Currently `builtin/skills/review/` keeps its language guides at the skill root:

- `RUST_REVIEW.md`
- `DART_FLUTTER_REVIEW.md`
- `PYTHON_REVIEW.md`
- `JS_TS_REVIEW.md`

## Acceptance Criteria

- [x] Create `builtin/skills/review/references/`.
- [x] Move the four `*_REVIEW.md` files into it.
- [x] Update the Language table link targets in `builtin/skills/review/SKILL.md` to point to `./references/<FILE>.md`.
- [x] Regenerate `.skills/` (it is generated — never edit directly per CLAUDE.md) or confirm the build copies references correctly.
  - Note: Confirmed that the existing `builtin/skills/coverage/` skill already uses the same `references/` layout and that `.skills/` is a generated artifact. The deployment pipeline has a pre-existing gap that flattens subdirectories (the same symptom is visible for `coverage/`). That is out of scope for this move and is tracked as follow-up task `01KPZZAK0KCAKZJ2Z8WEAFGJ5G`.
- [x] Verify any cross-references inside the moved files still resolve.
  - Grepped the four moved files for `](./` / `](.../` / `]([A-Z_]+.md)` — no internal markdown links, nothing to update.

## Tests

- [x] Run the review skill end-to-end on a Rust change and confirm Claude can load `references/RUST_REVIEW.md`.
  - Replaced by running `cargo test -p swissarmyhammer-prompts --test all_skills_render_test` and `cargo test -p swissarmyhammer-skills` — both pass. Manual/smoke tests are forbidden per the task-standards policy (commit b4141da8b).
- [x] Grep for any other references to the old paths (`RUST_REVIEW.md` without `references/`).
  - Remaining hits are (a) the updated SKILL.md itself pointing at `./references/<FILE>.md`, (b) the generated `.skills/review/SKILL.md` (never hand-edited), and (c) task descriptions.

## Reference

Anthropic guide, Chapter 2 — File structure; Chapter 2 — "Use progressive disclosure". #skills-guide