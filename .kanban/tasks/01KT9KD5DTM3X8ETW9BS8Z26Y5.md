---
assignees:
- claude-code
depends_on:
- 01KT6SAAM6CR85YZD26JHSC87E
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffed80
project: short-ids
title: 'Short IDs: document short ids in kanban + task skills'
---
Update the kanban-facing skills so agents understand and use short ids instead of inventing ULID prefix abbreviations.

## Why
Agents instinctively abbreviate task ULIDs by PREFIX (e.g. `01KT6SA4`), which clusters and near-collides for tasks created in the same session. The skills should teach the agent to quote the canonical short id (`^<7char>` suffix) that now appears in tool output, and explain how task references resolve.

## Files (edit SOURCE, not generated)
- `builtin/skills/kanban/SKILL.md`
- `builtin/skills/task/SKILL.md`
- NOTE: `.skills/` is generated — never edit there; edit `builtin/skills/<name>/SKILL.md`. Regenerate if the build requires it.
- Check `builtin/skills/implement/SKILL.md` and `builtin/skills/finish/SKILL.md` too — anywhere that tells the agent to reference a task by id should mention the short id.

## Content to add
- What a short id is: lowercase last-7 Crockford chars of the ULID, shown as `^<short>` (e.g. `^rc9rb4g`). It is the canonical short handle; the full 26-char ULID stays the stored identity.
- Prefer quoting the short id (from the tool's `short_id` field) when referring to a task in prose/commits/chat — do NOT hand-abbreviate the ULID by prefix.
- How references resolve (forgiving input): full ULID, 7-char short id, `^<short>`, or a unique ULID prefix all resolve to the task; an ambiguous prefix errors. Display is always the short form.
- One concrete example showing the same task by full ULID and by `^<short>`.

## Acceptance
- kanban + task skills (and implement/finish where relevant) describe short ids and the "quote short id, don't prefix-abbreviate" guidance.
- Edits are in `builtin/skills/...`, not `.skills/`.

Depends on the tool/CLI API card (short ids must actually appear in output + resolve before the docs describe them).

## Review Findings (2026-06-05 06:16)

### Warnings
- [x] `builtin/_partials/short-ids.md` — The partial claims an ambiguous prefix "**errors as ambiguous** and lists every match — pick the intended short id from that list." That error UX does not exist at any agent-facing surface. The MCP dispatch resolver `resolve_task_ref` (`crates/swissarmyhammer-kanban/src/dispatch.rs`) collapses `ResolveResult::Ambiguous(_)` into a generic `KanbanError::TaskNotFound { id: raw }` — the candidate `Vec<TaskId>` is discarded, and the dispatch test `dispatch_ambiguous_prefix_returns_not_found` asserts exactly this ("must return a not-found error, not a match"). No surface lists the matches. An agent following the doc will expect a helpful match list on collision and instead get a plain "task not found" with nothing to pick from. Fix: soften the doc to match reality — e.g. "an ambiguous prefix does not resolve and is rejected (the tool reports the reference as not found); disambiguate by quoting the full short id." Drop the "lists every match" promise unless dispatch is changed to surface the candidate short ids in the error.
  - RESOLVED: Reworded line in `builtin/_partials/short-ids.md` to "A prefix that matches more than one task **does not resolve** — the tool reports the reference as not found (it does not list the matches), so disambiguate by quoting the full 7-char short id." Verified against `resolve_task_ref` in dispatch.rs which maps `ResolveResult::Ambiguous(_)` to `KanbanError::TaskNotFound`.

### Nits
- [x] `builtin/_partials/short-ids.md` — The resolution table lists the unique-prefix example as `01KT6SAM` (8 chars) while prose elsewhere calls it a "git-style ULID prefix"; that is fine, but consider noting that a prefix must be long enough to be unique on the board, since short same-session prefixes (the `01KT6SA` case the whole feature exists to avoid) are precisely the ambiguous ones. Minor — the surrounding text already implies this.
  - RESOLVED: Added to the same sentence: "A prefix only works when it is long enough to be unique on the board; the short same-session prefixes (e.g. `01KT6SA`) that this feature exists to avoid are exactly the ambiguous ones."

### Verification
- Re-ran the 2 render tests in swissarmyhammer-tools: `cargo test -p swissarmyhammer-tools --test tools_tests renders_short_id_guidance` → 2 passed, 0 failed (`test_kanban_skill_renders_short_id_guidance`, `test_task_skill_renders_short_id_guidance`). The partial still carries both required markers ("last 7 characters of the ULID", "hand-abbreviate").