---
assignees:
- claude-code
depends_on:
- 01KQQSXM2PEYR1WAQ7QXW3B8ME
position_column: todo
position_ordinal: d080
project: spatial-nav
title: 'Spatial-nav #2: nav.drillIn = focus first child'
---
## Reference

Part of the spatial-nav redesign. Full design: **`01KQQSXM2PEYR1WAQ7QXW3B8ME`** — read it before starting.

**This component owns:** `nav.drillIn` (Enter key). Make it focus the focused scope's first child. On a leaf (no children), no-op.

**Contract (restated from design):**

> First child = the child whose rect is topmost; ties broken by leftmost. Children = registered scopes whose `parent_zone` is the focused scope's FQM.

> If focused scope has no children, return `focused_fq` (no-op, per no-silent-dropout). Do not "drill into nothing" via the keyboard — that's the consumer's job (e.g. focusing an inline editor) and is handled by `01KQQDXHANWGMBG872KZ3FZ86P`.

## What

### Files to modify

- `swissarmyhammer-focus/src/navigate.rs` (or wherever `drill_in` currently lives — review first):
  - Implement `drill_in` as: find children of `focused_fq` (entries whose `parent_zone == focused_fq`); if any, return the first by topmost-then-leftmost geometric ordering; if none, return `focused_fq`.
  - **Audit the current `drill_in` implementation** — it may already do this, or it may consult last-focused memory. Last-focused memory is a related-but-separate concern; this task does NOT remove it. If `drill_in` currently returns the *last-focused leaf inside the zone* rather than the *first child*, decide:
    - **Option A:** Keep last-focused memory as the primary; first-child is the fallback when no last-focused is recorded.
    - **Option B:** Make first-child the primary; last-focused is a separate op (`nav.resume`?).
    - Recommend Option A — preserves the user's mental "I came back to where I was" expectation. Document the choice in the README.

- `swissarmyhammer-focus/README.md`:
  - Add / update a "## Drill in" section describing the contract and the first-child / last-focused interaction.

- `kanban-app/ui/src/components/app-shell.tsx`:
  - Confirm `buildDrillCommands` (line 346) still routes Enter → `actions.drillIn(focusedFq, focusedFq)` correctly. The "no descent" handling stays as-is so the editor-focus extension in `01KQQDXHANWGMBG872KZ3FZ86P` keeps working.

### Tests

- **Unit test in `swissarmyhammer-focus/src/navigate.rs::tests` or new `tests/drill_first_child.rs`**:
  - Focused scope with no children → `drill_in` returns focused FQM (no-op).
  - Focused scope with one child → `drill_in` returns the child's FQM.
  - Focused scope with multiple children laid out horizontally → `drill_in` returns the leftmost (topmost-then-leftmost rule, same Y).
  - Focused scope with multiple children laid out vertically → `drill_in` returns the topmost.
  - Focused scope with last-focused memory recorded → behaviour matches the chosen Option A or B.
- **Existing test `swissarmyhammer-focus/tests/drill.rs`** — update to reflect the new contract; document any behavioural change in the commit message.
- Run `cargo test -p swissarmyhammer-focus drill` and confirm green.

## Acceptance Criteria

- [ ] `nav.drillIn` on a scope with children focuses the first child (topmost-then-leftmost).
- [ ] `nav.drillIn` on a leaf is a no-op (returns focused FQM).
- [ ] Last-focused memory behaviour is documented (Option A or B) and tested.
- [ ] `drill.rs` integration tests pass; any updates have rationale in the commit message.
- [ ] README "## Drill in" section captures the contract.
- [ ] `cargo test -p swissarmyhammer-focus` passes.
- [ ] `01KQQDXHANWGMBG872KZ3FZ86P` (drill into editor on Enter) still works — the editor-focus fall-through happens AFTER `drill_in` returns the focused FQM.

## Workflow

- Use `/tdd`. Audit current `drill_in` first to know whether you're refactoring or implementing. Write the first-child unit tests, decide Option A vs B, implement, sweep `drill.rs`.
#spatial-nav-redesign