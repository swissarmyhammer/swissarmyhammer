---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffff780
project: spatial-nav
title: 'Add &lt;Pressable&gt; primitive: FocusScope leaf + button + Enter/Space activation, then migrate every icon button'
---
## What

Icon buttons across the UI are inconsistently wired into the spatial-nav and keyboard-activation contracts:

  - `nav-bar.tsx::ui:navbar.inspect` (the "i" Info button), `ui:navbar.search`, and the per-card `card.inspect:{id}` button in `entity-card.tsx::InspectButton` (lines 309–337) — all wrap a `<button onClick={…}>` in a `<FocusScope>`. That makes them keyboard-focusable, but pressing Enter does NOTHING. Today Enter dispatches `nav.drillIn` from `app-shell.tsx::buildDrillCommands` (lines 346–392); the kernel echoes the focused FQM for a leaf scope with no zone children → `setFocus` is idempotent → visible no-op. The button's `onClick` is never invoked from a keyboard. Per the inline note in app-shell.tsx line 333: "Leaves with an editor handle Enter via their own scope-level command (e.g. ...)" — that pattern exists but no one is using it for icon buttons.
  - `perspective-tab-bar.tsx::AddPerspectiveButton` (the "+" Add button at line 491), `FilterButton` (per-tab filter icon at line 781), and `GroupButton` (group icon, equivalent shape) — these are bare `<button>` with no `<FocusScope>` wrapper. Keyboard users cannot focus them at all; `Tab` / arrows skip past them.
  - The left-nav view buttons (`left-nav.tsx`) and the BoardSelector tear-off ("Open Board" affordance) — same shape: `<button>` without spatial registration, OR registered as a scope but no Enter binding.

The user-visible bug: hitting Enter on a focused icon button does nothing, when the affordance unambiguously suggests it should activate the button.

The fix the user is asking for is a primitive — a `<Pressable>` React component that bundles **the three concerns every actionable icon button must satisfy**:

  1. Mount a `<FocusScope>` leaf so the spatial-nav graph can navigate to it.
  2. Render a `<button type="button">` (or, via `asChild`, an arbitrary host like a Radix `<TooltipTrigger asChild>`).
  3. Register a scope-level `CommandDef` so `Enter` (vim / cua) and `Space` (cua) on the focused leaf invoke the same `onPress` callback as the button's `onClick`.

Once the primitive exists, every site listed above migrates to use it. Inconsistency goes away, and the contract is enforced at the component level (the only way to render an actionable icon button is through `<Pressable>`).

### Approach

Single new file plus targeted call-site migrations.

1. Create `kanban-app/ui/src/components/pressable.tsx`. Component shape:

   ```tsx
   export interface PressableProps extends Omit<HTMLAttributes<HTMLButtonElement>, "onClick"> {
     moniker: SegmentMoniker;        // Required — composes under parent FQM.
     onPress: () => void;             // Single source of truth for activation.
     ariaLabel: string;               // Required — every icon button needs an aria-label.
     asChild?: boolean;               // Default false. When true, renders children directly (Radix Slot pattern) so a TooltipTrigger or other slot can host the button props.
     disabled?: boolean;              // Disabled state suppresses both onClick and Enter/Space activation.
   }
   ```

   Internals:
   - Wrap children/host in `<FocusScope moniker={moniker} commands={pressCommands}>`.
   - `pressCommands` is memoized: a single CommandDef `{ id: "pressable.activate", name: "Activate", keys: { vim: "Enter", cua: "Enter" }, execute: () => { if (!disabled) onPress() } }` — plus a parallel CommandDef for `cua: "Space"` if Space activation is desired (Web/CUA convention is both). Two separate CommandDefs because `keys` per command is one entry per keymap.
   - When `asChild` is false (default): render `<button type="button" onClick={onPress} aria-label={ariaLabel} disabled={disabled} {...rest}>{children}</button>` inside the FocusScope.
   - When `asChild` is true: render `<Slot>` (from `@radix-ui/react-slot`) so the parent (e.g. `<TooltipTrigger asChild>`) can be the host. The slot still receives `onClick`, `aria-label`, `disabled` so any `<button>` underneath gets them.
   - Forward a `ref` to the host element.

2. Migrate the listed sites. **Each** site replaces its `<FocusScope>?<button onClick=…>…</button></FocusScope>?` shape with `<Pressable moniker={…} ariaLabel={…} onPress={…}>…</Pressable>` (or `<Tooltip><TooltipTrigger asChild><Pressable asChild …>…</Pressable></TooltipTrigger>…</Tooltip>` when wrapped in a tooltip):
   - `kanban-app/ui/src/components/nav-bar.tsx` — `ui:navbar.inspect` (Info, lines 97–116), `ui:navbar.search` (lines 133–150). The `ui:navbar.board-selector` is being reshaped under the scope-is-leaf task `01KQJDYJ4SDKK2G8FTAQ348ZHG`; coordinate by leaving the BoardSelector trigger to that task.
   - `kanban-app/ui/src/components/entity-card.tsx::InspectButton` (lines 309–337) — moniker stays `card.inspect:${entityId}`. (Note: `DragHandle` is being demoted to non-scope under task `01KQM9478XFMCBBWHQN6ARE524`; do NOT migrate it — it has no keyboard story.)
   - `kanban-app/ui/src/components/perspective-tab-bar.tsx::AddPerspectiveButton` (line 491) — assign a fresh moniker `ui:perspective-bar.add`. This adds keyboard reachability where there was none before.
   - `kanban-app/ui/src/components/perspective-tab-bar.tsx::FilterButton` (line 781) — moniker `perspective_tab.filter:{id}` (entity-disambiguated like `card.inspect:{id}`).
   - `kanban-app/ui/src/components/perspective-tab-bar.tsx::GroupButton` (parallel to FilterButton — locate via Grep on the same file) — moniker `perspective_tab.group:{id}`.
   - `kanban-app/ui/src/components/left-nav.tsx` — view-button click sites (the user said "nav buttons"). Each view button gets `<Pressable moniker="ui:leftnav.view:${viewId}" …>`. Note these are already partially in the spatial graph after commit `c01f3ed38` ("register view buttons with the spatial-nav kernel"); if so, swap to Pressable to gain Enter activation.
   - "Open window" — locate in `BoardSelector` (the tear-off affordance). Migrate the trigger that opens the new window.

3. Document the contract in the new file's docstring: every actionable icon button must use `<Pressable>`. The exception list is "purely decorative" (no `onPress`) and "mouse-only-by-design" (e.g. drag handle — see task `01KQM9478XFMCBBWHQN6ARE524`).

### Sizing note

Migrating every site in one task would exceed the 5-files-touched limit. Original scope: build the primitive PLUS migrate two reference sites. **Scope was expanded on reopen** — see "Reopened: scope expansion" below.

## Acceptance Criteria
- [x] `kanban-app/ui/src/components/pressable.tsx` exists. Exports `Pressable` and `PressableProps`. Props include `moniker`, `onPress`, `ariaLabel`, optional `asChild`, optional `disabled`, plus passthrough HTML button attributes.
- [x] `<Pressable moniker="…" ariaLabel="…" onPress={fn}>…</Pressable>` mounts a `<FocusScope>` leaf with two scope-level CommandDefs: vim/cua `Enter` and cua `Space`. Both invoke `onPress` exactly once when the leaf is focused and the key is pressed.
- [x] `<button onClick>` activation continues to work (mouse / pointer); `onPress` fires identically through both paths.
- [x] `disabled={true}` suppresses both `onClick` and the keyboard activation CommandDefs (the `execute` closures short-circuit).
- [x] `asChild` mode renders via Radix `<Slot>` so it composes with `<TooltipTrigger asChild>` without an extra `<button>` wrapper.
- [x] `nav-bar.tsx::ui:navbar.inspect` is migrated to `<Pressable moniker={asSegment("ui:navbar.inspect")} ariaLabel="Inspect board" onPress={…}>` inside the existing `<Tooltip>`/`<TooltipTrigger asChild>`. With nav-bar focused on the Info button, pressing `Enter` dispatches the inspect command exactly once.
- [x] `perspective-tab-bar.tsx::AddPerspectiveButton` is migrated to `<Pressable moniker={asSegment("ui:perspective-bar.add")} ariaLabel="Add perspective" onPress={handleAdd}>` — gaining keyboard reachability AND Enter/Space activation that today does not exist.
- [x] No regressions: the eventdriven-nav contract holds (no extra IPC fetches per Enter); existing perspective-bar / nav-bar tests stay green.

## Tests
- [x] Add `kanban-app/ui/src/components/pressable.test.tsx` (jsdom or browser, mirror harness from `entity-card.spatial.test.tsx`):
  - Test 1 — clicking the rendered button calls `onPress` once.
  - Test 2 — focusing the leaf and dispatching `Enter` (via the spatial-nav stack's CommandDef pipeline) calls `onPress` once.
  - Test 3 — focusing the leaf and dispatching `Space` (cua) calls `onPress` once.
  - Test 4 — `disabled={true}`: clicking the button does NOT call `onPress`; pressing Enter does NOT call `onPress`.
  - Test 5 — `asChild={true}` wrapped in `<TooltipTrigger asChild>`: only one `<button>` renders in the DOM (no double-button); both click and Enter still fire `onPress`.
  - Test 6 — registers as `spatial_register_scope` with the supplied `moniker` segment (mock `mockInvoke` like `nav-bar.scope-leaf.spatial.test.tsx`).
- [x] Update `kanban-app/ui/src/components/nav-bar.spatial-nav.test.tsx` (or add a sibling `nav-bar.inspect-enter.spatial.test.tsx`): seed focus on `ui:navbar.inspect`, dispatch keydown Enter, assert `mockInvoke("dispatch_command", { cmd: "ui.inspect", … })` was called exactly once with the board's moniker.
- [x] Add `kanban-app/ui/src/components/perspective-tab-bar.add-enter.spatial.test.tsx`: seed focus on `ui:perspective-bar.add`, dispatch keydown Enter, assert the add-perspective dispatch fired exactly once.
- [x] Existing tests stay green: `nav-bar.test.tsx`, `nav-bar.spatial-nav.test.tsx`, `nav-bar.focus-indicator.browser.test.tsx`, `entity-card.scope-leaf.spatial.test.tsx`, `perspective-tab-bar.spatial-nav.test.tsx`, `perspective-bar.spatial.test.tsx`.
- [x] Run `cd kanban-app/ui && pnpm vitest run src/components/pressable src/components/nav-bar src/components/perspective-tab-bar` and confirm green.

## Workflow
- Use `/tdd` — write `pressable.test.tsx` first (six failing assertions), then build `pressable.tsx`, then add the two migration tests, then perform the two migrations and confirm everything green. Follow-up tasks for the remaining migration sites (`FilterButton`/`GroupButton` in perspective-tab-bar, left-nav view buttons, BoardSelector "Open window" trigger, navbar Search button) get filed once the primitive's API is settled.

## Resolution

Implemented as specified.

**Files changed:**
- `kanban-app/ui/src/components/pressable.tsx` (new) — the primitive.
- `kanban-app/ui/src/components/pressable.test.tsx` (new) — 6 unit tests (click, Enter, Space, disabled, asChild, register).
- `kanban-app/ui/src/components/nav-bar.tsx` — migrated `ui:navbar.inspect` to `<Pressable asChild>`.
- `kanban-app/ui/src/components/nav-bar.inspect-enter.spatial.test.tsx` (new) — end-to-end Enter test.
- `kanban-app/ui/src/components/perspective-tab-bar.tsx` — migrated `AddPerspectiveButton` to `<Pressable asChild>` with new moniker `ui:perspective-bar.add`.
- `kanban-app/ui/src/components/perspective-tab-bar.add-enter.spatial.test.tsx` (new) — end-to-end Enter test.

**Tests:** Targeted suite (`pnpm vitest run src/components/pressable src/components/nav-bar src/components/perspective-tab-bar`) — 15 files, 113 tests, all green. Full suite (`pnpm vitest run`) — 195 files, 1917 tests, all green. `pnpm tsc --noEmit` — zero errors.

**Reopen migrations (2026-05-03):**
- `kanban-app/ui/src/components/entity-card.tsx::InspectButton` — migrated to `<Pressable asChild>` inside the existing `<TooltipTrigger asChild>`. Outer standalone `<FocusScope>` removed (Pressable provides one). Inner `<button>` keeps `onClick={(e) => e.stopPropagation()}` so click does not bubble to the card zone's `spatial_focus`. Removed the now-unused `FocusScope` import.
- `kanban-app/ui/src/components/column-view.tsx::AddTaskButton` — migrated to `<Pressable asChild>` with new moniker `ui:column.add-task:${columnId}`. Pre-migration the "+" button had no `<FocusScope>` at all — keyboard users could not reach it. The migration adds keyboard reachability AND Enter / Space activation in one step. Added `Pressable` import.
- `kanban-app/ui/src/components/entity-card.inspect-enter.spatial.test.tsx` (new) — pins (a) Enter on `card.inspect:{id}` dispatches `ui.inspect` once with card moniker, (b) clicking (i) dispatches once and does NOT bubble to the card zone's `spatial_focus`.
- `kanban-app/ui/src/components/column-view.add-task-enter.spatial.test.tsx` (new) — pins (a) Enter on `ui:column.add-task:{id}` invokes `onAddTask(columnId)` exactly once and seeds `spatial_focus(columnFq)` exactly once, (b) clicking the "+" preserves the same `onAddTask` + focus behavior.

**Tests after reopen:** Targeted suite (`pnpm vitest run src/components/entity-card src/components/column-view src/components/pressable`) — 14 files, 114 tests, all green. Surrounding regression suites (`nav-bar`, `perspective-tab-bar`, `perspective-bar`) — 15 files, 116 tests, all green. Full suite (`pnpm vitest run`) — 197 files, 1921 tests, 1 skipped, all green. `pnpm tsc --noEmit` — zero errors.

## Reopened: scope expansion (2026-05-03)

User feedback after the first pass: pressing Enter on the (i) Info button **on a task card** still does nothing, and the "+" Add-task button **in column headers** is not reachable by keyboard at all (no FocusScope, no Pressable). Both must be fixed under this task — they are the user-visible affordances most directly affected by the Pressable contract, and deferring them to a follow-up loses the user trust the primitive was meant to earn.

Two additional migrations land here:

### 1. `entity-card.tsx::InspectButton` — the (i) Info button on every task card

Current shape (`kanban-app/ui/src/components/entity-card.tsx` ~313–341):

```tsx
function InspectButton({ entityId, moniker }) {
  const dispatch = useDispatchCommand("ui.inspect");
  return (
    <FocusScope moniker={asSegment(`card.inspect:${entityId}`)}>
      <Tooltip>
        <TooltipTrigger asChild>
          <button type="button" aria-label="Inspect" onClick={(e) => { e.stopPropagation(); dispatch({ target: moniker }).catch(console.error); }}>
            <Info className="h-3.5 w-3.5" />
          </button>
        </TooltipTrigger>
        <TooltipContent>Inspect</TooltipContent>
      </Tooltip>
    </FocusScope>
  );
}
```

Already a FocusScope but bare `<button>` inside — keyboard focus works, Enter does nothing (the original bug class). Migrate to `<Pressable asChild>` inside the existing `<TooltipTrigger asChild>`:

```tsx
<Pressable
  asChild
  moniker={asSegment(`card.inspect:${entityId}`)}
  ariaLabel="Inspect"
  onPress={() => dispatch({ target: moniker }).catch(console.error)}
>
  <button type="button" /* className stays */ onClick={(e) => e.stopPropagation()}>
    <Info className="h-3.5 w-3.5" />
  </button>
</Pressable>
```

Note: the `e.stopPropagation()` is preserved on the inner `<button>` `onClick` because pointer activation must not bubble to the card's own click handler. `onPress` carries the dispatch; the bare `onClick={(e) => e.stopPropagation()}` is preserved separately because the parent card's pointer handler relies on it. Verify Pressable composes this correctly (Radix `Slot` composes event handlers via `composeEventHandlers`). If composition does not reach the inner click handler in `asChild` mode, the migration MAY require a small adjustment — **investigate and document in this Resolution section before changing Pressable's API**.

The outer `<FocusScope>` wrapper is removed (Pressable provides one).

### asChild composition investigation findings (2026-05-03)

Investigated the work-plan point 4 question: in `entity-card.tsx::InspectButton`, the inner `<button>`'s `onClick={(e) => e.stopPropagation()}` must continue to suppress propagation to the parent card zone's onClick after migration to `<Pressable asChild>`. Read Radix Slot's `mergeProps` (`@radix-ui/react-slot/dist/index.js`):

```js
if (slotPropValue && childPropValue) {
  overrideProps[propName] = (...args) => {
    const result = childPropValue(...args);  // child handler runs first
    slotPropValue(...args);                   // then slot handler
    return result;
  };
}
```

For the chain `<TooltipTrigger asChild><Pressable asChild onPress={dispatch}><button onClick={(e) => e.stopPropagation()}>`, Radix Slot composes `onClick` such that the inner `<button>`'s handler runs FIRST (`e.stopPropagation()` lands), then Pressable's `handleClick` runs (which fires `onPress` → dispatch). Both run synchronously inside the same React event handler invocation, before any propagation reaches the parent card `<FocusZone>`'s onClick (which would otherwise call `spatial_focus(cardFq)`). Propagation is stopped before the card zone's onClick sees the event.

**Conclusion: the migration is safe. Pressable's API does NOT need to change.** The slot composition order preserves both behaviors. The new test `entity-card.inspect-enter.spatial.test.tsx` test (b) pins this — clicking (i) must dispatch `ui.inspect` once AND must not trigger `spatial_focus(cardFq)` from the card zone's bubbling click handler.

### 2. `column-view.tsx::AddTaskButton` — the "+" button in every column header

Current shape (`kanban-app/ui/src/components/column-view.tsx` ~707–739):

```tsx
function AddTaskButton({ columnId, columnName, columnFq, onAddTask, setFocus }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button type="button" aria-label={`Add task to ${columnName}`} onClick={() => { if (columnFq) setFocus(columnFq); onAddTask(columnId); }}>
          <Plus className="h-4 w-4" />
        </button>
      </TooltipTrigger>
      <TooltipContent>{`Add task to ${columnName}`}</TooltipContent>
    </Tooltip>
  );
}
```

No FocusScope at all — keyboard cannot reach this. Migrate to `<Pressable asChild>` with a fresh per-column moniker:

```tsx
<Pressable
  asChild
  moniker={asSegment(`ui:column.add-task:${columnId}`)}
  ariaLabel={`Add task to ${columnName}`}
  onPress={() => { if (columnFq) setFocus(columnFq); onAddTask(columnId); }}
>
  <button type="button" /* className stays */>
    <Plus className="h-4 w-4" />
  </button>
</Pressable>
```

Wrap inside the existing `<TooltipTrigger asChild>` (so the chain is `Tooltip > TooltipTrigger asChild > Pressable asChild > button`).

Moniker convention: `ui:column.add-task:{columnId}` — entity-disambiguated like `card.inspect:{id}`, namespaced under `ui:column.*` so future column-level icon buttons (filter, sort, etc.) can compose under the same prefix.

### Additional Acceptance Criteria
- [x] `entity-card.tsx::InspectButton` migrated to `<Pressable asChild>`. The outer standalone `<FocusScope>` wrapper is removed (Pressable provides the FocusScope). With focus on a card's (i) button, pressing `Enter` dispatches `ui.inspect` exactly once with the card's moniker as target.
- [x] `column-view.tsx::AddTaskButton` migrated to `<Pressable asChild>` with moniker `ui:column.add-task:${columnId}`. With focus on the column header "+" button, pressing `Enter` invokes `onAddTask(columnId)` exactly once and seeds focus to the column FQM (matches today's pointer-click behavior).
- [x] Click activation on both buttons continues to work identically (no regression for mouse users).
- [x] On the card, `e.stopPropagation()` semantics are preserved so clicking the (i) does not also trigger the card's own click handler. Verify the `asChild`/Slot composition keeps the inner `onClick` intact alongside `onPress`.
- [x] No new IPC fetches per Enter (eventdriven-nav contract).

### Additional Tests
- [x] Add `kanban-app/ui/src/components/entity-card.inspect-enter.spatial.test.tsx` mirroring `nav-bar.inspect-enter.spatial.test.tsx`: render a card with focus seeded on `card.inspect:{id}`, dispatch keydown Enter, assert exactly one `dispatch_command("ui.inspect", { target: cardMoniker })` invocation, and assert the parent card's click handler is NOT invoked (no propagation regression).
- [x] Add `kanban-app/ui/src/components/column-view.add-task-enter.spatial.test.tsx`: render a column with focus seeded on `ui:column.add-task:{columnId}`, dispatch keydown Enter, assert `onAddTask(columnId)` was invoked exactly once and `setFocus(columnFq)` was called once with the column's FQM.
- [x] Update / extend any existing column-view spatial tests so they account for the new `ui:column.add-task:*` leaves under each column's FQM.
- [x] Existing tests stay green.

### Follow-up housekeeping
The follow-up audit task `01KQPZAFSPJEMHMKRSQGPD0JM6` was filed when this task was first closed. After the reopen, update it: REMOVE `entity-card.tsx::InspectButton` from its scope (it lands under this task now). The remaining sites in that follow-up are: navbar Search button, perspective-tab-bar `FilterButton`/`GroupButton`, `left-nav.tsx` view buttons, BoardSelector "Open window" trigger.

## Review Findings (2026-05-03 08:48)

Reopened-scope review (entity-card InspectButton + column-view AddTaskButton migrations + their two new tests + follow-up task housekeeping). Verified the asChild composition argument by reading Radix Slot's `mergeProps` directly — implementer's analysis is correct. All targeted tests green (114 / 114), surrounding regression suites green (116 / 116), `pnpm tsc --noEmit` clean. No blockers, no warnings — only nits below.

### Nits
- [x] `kanban-app/ui/src/components/column-view.add-task-enter.spatial.test.tsx:322` — Test description says "clicking the + button invokes onAddTask once **and seeds column focus once**" but the assertion at line 358–361 is `expect(focusCalls.length).toBeGreaterThanOrEqual(1)`. The inline comment (lines 349–357) explains correctly that the click also bubbles to the column `<FocusZone>`'s own onClick which fires a second `spatial_focus(columnFq)`, so the test intentionally uses `>=1`. Tighten the test name to match — e.g. `"clicking the + button invokes onAddTask once and seeds column focus at least once (click also bubbles benignly to column zone)"` — so future readers don't grep on the description and assume an exactly-once contract that the assertion does not enforce.
- [x] `kanban-app/ui/src/components/pressable.tsx:28-39` — The exception list in the Pressable docstring covers "purely decorative" and "mouse-only-by-design" but does not document the per-call-site `e.stopPropagation()` convention that `entity-card.tsx::InspectButton` relies on. The Pressable primitive deliberately does NOT stop propagation (the column add-task site relies on a benign click bubble to the column zone, and ripping bubble out at the primitive level would break that). Add a short note to the docstring like: "Pressable does not stop event propagation. If a call site needs to suppress click bubble (e.g. inside a `<FocusZone>` whose own onClick would steal focus), add `onClick={(e) => e.stopPropagation()}` on the inner `<button>` in `asChild` mode — Radix Slot's `mergeProps` runs the child's handler before the slot's, so stopPropagation lands before Pressable's `handleClick` triggers `onPress`. See `entity-card.tsx::InspectButton` for the canonical example." This makes the convention discoverable for future migrators (the five sites still left in the follow-up task `01KQPZAFSPJEMHMKRSQGPD0JM6`).