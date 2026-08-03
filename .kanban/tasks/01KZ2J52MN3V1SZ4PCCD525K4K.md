---
assignees:
- claude-code
position_column: todo
position_ordinal: f280
title: Three committed .skills/ snapshots have drifted from builtin/skills/
---
## What

Three generated `.skills/` files are tracked in git and have drifted far from
their `builtin/skills/` sources:

- `apps/kanban-cli/.skills/kanban/SKILL.md`
- `apps/code-context-cli/.skills/code-context/SKILL.md`
- `apps/code-context-cli/.skills/lsp/SKILL.md`

`diff builtin/skills/kanban/SKILL.md apps/kanban-cli/.skills/kanban/SKILL.md`
reports dozens of differing paragraphs. Neither `apps/kanban-cli/build.rs` nor
`apps/code-context-cli/build.rs` writes them, so nothing regenerates them on a
build. They are a runtime deploy artifact that was committed once and left.

Found while card ^3y5n9g6 deleted the `llama-agent` crate:
`apps/kanban-cli/.skills/kanban/SKILL.md:19` still tells users the kanban board
is "the single source of truth across Claude Code and llama-agent sessions".
The `builtin/skills/kanban/SKILL.md` source was corrected there; the snapshot
could not be, because `.skills/` must never be hand-edited.

Decide and act:

- Regenerate all three through the real deploy path
  (`swissarmyhammer-skills::deploy`), which re-renders each SKILL.md from
  `parse_skill_md` -> `format_skill_md` with template variables resolved — a
  raw copy is NOT equivalent, and

- add a test that fails when a committed snapshot drifts from its source, or

- stop tracking `.skills/` and add it to `.gitignore`.

Pick one. A snapshot that nothing regenerates and nothing checks will drift
again.

### Subtasks

- [ ] Decide: regenerate + guard, or untrack.
- [ ] Apply the decision to all three files.

## Acceptance Criteria

- [ ] Either every committed `.skills/*/SKILL.md` matches what the deploy path
      produces from its `builtin/skills/` source, or no `.skills/` file is
      tracked.
- [ ] If they stay tracked, a test fails when one drifts.

## Tests

- [ ] Run `cargo nextest run -p swissarmyhammer-skills`.
- [ ] Run `cargo nextest run --workspace`.
#cleanup #docs #skills