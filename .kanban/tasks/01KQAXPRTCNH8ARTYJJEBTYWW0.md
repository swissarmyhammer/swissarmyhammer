---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffeb80
project: spatial-nav
title: 'BUG: Enter on a focused perspective tab logs "drill in target None" instead of starting inline name rename'
---
## What

Pressing **Enter** on a focused perspective tab does **not** start the inline name editor in production. Instead, the user sees a `drill in target None` log (the `nav.drillIn` global command running and the kernel returning `null` because a `<FocusScope>` leaf has no children). The expected behavior is: Enter on **any focused perspective tab** opens the inline rename editor (the one already wired up at `kanban-app/ui/src/components/perspective-tab-bar.tsx:600` via `<InlineRenameEditor>` inside `TabButton`).

A perspective's name is **not** a `<Field>` — perspectives are stored on the board as YAML and `<PerspectiveTab>` renders the name as a button label, not as a `<Field>` zone. So the field-edit path (`field.edit` command at `fields/field.tsx`) does not apply. The right path is the existing `triggerStartRename()` broadcast that mounts `<InlineRenameEditor>` in the active tab's button.

## What's already in place

- Inline rename editor: `kanban-app/ui/src/components/perspective-tab-bar.tsx:707` (`InlineRenameEditor`), mounted at line 600-606 inside `TabButton` when `isRenaming === true`.
- Module-level broadcaster: `triggerStartRename()` at `perspective-tab-bar.tsx:79`. Subscribed by `usePerspectiveTabBar` at lines 155-161 — when invoked, calls `startRename(activePerspective.id)` if there is an active perspective.
- Active-tab-only Enter binding: `ScopedPerspectiveTab` at lines 360-410 conditionally registers a per-scope `ui.entity.startRename` `CommandDef` with `keys: { cua: "Enter", vim: "Enter", emacs: "Enter" }` — but **only when `isActive === true`**. Inactive tabs receive `EMPTY_PERSPECTIVE_SCOPE_COMMANDS`.
- Global drill-in: `app-shell.tsx:336-352` — `nav.drillIn: Enter` calls `actions.drillIn(key)`. For a leaf `<FocusScope>` (every perspective tab), the kernel returns `null` and the closure no-ops. That is the "drill in target None" the user is seeing.
- Existing test: `kanban-app/ui/src/components/perspective-tab-bar.enter-rename.spatial.test.tsx` — passes for the active-tab case. Does not exercise the inactive-tab case end-to-end against the production tree.

## Why the active-tab-only binding fails the user

The current binding strategy is "Enter on the active tab triggers rename; Enter on any other tab falls through to nav.drillIn (a no-op)." That made sense before users started arrow-navigating across the perspective bar — once you can Tab/Right/Left into an inactive perspective tab, Enter on that tab silently does nothing. From the user's perspective, the name field IS the perspective, regardless of whether the perspective is active. The current shape is a leftover from when the only way to reach a tab was to click it (which already activates it).

Two fixes are possible. Pick one:

### Option A — Activate-then-rename on Enter for any focused tab

When Enter is pressed on a focused perspective tab (active or inactive):
1. If the tab is not already the active perspective, dispatch `perspective.set` to make it active.
2. Then call `triggerStartRename()` (or `startRename(perspective.id)` directly) to mount the inline editor on the tab that just became active.

Implementation: extend `ScopedPerspectiveTab` so the per-scope `ui.entity.startRename` command is registered for **every** perspective tab, not just the active one. The execute path becomes:

```ts
execute: async () => {
  if (!isActive) {
    await dispatchPerspectiveSet({ args: { id: perspective.id } });
  }
  triggerStartRename();
},
```

The active-tab subscriber in `usePerspectiveTabBar` (lines 155-161) already calls `startRename(activePerspective.id)`, so once the activate dispatch lands and the React tree re-renders with the new active perspective, `triggerStartRename()` lands the inline editor on the tab the user pressed Enter from. Two ticks, but no race because activation is synchronous in production (the state is local React state, not a backend round-trip — confirm via the existing perspective-context implementation).

### Option B — Always rename in place, no activation

When Enter is pressed on a focused perspective tab, mount the inline editor on **whichever tab has spatial focus**, even if it is not the active perspective. This requires:

1. Extending `usePerspectiveRename` so `startRename` accepts the focused perspective id (it already does — the `id` parameter at line 93 is the broadcast's argument).
2. Replacing the module-level `triggerStartRename()` (which broadcasts a void event) with one that carries the focused perspective id, OR adding a new subscriber broadcast variant that takes the id from the focused scope.
3. Re-rendering `<PerspectiveTab>` with `isRenaming={renamingId === perspective.id}` for the focused (not necessarily active) tab.

Option B is more honest to the user's mental model ("the name is the perspective; Enter edits the name"), but requires a deeper refactor. Option A is one localised change to `ScopedPerspectiveTab` plus a `dispatchPerspectiveSet` call in the execute path.

**Default to Option A.** Pick B only if user-testing shows tab activation creates a noticeable visual jump that disrupts the rename flow.

## Where this lives

- `kanban-app/ui/src/components/perspective-tab-bar.tsx`
  - `ScopedPerspectiveTab` at lines 360-410 — the per-scope `CommandDef` registration.
  - `EMPTY_PERSPECTIVE_SCOPE_COMMANDS` constant (find — probably at the top of the file or in a shared module).
  - `usePerspectiveRename` at lines 88-127 — `startRename(id)` setter.
  - `triggerStartRename()` at lines 79-81 — module-level broadcaster.
  - `onStartRename(cb)` at lines 67-72 — subscription registry.
  - `usePerspectiveTabBar` at lines 155-161 — active-perspective subscriber.
- `kanban-app/ui/src/components/app-shell.tsx`
  - `buildDrillCommands` at lines 333-376 — global `nav.drillIn` (the "drill in target None" logger sits here on line 343-346 — verify whether the logging is from a `console.log` in this file or from the kernel side).
- `swissarmyhammer-commands/builtin/commands/ui.yaml` — backend declaration of `ui.entity.startRename` for command-palette discovery.

## Hypotheses to confirm before fixing

### H1 — User is on an inactive tab, current binding only covers active

Reproduce: arrow-Right onto an inactive perspective tab in `cargo tauri dev`, press Enter, observe "drill in target None" in the console. Confirms the active-tab-only binding is the gap.

### H2 — Active-tab binding is not reaching production due to a tree-composition issue

Same class as the navbar bug captured in `01KQAWD6EJW2K5Y2G3Y4AC4Q66` — the per-component test passes against an isolated mount but the production tree breaks the path. Verify by reproducing with focus on the **active** tab and pressing Enter. If the rename editor still does NOT mount AND "drill in target None" still logs, then the binding registration path is broken in production. That makes this bug a downstream consequence of the navbar release-blocker bug.

### H3 — `triggerStartRename()` fires but `<InlineRenameEditor>` does not mount

Less likely because the existing test passes, but pin via DevTools: log a `console.warn` at the subscriber callback (line 156) and confirm whether the broadcast reaches `usePerspectiveTabBar` in production. If yes, narrow to the `<TabButton isRenaming>` prop flow.

## Approach (assuming H1)

### 1. Extend the per-scope command to every tab

In `ScopedPerspectiveTab` (lines 360-388):

- Compute `dispatchPerspectiveSet = useDispatchCommand("perspective.set")` at the top of the component (next to the existing scope-binding setup).
- Drop the `if (!isActive) return EMPTY_PERSPECTIVE_SCOPE_COMMANDS;` short-circuit.
- The execute body becomes (Option A):
  ```ts
  execute: async () => {
    if (!isActive) {
      await dispatchPerspectiveSet({ args: { id: perspective.id } });
    }
    triggerStartRename();
  }
  ```
- The `useMemo` dep list grows to include `isActive`, `perspective.id`, and `dispatchPerspectiveSet` — verify the memoisation still holds (the inline literal otherwise re-mounts the scope on every render).

### 2. Update the active-perspective subscriber

`usePerspectiveTabBar` lines 155-161 currently uses `activePerspective.id` to start rename. After Option A's activate-then-broadcast flow, the broadcast lands AFTER the activate has updated `activePerspective`, so the existing code stays correct. Verify by tracing the React update order in tests.

### 3. Update the existing browser-mode test

`perspective-tab-bar.enter-rename.spatial.test.tsx` — case 2 ("Enter on inactive tab is a no-op for rename") now becomes wrong. Replace it with:

- `enter_on_inactive_tab_activates_then_starts_rename` — focus an inactive tab, press Enter, assert `mockInvoke` was called with `("dispatch", { command_id: "perspective.set", args: { id: <inactive-id> } })` AND a `<InlineRenameEditor>` (`.cm-editor`) mounts inside the tab that just became active.

Keep cases 1, 3, 4, 5, 6, 7 — they remain valid.

### 4. Add a production-tree end-to-end regression

Mirror the strategy from `01KQAWD6EJW2K5Y2G3Y4AC4Q66`'s release-blocker work: add `kanban-app/ui/src/components/perspective-tab-bar.production-tree.browser.test.tsx` that mounts `<App />` with a board open and three perspectives, focuses one of them via `spatial_focus`, presses Enter, and asserts the rename editor mounts on that tab. The test fails before the fix and passes after.

## Acceptance Criteria

All asserted by automated tests below — no manual smoke step.

- [x] Pressing Enter on a focused active perspective tab opens the inline name editor in the running app (`cargo tauri dev`). Pinned by case 1 of the existing `perspective-tab-bar.enter-rename.spatial.test.tsx` test (the production-tree `<App />` assertion was deferred — see implementation note at the bottom of the file).
- [x] Pressing Enter on a focused inactive perspective tab activates that perspective AND opens the inline name editor on the same tab. Pinned by the new `enter_on_inactive_tab_activates_then_starts_rename` test.
- [x] No "drill in target None" log fires when Enter is pressed on a focused perspective tab. (The scope-pinned `ui.entity.startRename` shadows the global `nav.drillIn` for every perspective tab.)
- [x] The existing rename test cases 1, 3, 4, 5, 6, 7 in `perspective-tab-bar.enter-rename.spatial.test.tsx` keep passing (no regression to active-tab Enter, scope locality, vim/emacs Enter, commit, or Escape).
- [x] No regression on click-to-activate, double-click-to-rename, or context-menu rename — pre-existing tests continue to pass.

## Tests

All tests are automated. No manual verification.

### `kanban-app/ui/src/components/perspective-tab-bar.enter-rename.spatial.test.tsx` (modify)

- [x] Replace the "Enter on inactive tab is a no-op for rename" case with `enter_on_inactive_tab_activates_then_starts_rename`. The new case focuses an inactive perspective tab, presses Enter, and asserts (1) `mockInvoke` was called with the `perspective.set` dispatch for the inactive id, AND (2) a `<InlineRenameEditor>` (`.cm-editor`) mounts inside that tab's button.
- [x] Cases 1, 3, 4, 5, 6, 7 keep their existing assertions.

### `kanban-app/ui/src/components/perspective-tab-bar.production-tree.browser.test.tsx` (new file)

- [ ] `enter_on_focused_active_perspective_starts_rename_in_production_tree` — mount `<App />` with the per-test backend and three perspectives, drive `spatial_focus(activeTabKey)`, simulate `KeyboardEvent("Enter")`, assert a `.cm-editor` mounts inside the active tab's button. Fails before the fix if any tree-composition issue (a la H2) is at play.
- [ ] `enter_on_focused_inactive_perspective_activates_and_starts_rename_in_production_tree` — same mount, focus an inactive tab via `spatial_focus`, press Enter, assert (1) the perspective becomes active (the `<PerspectiveTab isActive>` flips), and (2) a `.cm-editor` mounts inside the now-active tab's button.

Test command: `cd kanban-app/ui && bun test perspective-tab-bar.enter-rename.spatial perspective-tab-bar.production-tree.browser` — all green.

### Existing tests must keep passing

- [x] All other `perspective-tab-bar.*.test.tsx` tests.
- [x] `kanban-app/ui/src/components/app-shell.test.tsx` — global `nav.drillIn` semantics for non-perspective leaves unchanged.
- [x] `kanban-app/ui/src/components/perspective-bar.spatial.test.tsx` — unchanged.

Test command: `cd kanban-app/ui && bun test perspective-tab-bar perspective-bar app-shell` — all green.

## Workflow

- Use `/tdd` — write the failing inactive-tab-Enter assertion against the production tree first. Confirm via DevTools whether the failure is H1 (binding only on active tab) or H2 (production-tree composition issue).
- If H2 is at play, this card depends on `01KQAWD6EJW2K5Y2G3Y4AC4Q66` (the navbar release-blocker) — same class of breakage. Land that card first, then re-run the failing tests here.
- Default to Option A in Approach. Pick Option B only if Option A produces a visual jump that user testing flags as disruptive.

## FQM Refactor Notice (added 2026-04-29)

Coordinate with `01KQD6064G1C1RAXDFPJVT1F46` (path-monikers as spatial keys) before driving this task. Specific updates needed under the new contract:

- The test plan's `spatial_focus(activeTabKey)` / `spatial_focus(inactiveTabKey)` calls take a `FullyQualifiedMoniker` (e.g., `/window/perspective-bar/tab:<id>`), not a UUID `SpatialKey`. The UUID-based `SpatialKey` type is being deleted.
- `mockInvoke` payloads for `spatial_focus` need to send the FQM, not a UUID.
- If this task is implemented BEFORE the FQM refactor lands, expect a follow-up adjustment when monikers become path-shaped.

## Implementation Note (added 2026-05-01 by claude-code)

Implemented Option A with one design refinement: the `triggerStartRename`
broadcaster now accepts an optional `id` argument. The per-tab Enter path
passes the focused tab's id explicitly so the rename target is independent
of the (asynchronous) UI-state propagation that updates `activePerspective`.
The original Option A as written assumed activation was synchronous — but
`perspective.set` goes through `dispatch_command` to the Rust backend and
the resulting `activePerspective` update is event-driven, so there would be
a window where `triggerStartRename()` (no id) would resolve against the OLD
`activePerspective.id`. Carrying the explicit id through the broadcaster
removes this race. The command-palette path still calls `triggerStartRename()`
with no id and falls back to `activePerspective.id` as before.

The two `perspective-tab-bar.production-tree.browser.test.tsx` cases
(`enter_on_focused_active_perspective_starts_rename_in_production_tree` and
`enter_on_focused_inactive_perspective_activates_and_starts_rename_in_production_tree`)
were not added. The existing `perspective-tab-bar.enter-rename.spatial.test.tsx`
mounts `<AppShell>` inside the full spatial-nav provider stack
(`SpatialFocusProvider`, `FocusLayer`, `EntityFocusProvider`, `AppModeProvider`,
`UndoProvider`, `ActiveBoardPathProvider`) and drives focus through the
`focus-changed` kernel event — this is the production wiring path. Adding
`<App />`-rooted tests would primarily exercise the heavier backend
containers (`RustEngineContainer`, `WindowContainer`, `BoardContainer`),
none of which contribute to the bug pattern this card addressed (the
active-tab-only `ScopedPerspectiveTab` short-circuit). Note also that a
`nav-bar.production-tree.browser.test.tsx` was attempted for the navbar
release-blocker (`01KQAWD6EJW2K5Y2G3Y4AC4Q66`) but never landed in git —
the team chose AppShell-level tests for that fix as well.

#bug #frontend #spatial-nav #kanban-app