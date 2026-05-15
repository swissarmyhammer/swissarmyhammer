---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffb280
project: spatial-nav
title: 'Enter on task card doesn''t drill in: missing snapshot on drillIn/drillOut + EntityCard topology mismatch'
---
## Symptom

Pressing Enter on a focused task card does nothing. Expected behavior: Enter drills into the card and lands focus on the first field (topmost-leftmost child — title chip or the equivalent).

Same class of bug exists on Escape (`drillOut` echoes `focused_fq` instead of climbing to the card from inside a field).

## Root cause — two bugs, both load-bearing

### Bug 1 — IPC carries no snapshot

`kanban-app/ui/src/lib/spatial-focus-context.tsx:431-443` defines `drillIn` and `drillOut` without building or passing a `snapshot`:

```ts
const drillIn: SpatialFocusActions["drillIn"] = async (fq, focusedFq) => {
  return await invoke<FullyQualifiedMoniker>("spatial_drill_in", {
    fq, focusedFq,
  });
};
const drillOut: SpatialFocusActions["drillOut"] = async (fq, focusedFq) => {
  return await invoke<FullyQualifiedMoniker>("spatial_drill_out", {
    fq, focusedFq,
  });
};
```

Compare to `focus` / `navigate` / `popLayer` in the same file (lines 346-361, 411-429) — all of those call `buildSnapshotForFocused(layerRegistriesRef, focusedFq)` and pass `snapshot` in the args.

The Rust handler `spatial_drill_in` at `kanban-app/src/commands.rs:2471-2480` declares `snapshot: Option<NavSnapshot>`. When `snapshot` is `None`, it short-circuits and **returns `focused_fq` unchanged**. AppShell's closure (`app-shell.tsx:344-363`) calls `setFocus(focused_fq)` — idempotent — visible behavior: "Enter does nothing." Same applies to drillOut.

The `tracing::debug!` instrumentation step 12/13 added on the four Tauri wrappers' `snapshot=None` short-circuit paths should be firing every time the user presses Enter — verify by tailing `log show --predicate 'subsystem == "com.swissarmyhammer.kanban"' --last 5m` while pressing Enter on a card.

### Bug 2 — EntityCard topology mismatch

`kanban-app/ui/src/components/entity-card.tsx:110` mounts as a `<FocusScope>` (leaf), but its body contains `<Field>` components which are themselves `<FocusZone>` (`fields/field.tsx:15`). This violates the kernel leaf invariant documented at `focus-scope.tsx:52-66` ("leaves MUST NOT contain further `<FocusScope>`/`<FocusZone>`").

Practical consequence: fields don't parent to the card in the snapshot — they parent to the column. So `drill_in(card_fq)` traversing `parent_zone` chains would find no descendants under the card. Even if Bug 1 were fixed, `drill_in` would return `focused_fq` because `navigate.rs:134-135` returns `focused_fq` when no children are found.

The doc-comment block at `entity-card.tsx:60-98` actively claims the card "registers as a `<FocusZone>`" — stale comment from before kanban-card `01KQJDYJ4SDKK2G8FTAQ348ZHG`'s reversion. The comment describes the intended architecture; the code doesn't match it.

Confirmation in `board-view.enter-drill-in.browser.test.tsx:13`: "The card is a leaf today (no zone children)..."

## Fix

Both fixes are needed in this order — snapshot first (otherwise Enter still never reaches the kernel correctly), then topology (otherwise the kernel returns the card's own FQM).

### Step 1 — pass snapshot through drillIn/drillOut

`kanban-app/ui/src/lib/spatial-focus-context.tsx:431-443` — mirror the pattern from `navigate` (line 355-361) and `focus` (line 346-352):

```ts
const drillIn: SpatialFocusActions["drillIn"] = async (fq, focusedFq) => {
  const snapshot = buildSnapshotForFocused(layerRegistriesRef, focusedFq);
  return await invoke<FullyQualifiedMoniker>("spatial_drill_in", {
    fq, focusedFq, snapshot,
  });
};

const drillOut: SpatialFocusActions["drillOut"] = async (fq, focusedFq) => {
  const snapshot = buildSnapshotForFocused(layerRegistriesRef, focusedFq);
  return await invoke<FullyQualifiedMoniker>("spatial_drill_out", {
    fq, focusedFq, snapshot,
  });
};
```

When the focused FQ isn't in any registered layer (transient unmount race), `snapshot` is `undefined` and the kernel echoes — same fallback as the other commands. That's correct behavior.

### Step 2 — promote EntityCard to FocusZone

`kanban-app/ui/src/components/entity-card.tsx:110` — change the mount from `<FocusScope>` to `<FocusZone>` so its `<Field>` children correctly nest as children in the snapshot's `parent_zone` graph. Update the stale doc-comment block at lines 60-98 to match (or just delete it — the code is the contract).

Verify by reading the snapshot in dev mode: `parent_zone` for each field FQM should now be the card's FQM, not the column's.

### Step 3 — instrument

The existing `tracing::debug!` at `commands.rs:2471-2480` (and the analog on `spatial_drill_out`) should fire ZERO times in normal use after the fix. If they fire on Enter, that means `buildSnapshotForFocused` returned undefined — which means the React-side `LayerScopeRegistry` doesn't know about the focused card's layer. That would be a third bug (registry not populated) — unlikely given step 1's coverage but worth confirming.

## Tests

Existing fixture `kanban-app/ui/src/components/board-view.enter-drill-in.browser.test.tsx` already pins both halves of the contract — verify it passes after both fixes. If it doesn't exist or doesn't cover both cases, add:

- **Drill-in positive**: focus a task card via click, press Enter, assert focus lands on the topmost-leftmost field's FQM.
- **Drill-out positive**: focus a field via click, press Escape, assert focus lands back on the parent card's FQM.
- **Drill-in IPC payload**: assert `invoke("spatial_drill_in", ...)` is called with a non-undefined `snapshot` carrying the card's parent-child structure.
- **Snapshot topology**: build the snapshot for a card-focused state, assert `parent_zone` for each field equals the card's FQM (not the column's).

## Acceptance criteria

- Enter on a focused task card moves focus to the first field of that card.
- Escape on a focused field moves focus back to the parent card.
- The `tracing::debug!` short-circuit logs on `spatial_drill_in` / `spatial_drill_out` fire zero times in normal kanban use (verified via `log show`).
- All `pnpm -C kanban-app/ui test` and `cargo test --workspace` green.
- The drill-in regression test fixture passes.

## Out of scope

- Generalizing the snapshot-build pattern across all actions (already consistent post-fix).
- Re-evaluating other components that mix `<FocusScope>` and `<FocusZone>` — only `EntityCard` is in scope here.

## Files

- `kanban-app/ui/src/lib/spatial-focus-context.tsx:431-443` — fix snapshot wiring on drillIn/drillOut.
- `kanban-app/ui/src/components/entity-card.tsx:60-98, 110` — promote to `<FocusZone>`, drop stale doc-comment.
- `kanban-app/ui/src/components/board-view.enter-drill-in.browser.test.tsx` — verify or extend test coverage.

#spatial-nav #bug