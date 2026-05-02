---
assignees:
- claude-code
position_column: todo
position_ordinal: c080
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
   - `kanban-app/ui/src/components/left-nav.tsx` — view-button click sites (the user said "nav buttons"). Each view button gets `<Pressable moniker="ui:leftnav.view:${viewId}" …>`. Note these are already partially in the spatial graph after commit `c01f3ed38` ("register view buttons with the spatial-nav kernel") — confirm via `Grep "ui:leftnav"` whether they're already FocusScopes; if so, swap to Pressable to gain Enter activation.
   - "Open window" — locate in `BoardSelector` (the tear-off affordance). Migrate the trigger that opens the new window.

3. Document the contract in the new file's docstring: every actionable icon button must use `<Pressable>`. The exception list is "purely decorative" (no `onPress`) and "mouse-only-by-design" (e.g. drag handle — see task `01KQM9478XFMCBBWHQN6ARE524`).

### Sizing note

Migrating every site in one task would exceed the 5-files-touched limit. **Scope this task to building the primitive PLUS migrating exactly two reference sites:** `nav-bar.tsx::ui:navbar.inspect` (the "i" Info button the user explicitly called out) and `perspective-tab-bar.tsx::AddPerspectiveButton` (the "+" Add button the user noted is "missing a focus scope"). File the remaining migrations as follow-up tasks (one per file or one rolled-up audit task) once the primitive's API is proven on those two.

## Acceptance Criteria
- [ ] `kanban-app/ui/src/components/pressable.tsx` exists. Exports `Pressable` and `PressableProps`. Props include `moniker`, `onPress`, `ariaLabel`, optional `asChild`, optional `disabled`, plus passthrough HTML button attributes.
- [ ] `<Pressable moniker="…" ariaLabel="…" onPress={fn}>…</Pressable>` mounts a `<FocusScope>` leaf with two scope-level CommandDefs: vim/cua `Enter` and cua `Space`. Both invoke `onPress` exactly once when the leaf is focused and the key is pressed.
- [ ] `<button onClick>` activation continues to work (mouse / pointer); `onPress` fires identically through both paths.
- [ ] `disabled={true}` suppresses both `onClick` and the keyboard activation CommandDefs (the `execute` closures short-circuit).
- [ ] `asChild` mode renders via Radix `<Slot>` so it composes with `<TooltipTrigger asChild>` without an extra `<button>` wrapper.
- [ ] `nav-bar.tsx::ui:navbar.inspect` is migrated to `<Pressable moniker={asSegment("ui:navbar.inspect")} ariaLabel="Inspect board" onPress={…}>` inside the existing `<Tooltip>`/`<TooltipTrigger asChild>`. With nav-bar focused on the Info button, pressing `Enter` dispatches the inspect command exactly once.
- [ ] `perspective-tab-bar.tsx::AddPerspectiveButton` is migrated to `<Pressable moniker={asSegment("ui:perspective-bar.add")} ariaLabel="Add perspective" onPress={handleAdd}>` — gaining keyboard reachability AND Enter/Space activation that today does not exist.
- [ ] No regressions: the eventdriven-nav contract holds (no extra IPC fetches per Enter); existing perspective-bar / nav-bar tests stay green.

## Tests
- [ ] Add `kanban-app/ui/src/components/pressable.test.tsx` (jsdom or browser, mirror harness from `entity-card.spatial.test.tsx`):
  - Test 1 — clicking the rendered button calls `onPress` once.
  - Test 2 — focusing the leaf and dispatching `Enter` (via the spatial-nav stack's CommandDef pipeline) calls `onPress` once.
  - Test 3 — focusing the leaf and dispatching `Space` (cua) calls `onPress` once.
  - Test 4 — `disabled={true}`: clicking the button does NOT call `onPress`; pressing Enter does NOT call `onPress`.
  - Test 5 — `asChild={true}` wrapped in `<TooltipTrigger asChild>`: only one `<button>` renders in the DOM (no double-button); both click and Enter still fire `onPress`.
  - Test 6 — registers as `spatial_register_scope` with the supplied `moniker` segment (mock `mockInvoke` like `nav-bar.scope-leaf.spatial.test.tsx`).
- [ ] Update `kanban-app/ui/src/components/nav-bar.spatial-nav.test.tsx` (or add a sibling `nav-bar.inspect-enter.spatial.test.tsx`): seed focus on `ui:navbar.inspect`, dispatch keydown Enter, assert `mockInvoke("dispatch_command", { cmd: "ui.inspect", … })` was called exactly once with the board's moniker.
- [ ] Add `kanban-app/ui/src/components/perspective-tab-bar.add-enter.spatial.test.tsx`: seed focus on `ui:perspective-bar.add`, dispatch keydown Enter, assert the add-perspective dispatch fired exactly once.
- [ ] Existing tests stay green: `nav-bar.test.tsx`, `nav-bar.spatial-nav.test.tsx`, `nav-bar.focus-indicator.browser.test.tsx`, `entity-card.scope-leaf.spatial.test.tsx`, `perspective-tab-bar.spatial-nav.test.tsx`, `perspective-bar.spatial.test.tsx`.
- [ ] Run `cd kanban-app/ui && pnpm vitest run src/components/pressable src/components/nav-bar src/components/perspective-tab-bar` and confirm green.

## Workflow
- Use `/tdd` — write `pressable.test.tsx` first (six failing assertions), then build `pressable.tsx`, then add the two migration tests, then perform the two migrations and confirm everything green. Follow-up tasks for the remaining migration sites (`InspectButton` in entity-card, `FilterButton`/`GroupButton` in perspective-tab-bar, left-nav view buttons, BoardSelector "Open window" trigger, navbar Search button) get filed once the primitive's API is settled.
