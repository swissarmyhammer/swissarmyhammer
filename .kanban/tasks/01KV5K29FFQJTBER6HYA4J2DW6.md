---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kv65hm42k3k22zt22zbq5q52
  text: |-
    Implemented (TDD red→green).

    Key design discovery: the card's "add a separate emit_view_switch_context_menu emission" approach is impossible as literally written — `dedupe_by_id` keys on `(id, args)` (NOT target) and keeps the first occurrence, so a second `view.set` row carrying the same `view_id` args collapses against the palette row and is dropped before reaching the context-menu surface. The correct, data-driven solution is one row per view with the in-scope view's row flipping `context_menu` to true.

    Changes:
    - crates/swissarmyhammer-kanban/src/scope_commands.rs: `emit_view_switch` now takes `scope_chain`; it collects the `view:{id}` monikers in scope and emits each view's `view.set` row with `context_menu: true` ONLY for the in-scope view (typically exactly one), `false` otherwise. Added `view_switch_caption()` which renders "Switch to View {name}", appending "View" only when the stored name doesn't already contain it (so "Board" → "Switch to View Board", "Board View" → "Switch to Board View"). Updated module + emit_dynamic_commands docstrings.
    - apps/kanban-app/ui/src/components/left-nav.tsx: updated the three now-stale "view switching is palette-only / context menu never shows Switch to <ViewName>" docstrings to describe the scoped context-menu entry. ViewButton already wires useContextMenu + the `view:{id}` CommandScopeProvider — no logic change needed there.
    - apps/kanban-app/ui/src/components/view-switch-commands.retired.node.test.ts: updated docstring to note the new context-menu surfacing reuses canonical `view.set` (not a resurrected `view.switch:*` id), so the retired-id guard is unaffected. Guard kept (still valid).

    Tests:
    - dynamic_sources_headless.rs: 3 new tests — context_menu_view_switch_is_scoped_to_the_view_in_scope (exactly one context_menu:true view.set whose view_id == in-scope view, no sibling; palette rows for all views remain), context_menu_view_switch_caption_avoids_double_view_word, no_context_menu_view_switch_row_without_a_view_in_scope. RED confirmed (both new flag-assertions failed because both rows were context_menu:false), GREEN after.
    - left-nav.view-switch.browser.test.tsx: new test drives the real ViewButton/useContextMenu path — right-click View 2 fires `list command` with `view:v2` in the scope chain and only View 2's "Switch to View" entry reaches `show context menu`.

    Verification (all fresh):
    - cargo test -p swissarmyhammer-kanban --test dynamic_sources_headless: 10 passed.
    - cargo test -p swissarmyhammer-kanban --lib: 1222 passed.
    - npx vitest run (left-nav.view-switch + view-switch-commands.retired + context-menu.scoped + left-nav.browser): 12 passed.
    - cargo clippy -p swissarmyhammer-kanban --tests: clean (only 2 pre-existing warnings in swissarmyhammer-focus, unrelated).
    - npx tsc --noEmit: clean. cargo fmt --check: clean.
  timestamp: 2026-06-15T17:33:53.154400+00:00
- actor: claude-code
  id: 01kv66faxpq573vy8nng66jf1p
  text: |-
    Addressed all 5 Review Findings (test-code style/naming only — no production logic touched).

    left-nav.view-switch.browser.test.tsx:
    - Renamed `_cmd`/`_args` -> `_command`/`_arguments` in the hoisted mockInvoke and the beforeEach mock impl (and the inner `cmd ===` body refs -> `command ===`).
    - Renamed `cb` -> `callback` in the `listen` mock + emitTauriEvent forEach + the header comment.
    - Renamed `cur` -> `currentCallbacks` (and the inner `c` lambda param -> `registered`) in the listen unsubscribe closure.
    - Left the wire-protocol field accessors `.cmd`/`.args` and the `[cmd, args]` mock.calls tuple destructuring in assertions untouched: `cmd`/`args` are the real dispatch-payload field names, not the flagged mock signatures.

    dynamic_sources_headless.rs:
    - `switch_rows_for_scope` now takes `views: &[ViewInfo]` and `scope: &[String]` (was owned `Vec`s). Uses `views.to_vec()` into the DynamicSources struct (ViewInfo: Clone) and passes `scope` straight through. Updated all 3 call sites to `&views` / `&[...]`.
    - Hoisted `VIEW_X_ID`/`VIEW_Y_ID` ULID constants to module level right after `GRID_VIEW_A_ID`/`GRID_VIEW_B_ID`, matching the established `_ID`-suffix fixture convention; removed the 3 local redefinitions and updated every reference.
    - Ran `cargo fmt -p swissarmyhammer-kanban` (the renamed constants pushed two `vec![...]` lines past the width, so rustfmt reflowed them).

    Verification (all fresh, green):
    - cargo test -p swissarmyhammer-kanban --test dynamic_sources_headless: 10 passed; 0 failed.
    - cd apps/kanban-app/ui && npx vitest run left-nav.view-switch.browser.test.tsx: 3 passed.
    - cargo fmt --check: clean (exit 0).
    - npx tsc --noEmit: clean (exit 0).

    No behavior changes — structure/naming only. Moving back to review.
  timestamp: 2026-06-15T17:50:06.774281+00:00
- actor: claude-code
  id: 01kv6805dng4r6sr6tfbde3zry
  text: |-
    Addressed the 2 Review Findings (2026-06-15 12:50) and did a self-pass over the whole file for the same class of trivial style issues. Test-code style only — no behavior change.

    left-nav.view-switch.browser.test.tsx:
    - Added explicit `: void` return type to `emitTauriEvent` (the named finding).
    - Added a justification to all four `eslint-disable-next-line @typescript-eslint/no-explicit-any` comments (mockInvoke hoisted fn, the `@tauri-apps/api/core` invoke mock, and the two `mockImplementation` bodies): `-- Tauri invoke arguments are command-specific and untyped at this mock boundary` / `-- forwards the real invoke variadic signature, whose argument types vary by command`.

    Self-pass (same class — missing explicit return types on helper functions):
    - `renderLeftNavLoop()` -> `: RenderResult` (added `import type { RenderResult } from "@testing-library/react"`). The result is unused by callers, but the annotation makes the helper's contract explicit and drift-safe.
    - `uiStateSnapshot(activeViewId)` -> `: UIStateSnapshot` (added `import type { UIStateSnapshot } from "@/lib/ui-state-context"`). Annotating with the real backend snapshot type means tsc now errors if the fixture diverges from production — strictly better than the inferred object literal.
    - The remaining functions already carry explicit `Promise<unknown>` return types (the mock arrows) or are `it(...)` test bodies. No abbreviated identifiers were introduced.

    Note: this UI package has no eslint script/config wired (eslint v10 expects flat config that isn't present), so tsc + vitest are the authoritative gates here. The disable justifications still satisfy the project's eslint-disable-with-reason convention.

    Verification (fresh):
    - npx vitest run src/components/left-nav.view-switch.browser.test.tsx: 3 passed (exit 0).
    - npx tsc --noEmit: clean (exit 0).

    Flipped both 12:50 checklist items to [x]. Moving back to review.
  timestamp: 2026-06-15T18:16:46.773421+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffb380
project: builtin-commands
title: Surface per-view "Switch to View «name»" in the View context menu (and verify palette coverage everywhere)
---
## Problem

Right-clicking a View (the left-nav view buttons) shows no usable context menu — it should offer **"Switch to View «name»" for that view**. Separately, every view should be switchable from the command palette anywhere ("Switch to View «name»"), as dynamic commands derived from the available views.

This is a deliberate-decision reversal, not a pure bug. Today view switching is **palette-only by design**:

- `crates/swissarmyhammer-kanban/src/scope_commands.rs` → `emit_view_switch` emits one dynamic `view.set` row per view (`name: "Switch to {view.name}"`, `args: { view_id }`) but with **`context_menu: false`**, so it never appears on right-click.
- `apps/kanban-app/ui/src/components/left-nav.tsx` documents this verbatim: *"View switching is palette-only, so the context menu never shows a 'Switch to <ViewName>' entry; the `view:{id}` moniker is still needed for other dynamics (e.g. `entity.add:{type}`)."* Each view button already mounts a `CommandScopeProvider moniker={view:{id}}` and reads `useContextMenu`, so the scope plumbing exists — only the backend emission withholds the entry. (See also the regression marker `apps/kanban-app/ui/src/components/view-switch-commands.retired.node.test.ts`.)

The dynamic rows are NOT collapsed by dedup — `SeenKey = (id, target, args)` includes the args serialization (`scope_commands.rs:233`), so per-view `view.set` rows (same id, distinct `view_id` args) are distinct. The per-perspective `perspective.switch` dynamic uses this exact same-id/different-args pattern and works.

## Decision: the context menu for a given view shows ONLY that view

Right-clicking view X must show exactly **one** view-switch entry — "Switch to View «X»" (X's own) — NOT a list of all views. The palette is the place to switch to *any* view; the per-view context menu is scoped to the view you clicked.

## Critical constraint

`emit_view_switch` emits **unconditionally** (all views, independent of scope) for the palette. The context-menu entry must instead be **scope-resolved to the single `view:{id}` in the scope chain** — mirror `emit_entity_add`, which resolves the view from the `view:{id}` moniker in scope and emits exactly for that view. This both confines the entry to a view context (no leaking into task/column/global menus) and yields exactly one self-referential switch entry per view button.

## What

One cohesive change: per-view "Switch to View «name»" available in (a) the View context menu — scoped to that view only — and (b) the palette everywhere.

- `crates/swissarmyhammer-kanban/src/scope_commands.rs`:
  - Keep the existing palette emission (`emit_view_switch`: all views, `context_menu: false`) so every view is palette-switchable.
  - Add a **scope-resolved** context-menu emission (e.g. `emit_view_switch_context_menu`): for the `view:{id}` moniker present in `scope_chain`, look up that one view in `dyn_src.views` (index by id, as `emit_dynamic_commands` already does for `emit_entity_add`) and emit a single `view.set` row with `context_menu: true`, `args: { view_id }`, caption "Switch to View «name»" — for THAT view only. Do not emit other views' rows in the context menu. Avoid double-naming when `view.name` already contains "View" (caption should read naturally, e.g. `Switch to {{view.name}}` if names are like "Board View", or `Switch to View {{view.name}}` if names are like "Board").
  - Wire the new emission into `emit_dynamic_commands` (alongside `emit_view_switch`).
- `apps/kanban-app/ui/src/components/left-nav.tsx`: update the now-stale "view switching is palette-only … context menu never shows a Switch to <ViewName> entry" docstring; verify `ViewButton`'s `useContextMenu` renders the scoped context_menu row for its own `view:{id}` scope.
- Verify the palette path populates `DynamicSources.views` so the palette switch rows are available "anywhere" (not only when a view is focused). `DynamicSources.views` is built by `crates/swissarmyhammer-kanban/src/dynamic_sources.rs::build_dynamic_sources`; confirm the palette's `commands_for_scope` call receives a populated `dynamic` with `.views` regardless of current focus, and fix if it passes `None`/empty.

## Acceptance Criteria
- [ ] Right-clicking view X in the left-nav shows a context menu with exactly its own "Switch to View «X»" entry (one entry, X's own — NOT entries for other views); selecting it dispatches `view.set` with X's `view_id` and switches the active view.
- [ ] The command palette lists a "Switch to View «name»" entry for every available view, dispatchable from anywhere in the app (not only while a view is focused).
- [ ] View-switch context-menu rows do NOT appear in unrelated context menus (right-click a task/column/board surface shows no view-switch entry) — the context-menu emission is scope-resolved to the `view:{id}` moniker.
- [ ] No new backend command id; reuses the existing `view.set` (kanban-misc-commands) with per-view `args.view_id` and `{{view.name}}`-templated captions.

## Tests
- [ ] `crates/swissarmyhammer-kanban/tests/dynamic_sources_headless.rs` (the headless harness that asserts the exact dynamic rows `commands_for_scope` emits): with `view:{X}` in the scope chain, assert exactly ONE `view.set` row with `context_menu: true` whose `args.view_id == X` (and assert NO other view's id appears as a `context_menu: true` row); with NO `view:` moniker in scope, assert no `context_menu: true` view-switch row at all. Keep asserting the palette (`context_menu: false`) rows for all views.
- [ ] `apps/kanban-app/ui/src/components/left-nav.view-switch.browser.test.tsx`: extend to open the context menu on view button X and assert it contains "Switch to View «X»" and NOT "Switch to View «Y»" for a different view Y; selecting it dispatches `view.set` with X's `view_id`.
- [ ] Update/replace `apps/kanban-app/ui/src/components/view-switch-commands.retired.node.test.ts` to reflect the un-retired (scoped) context-menu behavior, or delete it if fully superseded, so the retired-surface marker doesn't contradict the new behavior.
- [ ] `cargo test -p swissarmyhammer-kanban --test dynamic_sources_headless` and `cd apps/kanban-app/ui && npx vitest run src/components/left-nav.view-switch.browser.test.tsx` both pass (new assertions red before the change, green after).

## Workflow
- Use `/tdd` — add the failing headless assertion (exactly the scoped view's `context_menu:true` `view.set` when `view:{X}` in scope; none otherwise) and the failing left-nav context-menu test first, then implement the scope-resolved emission and make them pass.

## Review Findings (2026-06-15 12:34)

### Warnings
- [x] `apps/kanban-app/ui/src/components/left-nav.view-switch.browser.test.tsx:22` — Parameter abbreviated as `_cmd` violates no-abbreviations rule; use `_command` instead. Rename `_cmd` to `_command` and `_args` to `_arguments` consistently throughout the mock implementations.
- [x] `apps/kanban-app/ui/src/components/left-nav.view-switch.browser.test.tsx:35` — Parameter abbreviated as `cb` violates no-abbreviations rule; use `callback` instead. Rename `cb` to `callback` throughout this mock implementation (lines 35, 37, 43).
- [x] `apps/kanban-app/ui/src/components/left-nav.view-switch.browser.test.tsx:41` — Variable abbreviated as `cur` violates no-abbreviations rule; use `current` or `currentCallbacks` instead. Rename `cur` to `currentCallbacks` or `current` for clarity.
- [x] `crates/swissarmyhammer-kanban/tests/dynamic_sources_headless.rs:301` — Function accepts concrete `Vec` types instead of slices or iterators. Rule: accept generics, not concrete types — use `&[T]` not `&Vec<T>`, or `impl IntoIterator<Item=T>` for owned-or-borrowed flexibility. Change to: `fn switch_rows_for_scope(views: &[swissarmyhammer_views::ViewInfo], scope: &[String],)` or use `impl IntoIterator` if the function needs to consume the vectors.
- [x] `crates/swissarmyhammer-kanban/tests/dynamic_sources_headless.rs:543` — VIEW_X and VIEW_Y are defined locally in 3 test functions instead of once at module level, repeating the same ULID values. The existing pattern (GRID_VIEW_A_ID, GRID_VIEW_B_ID at lines 368–371) shows the established convention — new view fixture constants should follow it. Define `const VIEW_X_ID: &str = "01JMVIEW0000000000XGRID0";` and `const VIEW_Y_ID: &str = "01JMVIEW0000000000YGRID0";` at module level after line 371, then use these in all three test functions instead of redefining locally.

## Review Findings (2026-06-15 12:50)

All 5 prior findings verified addressed (mock signatures `_command`/`_arguments`/`callback`/`currentCallbacks`, helper takes `&[ViewInfo]`/`&[String]`, `VIEW_X_ID`/`VIEW_Y_ID` hoisted to module level). Re-review surfaced 2 new in-scope test-code-style findings in the changed file.

### Warnings
- [x] `apps/kanban-app/ui/src/components/left-nav.view-switch.browser.test.tsx:121` — `function emitTauriEvent(event: string, payload: unknown)` lacks an explicit return type annotation; TS convention requires explicit return types on functions. Add `: void`.
- [x] `apps/kanban-app/ui/src/components/left-nav.view-switch.browser.test.tsx:41,49,158,240` — Four `eslint-disable-next-line @typescript-eslint/no-explicit-any` comments precede `any`-typed mock params with no justification. Add a justification to each disable comment (e.g. `-- Tauri invoke arguments vary by command name`) or type as `Record<string, unknown>`.

### Out of scope (not blocking — cross-file refactors beyond this card)
- Engine also flagged cross-file test-helper duplication (`mockInvoke`/`listeners` vs left-nav.browser.test.tsx; `uiStateSnapshot` vs views-container.view-set.test.tsx; `open_board` vs perspective_migration.rs), proposing extraction into shared test-utility modules. These are pre-existing, intentional per-test setup patterns spanning files this card did not own; extracting them is a broad cross-file refactor outside this card's scope and is not recorded as a blocker here.