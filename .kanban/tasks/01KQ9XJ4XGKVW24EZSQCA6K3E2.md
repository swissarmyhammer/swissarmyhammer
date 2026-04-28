---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffd680
project: spatial-nav
title: Space and double-click must inspect any &lt;Inspectable&gt; — move inspect ownership from board scope to the Inspectable wrapper
---
## What

Inspect dispatch should be a single, per-`<Inspectable>` concern. Today the two inspect gestures live in different places:

- **Double-click** is owned by `<Inspectable>` (`kanban-app/ui/src/components/inspectable.tsx:175`) via the `useInspectOnDoubleClick` hook. It is correctly per-Inspectable: every entity wrapper carries its own dispatcher.
- **Space** is owned by `board.inspect` (`kanban-app/ui/src/components/board-view.tsx:599` `makeInspectCommand`, registered at the BoardView's `<CommandScopeProvider>` at line 1020). It dispatches against `focusedMonikerRef.current`, not against any specific wrap.

Because `board.inspect` lives in the BoardView scope and the inspector layer is mounted as a sibling of the BoardView (not a descendant), pressing Space on a focused inspector field — which **is** an `<Inspectable>` (`fields/field.tsx:410`) — currently does **nothing**: the field's scope chain has no command bound to Space, the global root scope has no inspect command bound to Space, and the board scope is not reachable from the inspector layer's React subtree.

The fix is to give Inspectable a single owner for both gestures, matching the pattern used by `field.edit` (`01KQ9X3A9NMRYK50GWP4S4ZMJ4`) and `ui.entity.startRename`: a scope-level command surfaced through `CommandScope` so `extractScopeBindings` finds it on every focused inspectable, regardless of which layer the entity lives in.

## Why this is the right shape

`<Inspectable>` is already the documented "single source of inspect dispatch" — `inspectable.tsx:30` calls itself "the **single source** of the double-click → `ui.inspect` dispatch", and the architectural guards in `focus-architecture.guards.node.test.ts` (Guards B + C) enforce that every inspectable entity scope is wrapped in `<Inspectable>` and that UI-chrome scopes are NOT. That invariant is what lets us safely move Space ownership here: every inspectable will respond to Space, and only inspectables will. Perspective tabs and other chrome stay quiet on Space because they are intentionally outside the wrapper — see `perspective-tab-bar.no-inspect-on-dblclick.spatial.test.tsx` for the existing assertion of that direction.

The board-scoped `board.inspect` was a workaround for not having a per-wrapper dispatcher. With Inspectable as the dispatcher for both gestures, `board.inspect` is dead code and can be removed.

## Approach

### 1. Inspectable registers a scope-level Space → `ui.inspect` command

`kanban-app/ui/src/components/inspectable.tsx` — push a `<CommandScopeProvider>` (or build a `CommandScope` and provide via `CommandScopeContext.Provider`) inside `<Inspectable>` that contributes:

```ts
{
  id: "entity.inspect",
  name: "Inspect",
  keys: { cua: "Space", emacs: "Space" },
  execute: () => dispatch({ target: moniker }),
}
```

The provider sits between the consumer's outer ancestor and the inner `<FocusScope>` / `<FocusZone>`, so:

- The descendant primitive's `CommandScope` reads `parent = useContext(CommandScopeContext)` → gets the Inspectable's scope.
- `extractScopeBindings(focusedScope)` walks: focused leaf scope → leaf's parent (= Inspectable's scope, which carries `entity.inspect: Space`) → ... up to root.
- Inner scopes win on key collisions, so a leaf or zone that also wants Space (none currently) is free to shadow this.

The `<div onDoubleClick=...>` wrapper that exists today stays — the dblclick gesture continues to flow through `useInspectOnDoubleClick(moniker)` because dblclick is a DOM event, not a keybinding.

The same `dispatch` reference (`useDispatchCommand("ui.inspect")`) backs both the Space command and the dblclick handler — one register call per Inspectable, one dispatcher closed over once.

`useInspectOnDoubleClick` should be retained as the dblclick path (and as the public hook for callers that need to attach the handler to a non-`<div>` host like `<tr>` — see `data-table.tsx`).

### 2. Remove `board.inspect`

`kanban-app/ui/src/components/board-view.tsx` — delete `makeInspectCommand` (line 599), drop the entry from the `boardActionCommands` array (line 691), and remove the now-unused `dispatchInspect` plumbing through `BoardActionDeps` (lines 588–596) if no other action factory depends on it. The `cua: Space` binding it claimed is taken over by Inspectable; the `vim: Enter` binding has already been removed by `01KQ9X3A9NMRYK50GWP4S4ZMJ4`.

After removal, no global or board-scoped command claims Space. The only Space binding in production is the one Inspectable contributes per-instance.

### 3. No change to chrome

The architectural guards (`focus-architecture.guards.node.test.ts`) already enforce that UI-chrome scopes (`ui:*`, `perspective_tab:*`, `grid_cell:*`) are NOT wrapped in `<Inspectable>`. No change needed; the existing `perspective-tab-bar.no-inspect-on-dblclick.spatial.test.tsx` continues to assert chrome is non-inspectable on dblclick, and a new test (below) extends that to Space.

## Acceptance Criteria

All asserted by automated tests below — no manual smoke step.

### Inspect path

- [ ] **Space on a focused card dispatches `ui.inspect`** with `target = task:<id>`. (Regression-equivalent to today's `board.inspect: cua Space` behavior; the new owner is Inspectable.)
- [ ] **Space on a focused inspector field zone dispatches `ui.inspect`** with `target = field:<type>:<id>.<name>`. (Today this is broken — board scope is not reachable from inside the inspector layer. Fix verified.)
- [ ] **Space on a focused tag pill (an inspectable `tag:` moniker) dispatches `ui.inspect`** with `target = tag:<id>`.
- [ ] **Space on a focused column zone (the column body itself, which IS an inspectable `column:` moniker) dispatches `ui.inspect`** with `target = column:<id>`.
- [ ] **Double-click on every inspectable still dispatches `ui.inspect`** for that wrapper's moniker. (Regression guard for the existing dblclick path.)

### Chrome stays non-inspectable

- [ ] **Space on a focused perspective tab does NOT dispatch `ui.inspect`** (perspective tabs are chrome, intentionally outside Inspectable).
- [ ] **Space on a focused navbar leaf (board-selector, search button, inspect button) does NOT dispatch `ui.inspect`** (navbar children are chrome).
- [ ] **Space on a focused grid cell `<FocusScope>` (the cursor target) does NOT dispatch `ui.inspect`** — the inner Field zone IS inspectable but the cell wrapper is chrome; the existing `handleEvents={false}` chain on the inner Field does not change. Verify the cell does not double-fire when both layers are present.

### Targets and side effects

- [ ] The `target` argument carried by every Space-dispatched `ui.inspect` matches the `moniker` of the closest enclosing `<Inspectable>` — not the focused leaf moniker. (When a leaf inside a card-zone receives focus, Space dispatches `ui.inspect({ target: task:<id> })`, not against the leaf's own moniker.)
- [ ] Pressing Space on a focused entity inside an `<input>`, `<textarea>`, `<select>`, or `[contenteditable]` does NOT dispatch `ui.inspect` — the editable surface owns Space (it inserts a literal space character). The same exclusion `useInspectOnDoubleClick` applies for dblclick must apply to the Space command's execute path.

## Tests

All tests are automated. No manual verification.

### Frontend — `kanban-app/ui/src/components/inspectable.space.browser.test.tsx` (new file)

Mounts `<Inspectable>` inside the production provider stack and asserts dispatch records.

- [ ] `space_on_focused_inspectable_dispatches_inspect_with_wrapper_moniker` — wrap a focusable element in `<Inspectable moniker="task:T1">`, focus the inner element, fire `keydown { key: " " }`, assert exactly one `ui.inspect` dispatch with `target = task:T1`.
- [ ] `space_on_focused_descendant_dispatches_inspect_with_nearest_inspectable_moniker` — `<Inspectable moniker="task:T1">` containing `<Inspectable moniker="field:task:T1.title">` containing a leaf; focus the deepest leaf, fire Space, assert dispatch target is `field:task:T1.title` (the closest Inspectable wins).
- [ ] `space_inside_input_does_not_dispatch_inspect` — focus an `<input>` inside an Inspectable, fire Space, assert zero `ui.inspect` dispatches and the input now has a literal space in its value.
- [ ] `space_inside_contenteditable_does_not_dispatch_inspect` — same with a `[contenteditable]` host.
- [ ] `dblclick_on_inspectable_still_dispatches_inspect` — regression guard for the existing dblclick path; wrap, dblclick on the inner element, assert exactly one dispatch with the wrapper moniker.

Test command: `bun run test:browser inspectable.space.browser.test.tsx` — all five pass.

### Frontend — `kanban-app/ui/src/components/board-view.space-inspect.browser.test.tsx` (new file)

Mounts the production board view against the per-test backend.

- [ ] `space_on_focused_card_in_board_dispatches_inspect` — focus a card via click, fire Space, assert one `ui.inspect` dispatch with `target = task:<id>`. (Replaces the existing `board.inspect: Space` assertion in `spatial-nav-end-to-end.spatial.test.tsx` lines 868–910 — that test continues to pass after the migration; verify it does not need changes other than maybe the source of the dispatcher.)
- [ ] `space_on_focused_perspective_tab_does_not_dispatch_inspect` — keep the existing assertion (`spatial-nav-end-to-end.spatial.test.tsx:913`) green after the migration. Adapt only if the test today relies on `board.inspect` being absent from the perspective bar's React subtree (it should not — the assertion is on the dispatched command, not on which scope claimed the key).
- [ ] `board_inspect_command_id_is_no_longer_registered` — query the registered global command list; assert `board.inspect` is absent from the BoardView's CommandScope. Regression guard so a future revert cannot silently re-introduce the duplicate binding.

Test command: `bun run test:browser board-view.space-inspect.browser.test.tsx` — all three pass.

### Frontend — `kanban-app/ui/src/components/inspector-field.space-inspect.browser.test.tsx` (new file)

Pins the bug fix — Space inside the inspector layer now reaches `ui.inspect`.

- [ ] `space_on_focused_inspector_field_dispatches_inspect_with_field_moniker` — open the inspector for a task, focus a field zone (`field:task:T1.<name>`), fire Space, assert one `ui.inspect` dispatch with `target = field:task:T1.<name>`.

Test command: `bun run test:browser inspector-field.space-inspect.browser.test.tsx` — passes.

### Frontend — augment existing test

- [ ] `kanban-app/ui/src/spatial-nav-end-to-end.spatial.test.tsx` — confirm the existing "Space on a focused card dispatches ui.inspect" (lines 874–910) and "Space on a focused perspective tab does NOT dispatch ui.inspect" (lines 913–942) tests still pass with the new dispatch site. Update inline comments to reflect that Inspectable (not BoardView) is now the source of Space's binding.

Test command: `bun run test:browser spatial-nav-end-to-end.spatial.test.tsx` — all pre-existing tests pass.

## Workflow

- Use `/tdd` — write the inspectable.space + inspector-field.space-inspect failing tests first, then move the binding into Inspectable and remove `board.inspect`.
- Single ticket — Space ownership is one concern, even though it spans Inspectable, BoardView, and a chrome regression guard. Each acceptance criterion has a corresponding test; no scattering.
- Architectural guards (`focus-architecture.guards.node.test.ts`) already encode the Inspectable invariants — re-run them after the change to confirm no chrome accidentally became inspectable in the migration.
