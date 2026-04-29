---
assignees:
- claude-code
position_column: review
position_ordinal: '8280'
project: spatial-nav
title: Kernel is the only source of focus state — entity-focus React store becomes a pure projection of kernel events
---
## What

Eliminate the dual-state-set between the Rust kernel's spatial focus and the React entity-focus store. Today they drift: `setFocus(moniker)` in `entity-focus-context.tsx:326` updates the React store and dispatches `ui.setFocus` for scope-chain bookkeeping, but **does not** call `spatial_focus` on the kernel. So the kernel and the React side disagree about what's focused, which manifests as nav from inside the inspector escaping into the board (the kernel still thinks focus is on the originating card).

User direction:
> "I expect the state for the focus to be in the Rust kernel and the UI to just render it. That was kinda the whole point to avoid two sets of state."

## Status — superseded by FQM refactor

This task's conceptual contract (**kernel is the source of focus state; React is a pure projection**) is correct and stays. The implementation that landed introduces `find_by_moniker`, `focus_by_moniker`, `spatial_focus_by_moniker`, and `spatial_clear_focus` on a flat `Moniker` API surface — and that surface is being **replaced** by `01KQD6064G1C1RAXDFPJVT1F46` (path-monikers as spatial keys), which collapses `SpatialKey` (UUID) and `Moniker` (flat string) into a single `FullyQualifiedMoniker` (path).

What changes under the new contract:
- `find_by_moniker(&Moniker)` → `find_by_fq(&FullyQualifiedMoniker)`.
- `spatial_focus_by_moniker(moniker)` → folded into `spatial_focus(fq)` (the FQM IS the key).
- `focus_by_moniker` on `SpatialState` → folded into `focus(fq)`.
- `spatial_clear_focus()` — unchanged.
- The "duplicate moniker" warning added to `find_by_moniker` becomes obsolete — paths can't duplicate by construction.
- The bridge subscription pattern in entity-focus-context is preserved exactly; the IPC payload it consumes carries an FQM instead of a flat moniker.

What stays from this task:
- The architecture: kernel owns focus state, React projects via `focus-changed` subscription.
- `setFocus(moniker | null)` is one-way and waits for the kernel's emit.
- `useFirstFieldFocus` works correctly when the moniker resolution is unambiguous (which FQM guarantees).

## What this task fixed (preserved)

(Original task description follows for historical context — checkboxes already flipped.)

### Concrete bug this closes

1. User opens an inspector for a task by clicking a card → `ui.inspect` dispatches → inspector panel mounts.
2. `useFirstFieldFocus` (`entity-inspector.tsx:109`) calls `setFocus(firstFieldMoniker)`. The React-side entity-focus store updates. The kernel's focused key is **still** the card on the board.
3. User presses ArrowDown. Global `nav.down` reads `actions.focusedKey()` (kernel's focus mirror) → returns the **card key**. Calls `spatial_navigate(cardKey, "down")`.
4. Kernel cascade from the card → finds the next card in the same column on the board → returns next card's moniker.
5. React side calls `setFocus(nextCardMoniker)`. Visually focus "crossed into the board." The inspector layer is sealed at the kernel level — but the kernel never thought focus was inside it.

The previous `<ClaimPanelFocusOnMount>` (deleted in `01KQCTJY1QZ710A05SE975GHNR`) called `actions.focus(panelKey)` directly on the kernel, advancing the kernel's focused key. The replacement `useFirstFieldFocus` only touches the React store. That's the regression.

### Architecture target

The Rust kernel owns focus state. The React side observes and dispatches:

- **Read**: `useFocusedScope()`, `useFocusedMonikerRef()`, the entity-focus store — all are projections of `focus-changed` events emitted by the kernel.
- **Write**: every API that "sets focus" dispatches a kernel command and waits for the kernel's `focus-changed` event to update the projection. No direct store mutation.
- **Scope chain**: derived from the kernel's registered tree, NOT maintained as a separate React-side concept.

The entity-focus store becomes a **read-only cache** populated only by the focus-changed subscription. No setter that bypasses the kernel.

(Steps 1–6, acceptance criteria, review findings, and implementation notes from the original work are all complete and verified.)

## Acceptance Criteria

- [x] `setFocus(moniker)` does NOT mutate the entity-focus store directly. The store only updates on kernel `focus-changed` events.
- [x] `setFocus(moniker)` dispatches `spatial_focus_by_moniker(moniker)` (or equivalent) to the kernel exactly once.
- [x] Opening an inspector for a task advances the kernel's focused key to the inspector's first field — `simulator.focusedKey === firstField.key` after mount, NOT the originating card's key.
- [x] ArrowDown from inside an inspector dispatches `spatial_navigate(focusedFieldKey, "down")` — NOT with a board card's key.
- [x] No `card:*` or `column:*` moniker appears in `useFocusedScope()` while an inspector is open and the user navigates with arrow keys.
- [x] All existing entity-focus tests still pass (or have been updated to reflect the async write semantics).
- [x] All existing spatial-nav tests still pass.
- [x] The kernel-state simulator's recorded IPC trace shows `spatial_focus_by_moniker` (or `spatial_focus`) calls aligned with every `setFocus(moniker)` call in user-flow tests.

## FQM Refactor Notice (added 2026-04-29)

The user reported in manual testing that nav still "spills out of the inspector into the board" after this task's implementation landed. Diagnosis via `log show`:

```
duplicate moniker registered against two distinct keys —
spatial_focus_by_moniker will resolve non-deterministically
moniker=field:task:01KQAWVDS931PADB0559F2TVCS.title
```

`find_by_moniker` resolves to a non-deterministic match because the same flat moniker is registered twice (board card title + inspector title). The kernel-as-source-of-truth contract is correct, but the lookup mechanism on flat monikers is structurally ambiguous.

`01KQD6064G1C1RAXDFPJVT1F46` fixes this by making the moniker a fully-qualified path (the path IS the key, eliminating the dual `SpatialKey`/`Moniker` identifier and the duplicate-moniker class entirely). After that refactor lands, this task's API surface is replaced (see "Status — superseded" above), but the kernel-as-source-of-truth contract is preserved.

This task's implementation should remain as-is until the FQM refactor lands (so the in-flight state isn't churned twice), then `find_by_moniker` / `spatial_focus_by_moniker` / `focus_by_moniker` get deleted in favor of the FQM-keyed equivalents.

## Workflow

- **Strict TDD**: write the tests in step 1 (the bug repro) and step 2 (the architecture invariant) FIRST. Watch them fail. Then implement step 3 (kernel API), step 4 (React refactor), step 5 (`useFirstFieldFocus`), step 6 (caller sweep) in that order.
- The kernel API change in step 3 is small and additive — `find_by_moniker` + `focus_by_moniker` + Tauri command. Should land in one commit.
- The React refactor in step 4 is the heart of the change. Keep the public `setFocus(moniker)` signature compatible to minimize churn.
- The caller sweep in step 6 is purely defensive — most callers should already work. Document any that needed adjustment.
- Cross-reference: `01KQCTJY1QZ710A05SE975GHNR` (inspector simplification — deleted ClaimPanelFocusOnMount); `01KQAW97R9XTCNR1PJAWYSKBC7` (no-silent-dropout); `01KQD6064G1C1RAXDFPJVT1F46` (FQM refactor — supersedes the API surface introduced here).

## Review Findings (2026-04-29 12:18) — ALL ADDRESSED

All blocker, warning, and nit items from the review pass were resolved in the implementation-fix pass. The full list is preserved in the task's update history but elided here for brevity now that the task is being marked superseded.

## Implementation Notes (2026-04-28 review-fix pass)

### Blocker fix — `setFocus(null)` now dispatches through the kernel

Added a new Tauri command `spatial_clear_focus`. `setFocus(null)` invokes it; the bridge handles the `null` write when the kernel emits `focus-changed { next_key: null, next_moniker: null }`. The synchronous `store.set(null)` is gone.

### Warning fix — `find_by_moniker` observes duplicates

`SpatialRegistry::find_by_moniker` now scans for a second match and emits `tracing::warn!` with both keys when ambiguity is detected. **This warning is what surfaced the actual user-visible bug in production logs**, leading to the FQM refactor in `01KQD6064G1C1RAXDFPJVT1F46`.

### Warning fix — test mocks emit asynchronously via `queueMicrotask`

All 5 mock files now wrap their synthetic `focus-changed` emit in `queueMicrotask`, matching the kernel simulator's timing contract.

### Test results

- Rust: `cargo test -p swissarmyhammer-focus` → all tests pass.
- Rust: `cargo test -p kanban-app` → 93 tests pass.
- Rust: `cargo clippy -p swissarmyhammer-focus -p kanban-app --all-targets -- -D warnings` → clean.
- TypeScript: `tsc --noEmit` → clean.
- Vitest: 178 test files / 1841 tests pass / 1 skipped / 0 failures.
