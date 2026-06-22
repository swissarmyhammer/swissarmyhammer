---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvqrfkxktmdwefzjbcjrqakz
  text: |-
    Investigated — this is a review-engine hallucination, not a real bug.

    Evidence:
    - `grep -n 'fn emit_view_switch'` in crates/swissarmyhammer-kanban/src → exactly ONE definition, a free fn at scope_commands.rs line 312. Called once at line 603; the other two hits (lines 229, 581) are doc-comment references. No duplicate.
    - The cited location `scope_commands.rs:18` does not contain `emit_view_switch` at all.
    - A true double-definition would be a Rust E0428 compile error. `cargo check -p swissarmyhammer-kanban` → Finished clean (exit 0), and `cargo nextest run -p swissarmyhammer-kanban` is green (1402 passed). A duplicate could not compile.

    This is the same false-positive pattern the review engine produced this session for `depends_on_refs` (flagged confirmed:1 AND refuted:1, file is ground truth).

    Acceptance criteria status: there is a single source of truth (already), and the workspace builds clean with tests green. Nothing to change — closing as not-a-bug. No code change, so no commit.
  timestamp: 2026-06-22T13:31:55.699979+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffd680
title: 'Bug: emit_view_switch double-definition in swissarmyhammer-kanban/src/scope_commands.rs:18'
---
## What

The review engine's full-tree sweep flagged a genuine blocker: a double-definition of `emit_view_switch` in `crates/swissarmyhammer-kanban/src/scope_commands.rs:18`.

Discovered incidentally during the review of z3ax1jz (a UI-test card); out of scope there, captured here so it isn't lost.

## Acceptance Criteria
- [ ] Confirm the double-definition at `crates/swissarmyhammer-kanban/src/scope_commands.rs:18`
- [ ] Resolve it (remove/merge the duplicate) so there is a single source of truth
- [ ] Workspace builds clean; relevant tests green

#bug