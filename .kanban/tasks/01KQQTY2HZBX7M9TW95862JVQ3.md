---
assignees:
- claude-code
depends_on:
- 01KQQSXM2PEYR1WAQ7QXW3B8ME
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffff8380
project: spatial-nav
title: 'Spatial-nav #2: nav.drillIn = focus first child'
---
## Reference

Part of the spatial-nav redesign. Full design: **`01KQQSXM2PEYR1WAQ7QXW3B8ME`** — read it before starting.

**This component owns:** `nav.drillIn` (Enter key). Make it focus the focused scope's first child. On a leaf (no children), no-op.

**Contract (restated from design):**

> First child = the child whose rect is topmost; ties broken by leftmost. Children = registered scopes whose `parent_zone` is the focused scope's FQM.

> If focused scope has no children, return `focused_fq` (no-op, per no-silent-dropout). Do not "drill into nothing" via the keyboard — that's the consumer's job (e.g. focusing an inline editor) and is handled by `01KQQDXHANWGMBG872KZ3FZ86P`.

## Audit findings (2026-05-03)

`drill_in` lives in `src/registry.rs` (NOT `navigate.rs` — the task body's note "or wherever drill_in currently lives" applies; #6 only touches the rect-validation parts of registry.rs, no overlap with `drill_in`). The function ALREADY implements the full contract per **Option A**:

1. Honor `zone.last_focused` when it still resolves to a registered scope.
2. Otherwise fall back to first-child by `(top, left)` ordering.
3. Empty zone → echo `focused_fq`.
4. Leaf → echo `focused_fq`.
5. Unknown FQM → `tracing::error!` + echo `focused_fq`.

`tests/drill.rs` already covers all five test scenarios listed in the task (live `last_focused`, stale `last_focused`, no `last_focused` vertical layout, no children, leaf, unknown FQM, round-trip, plus inspector field-zone horizontal pills + empty field). 13/13 pass.

The implementation work was therefore zero — the contract was already met by prior work. This card's deliverable was the **README documentation** of the contract and the Option A vs B choice, plus the audit confirming the code matches the contract.

## What

### Files modified

- `swissarmyhammer-focus/README.md` — added new `## Drill in` section between `## Cardinal nav (geometric)` and `## Edge commands`. Documents:
  - Contract (children, first-child ordering, no-children fallthrough, leaf, unknown FQM)
  - **Option A choice** with rationale (last-focused primary preserves "I came back to where I was"; cold-start naturally degrades to first-child)
  - Cross-references to `src/registry.rs::drill_in`, `tests/drill.rs`, `tests/no_silent_none.rs`, and `01KQQDXHANWGMBG872KZ3FZ86P` (Tauri editor-focus fall-through)

### Files NOT modified (and why)

- `src/registry.rs::drill_in` — already implements Option A, no changes needed.
- `src/navigate.rs` — `drill_in` does not live here; nothing to do.
- `tests/drill.rs` — coverage matches contract; no test additions or rewrites needed.
- `kanban-app/ui/src/components/app-shell.tsx::buildDrillCommands` — `actions.drillIn(focusedFq, focusedFq)` wiring is intact; the result-equals-focused-FQM fall-through that `01KQQDXHANWGMBG872KZ3FZ86P` depends on still fires correctly.

## Test status

- `cargo test -p swissarmyhammer-focus --test drill` — 13 passed, 0 failed.
- `cargo test -p swissarmyhammer-focus --test no_silent_none` — 10 passed, 0 failed.
- `cargo test -p swissarmyhammer-focus --test readme_contract` — 1 passed, 0 failed.
- `cargo test -p swissarmyhammer-focus` (full crate) — all green.
- `cargo clippy -p swissarmyhammer-focus --all-targets -- -D warnings` — clean.

## Acceptance Criteria

- [x] `nav.drillIn` on a scope with children focuses the first child (topmost-then-leftmost).
- [x] `nav.drillIn` on a leaf is a no-op (returns focused FQM).
- [x] Last-focused memory behaviour is documented (Option A) and tested.
- [x] `drill.rs` integration tests pass; no behavioural change required (rationale: existing implementation already matched the contract).
- [x] README "## Drill in" section captures the contract.
- [x] `cargo test -p swissarmyhammer-focus` passes.
- [x] `01KQQDXHANWGMBG872KZ3FZ86P` (drill into editor on Enter) still works — the editor-focus fall-through happens AFTER `drill_in` returns the focused FQM (verified via `app-shell.tsx::buildDrillCommands`).

## Workflow

- Use `/tdd`. Audit current `drill_in` first to know whether you're refactoring or implementing. Write the first-child unit tests, decide Option A vs B, implement, sweep `drill.rs`.
#spatial-nav-redesign