---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffdd80
title: Filter tab button must focus via nav.focus, not a parallel Tauri channel
---
## What

The `command-driven-ui` migration of the Filter tab button (task `01KRE1YA65MMG29RDQDQ0VPJQG`) replaced the hardcoded `<FilterFocusButton>` with a registry-rendered `<CommandButton>` for `perspective.filter.focus`. The unit tests pass — but **clicking the button no longer focuses the formula bar in the live app**.

Root cause was architectural: the migration invented a **parallel focus channel** instead of using the existing `nav.focus` primitive.

Pre-fix wiring (the parallel channel — deleted by this card):

1. Click `<CommandButton>` → dispatch `perspective.filter.focus`.
2. `FocusFilterCmd::execute` returns a `{"FocusFilter": {"perspective_id": ...}}` marker envelope.
3. Tauri dispatcher's `handle_focus_filter` catches the envelope and emits a Tauri event `ui.focus.filter`.
4. `<FilterEditorBody>` `listen`s for that event, id-checks the payload, and calls `innerRef.current?.focus()` on the CM6 contenteditable.

The architecture per `01KR7CDEFWWVF4WH0BCHE8Y21J` is explicit: **every focus claim flows through `nav.focus({ args: { fq } })`**. Components do not call `spatial.focus(fq)`, `setFocus(fq)`, or imperative `.focus()` directly — they dispatch `nav.focus`.

## Implementation notes (post-merge)

### Stage that was broken in the live app

Stage 1 — the click site. The `<CommandButton>` dispatched `perspective.filter.focus` through the backend command pipeline. The backend's `FocusFilterCmd` returned a `FocusFilter` marker envelope. The Tauri dispatcher's `handle_focus_filter` then emitted `ui.focus.filter`. `<FilterEditorBody>` subscribed to it and called `focus()` on the CM6 editor. The reason this broke in production while passing tests: the kernel's "where is focus now?" model was never updated — the `.focus()` call only moved DOM focus, leaving spatial-nav unaware. Even if all the test mocks lined up, the production path went around the kernel.

### What was changed

**Frontend (`kanban-app/ui/src/components/perspective-tab-bar.tsx`):**

- Added `FilterEditorFqContext` — a context carrying a `MutableRefObject<FullyQualifiedMoniker | null>`. Provided at the `<PerspectiveTabBar>` level so both the tab buttons (writer's sibling subtree) and the formula bar (writer subtree) can share it.
- Added `<FilterFocusCommandButton>` adapter that mirrors `<CommandButton>`'s render (icon via `commandIconFor`, `text-primary` highlight, `${surface}.${command.id}:${surfaceId}` moniker) but overrides `onPress` to dispatch `nav.focus({ args: { fq } })` against the captured FQM. Used only for `perspective.filter.focus`; all other tab commands still go through `<CommandButton>`.
- `<RegistryTabButtons>` now special-cases the `perspective.filter.focus` command id and routes it to `<FilterFocusCommandButton>`. All other commands continue to use `<CommandButton>` unchanged.
- `<FilterEditorDrillOutWiring>` (already inside the `filter_editor:${id}` `<FocusScope>`) now writes its composed FQM into the context ref via `useEffect`, with an identity-check cleanup so a sibling instance's value can't be clobbered.
- `<PerspectiveTabBar>` instantiates the ref via `useRef<FullyQualifiedMoniker | null>(null)` and wraps everything in `<FilterEditorFqContext.Provider>`.

**Frontend (`kanban-app/ui/src/components/filter-editor.tsx`):**

- Deleted the `listen("ui.focus.filter", …)` `useEffect`. Replaced the block with a routing-history comment.
- Dropped the now-unused `import { listen } from "@tauri-apps/api/event"`.

**Tauri glue (`kanban-app/src/commands.rs`):**

- Deleted `handle_focus_filter` and its call site in `handle_ui_trigger_results`.

**Backend command (`swissarmyhammer-kanban/src/commands/perspective_commands.rs`):**

- Replaced `FocusFilterCmd::execute`'s `FocusFilter` marker envelope with a `Value::Null` no-op. The struct is kept (with updated doc comment) only to satisfy the `test_all_yaml_commands_have_rust_implementations` / `test_no_orphan_rust_commands_without_yaml` invariants in `commands/mod.rs`. The deleted `focus_filter_command_dispatches_focus_event` and `focus_filter_command_prefers_explicit_perspective_id_arg` tests have been replaced by a single `focus_filter_command_is_a_noop` test that pins the new no-op contract. The `focus_filter_command_is_always_available` test is kept (and re-justified — the tab-button slot needs it to emit on every active perspective).
- Updated the registration comment in `swissarmyhammer-kanban/src/commands/mod.rs` to describe the new no-op rationale.

**YAML (`swissarmyhammer-kanban/builtin/commands/perspective.yaml`):**

- Rewrote the `perspective.filter.focus` entry's comment block to explain the new mechanism (click dispatches `nav.focus` against `filter_editor:${id}`) and to cross-reference `nav.yaml`, `spatial-focus-context.tsx`, and the `FilterFocusCommandButton` adapter. The YAML shape itself is unchanged (`scope: "entity:perspective"`, `tab_button: { icon: filter }`, `params`, `keys: {}`).

### Tests

- **New** `kanban-app/ui/src/components/perspective-tab-bar.filter-migration.test.tsx`:
  - `filter_button_click_dispatches_nav_focus_with_filter_editor_fq` — clicks the Filter button, asserts a `spatial_focus` IPC fires with `fq` ending in `filter_editor:p1`, and asserts the deleted `dispatch_command(perspective.filter.focus)` backend path is NOT hit.
  - `filter_button_click_targets_the_currently_active_perspective` — sibling-perspective test (active is `p2`, asserts focus lands on `filter_editor:p2`).
  - Three existing tests for the `text-primary` highlight, the unset-filter glyph fill, and the new spatial-nav moniker `perspective_tab.perspective.filter.focus:p1` — all kept and still passing.
  - The old `filter_editor_focuses_on_ui_focus_filter_event_for_matching_perspective` test and its `mockListen` / `listeners` machinery were deleted along with the channel.

- **Updated** `kanban-app/ui/src/components/perspective-tab-bar.filter-enter.spatial.test.tsx`:
  - The Enter-keyboard activation test now asserts that Enter on the focused filter leaf produces a `spatial_focus` IPC with `fq` ending in `filter_editor:p1` — the new `nav.focus` path — and does NOT dispatch the deleted `perspective.filter.focus` backend command.

### Verification

- `git grep -i 'FocusFilter\|ui\.focus\.filter'` in `kanban-app/` and `swissarmyhammer-kanban/` (excluding `.kanban/` and `.md`) returns only historical/doc-comment references describing what was deleted. The kept `FocusFilterCmd` is a no-op marker for the YAML ↔ Rust invariant.
- `npx vitest run` — **2142 passed, 228 files**.
- `cargo test -p swissarmyhammer-kanban` — **1141 passed**.
- `cargo test -p kanban-app` — **106 passed**.
- `npx tsc --noEmit` — clean.
- `cargo clippy -p swissarmyhammer-kanban --lib` — clean.

### Behaviour preserved

- Palette and (future) keybinding invocation of `perspective.filter.focus` still resolve cleanly: they hit the no-op `FocusFilterCmd` backend handler which returns `Value::Null` (silent no-op). The click-site routing through `nav.focus` is the user-facing path; once those surfaces migrate they will also go through `nav.focus` directly.

#bug #command-driven-ui #frontend #kanban-app #perspectives #spatial-nav

## Review Findings (2026-05-13)

**Mode:** Structured code review against the six review layers + the JS/TS and Rust guidelines.

**Scope:** Changes for this card on the `kanban` branch — `perspective-tab-bar.tsx`, `filter-editor.tsx`, `kanban-app/src/commands.rs`, `swissarmyhammer-kanban/src/commands/perspective_commands.rs`, `swissarmyhammer-kanban/src/commands/mod.rs`, `swissarmyhammer-kanban/builtin/commands/perspective.yaml`, and the two test files in `kanban-app/ui/src/components/`.

**Counts:** 0 blockers, 0 warnings, 3 nits.

### Blockers

None.

### Warnings

None.

### Nits

1. **`perspective-tab-bar.filter-migration.test.tsx:351-383` — sibling-perspective test does not exercise a dynamic switch.**
   The test `filter_button_click_targets_the_currently_active_perspective` sets `p2` as initially active and asserts focus lands on `filter_editor:p2`. That validates the writer picks up the active perspective at mount-time, but it does NOT validate the dynamic case the original parallel-channel bug was vulnerable to: render with `p1` active, switch to `p2` (which remounts `<FilterFormulaBarFocusable>` via its `key={activePerspective.id}`), then click. The current test would still pass if `FilterEditorDrillOutWiring`'s cleanup didn't properly clear-and-reset the ref — because there's only ever one writer in this fixture. A stronger test would re-render with a new active perspective and re-click. Not a blocker; the static case still pins the contract, and the spatial-nav Enter test (`perspective-tab-bar.filter-enter.spatial.test.tsx`) covers the registration path. Worth a follow-up if the ref-write semantics ever change.

2. **`perspective-tab-bar.tsx:1378-1395` — multi-line "DELETED" tombstone comment will rot.**
   The 17-line comment block describing what `<FilterFocusButton>` used to be and which cards deleted/rewired it is fine for archeology *today* but becomes dead weight on every read going forward. Git blame already carries the deletion. A one-liner pointing to the card id (e.g. `// FilterFocusButton: replaced by FilterFocusCommandButton — see card 01KRGZY33P99J7CGG0XRQGZ352`) would preserve the breadcrumb without the rot surface. Same critique applies to the analogous block at lines 1472-1491 (`<GroupPopoverButton>` tombstone) which is out of scope for this card but worth flagging together. The `FilterEditorFqContext` rationale comment at lines 60-80 is load-bearing and should stay.

3. **`perspective-tab-bar.tsx:519-574` — `<FilterFocusCommandButton>` hardcodes `perspective_tab` as the surface segment.**
   The moniker built at line 549-551 (`perspective_tab.${command.id}:${perspectiveId}`) hardcodes the surface — unlike the generic `<CommandButton>` (`command-button.tsx:58`) which takes `surface` as a prop. Today the adapter is special-cased only inside `<RegistryTabButtons>` so this is harmless, but if any future surface ever wants the same dispatch-override semantics this couples the adapter to one location. A small cleanup would be to thread `surface` through as a prop to mirror `<CommandButton>`'s shape, even though the only call site is constant. Nit; ship as-is is fine.

### What I verified positively

- **No-op `FocusFilterCmd`:** struct doc (`perspective_commands.rs:823-847`), registration comment (`commands/mod.rs:207-219`), YAML doc block (`perspective.yaml:44-77`), and the kept `focus_filter_command_is_a_noop` test all agree on the contract and reference each other. A future reader has three breadcrumbs into the rationale. The YAML↔Rust completeness invariant is real and documented; deleting the struct would have widened the invariant for a single special case. Keep-as-no-op is the right call.
- **Listener cleanup is complete:** `filter-editor.tsx` no longer imports `listen`, no `ui.focus.filter` listener, only a routing-history comment at lines 401-416 (which is load-bearing — it tells the next reader why there's no `useEffect` here). `kanban-app/src/commands.rs` no longer references `FocusFilter` / `handle_focus_filter` / `ui.focus.filter` in active code (grep returns only unrelated comment fragments).
- **`nav.focus` dispatch pattern parity:** `FilterFocusCommandButton` (lines 528-547) and `FilterEditorDrillOutWiring` (lines 979-996) use the same hook (`useDispatchCommand("nav.focus")`), the same args shape (`{ args: { fq } }`), and the same `void … .catch(console.error)` rejection convention. Consistent.
- **Ref-based context carrier race:** the writer (`<FilterEditorDrillOutWiring>`) and reader (`<FilterFocusCommandButton>`) both gate on `activePerspective`, so they mount in the same React commit. The writer's `useEffect` runs after commit but before any user click can land (clicks require paint). Cleanup uses strict identity (`filterEditorFqRef.current === filterFq`) so a perspective switch's stale cleanup can't clobber the new writer. The "no FQM available" fallback in the click handler is a graceful warning + no-op. No observable race.
- **Tests pin observable behaviour:** assertions target `spatial_focus` IPC payloads (kernel-facing) and `data-segment` DOM markers (DOM-observable), not React-internal refs or component names. Implementation can be refactored without breaking the contract.
- **Architecture fit:** `<CommandButton>` has no extensibility hook for custom dispatch (`useDispatchCommand()` is hardcoded internally; `handlePress` is the only path to dispatch). Forking a sibling adapter for the one special case is the right pattern given today's API. A future refactor could add an `onPress` override prop to `<CommandButton>` if more cases arise.

**Outcome:** No blocking issues. Recommend moving to `done`. The three nits are quality-of-life and can be deferred or rolled into a future cleanup pass.