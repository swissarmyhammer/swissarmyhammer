---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffff8f80
title: Clear Filter command does not reset formula bar editor buffer
---
## What

When `perspective.clearFilter` is dispatched from outside the filter formula bar — context menu on a perspective tab, the command palette, a keybinding, undo/redo, or an event from another window — the backend clears `perspective.filter` and entities refresh correctly, but the perspective bar's formula-bar editor continues to display the stale filter text.

Root cause is in the rendering pipeline between three files:

- `kanban-app/ui/src/components/perspective-tab-bar.tsx` — `FilterFormulaBar` is keyed on `activePerspective.id` only (`key={activePerspective.id}`), so a filter change on the *same* perspective does not remount it. See `PerspectiveTabBar()` where `<FilterFormulaBar key={activePerspective.id} filter={activePerspective.filter} ...>` is rendered.
- `kanban-app/ui/src/components/filter-editor.tsx` — `FilterEditorBody` passes `filter` through as `value={filter}` on `TextEditor`, but never reacts to subsequent `filter` prop changes. The only path that imperatively clears the CM6 buffer is `handleClearAndReset` (the inline `×` button), which calls `innerRef.current?.setValue("")`.
- `kanban-app/ui/src/components/fields/text-editor.tsx` — by design, `TextEditorProps.value` is captured only at mount (see the docstring: "Subsequent changes to this prop do NOT reset the document"). The `TextEditorHandle.setValue` is the only way to reset the buffer imperatively.

So when `perspective.clearFilter` fires via any non-inline path:
1. Backend clears filter → emits `entity-field-changed` for the perspective.
2. `PerspectiveProvider` (`kanban-app/ui/src/lib/perspective-context.tsx`) refetches → `activePerspective.filter` becomes undefined.
3. `FilterFormulaBar` re-renders with `filter=""`, but the key is unchanged so it does not remount.
4. `FilterEditorBody` renders with `filter=""`, but `TextEditor` ignores the new prop, so CM6 still shows the old text.

### Approach

Add a prop-to-buffer reconciliation path in `FilterEditorBody` that only fires for *external* filter changes — do **not** run it while the user is typing, because the debounced autosave (`handleChange` → `applyFilter` → `dispatch perspective.filter` → refresh) would otherwise cause the server's echoed value to clobber in-flight keystrokes.

Suggested shape (in `kanban-app/ui/src/components/filter-editor.tsx`):

- [x] Track the last value this editor itself dispatched in a `lastDispatchedRef` inside `useFilterDispatch` (update inside `applyFilter`'s clear and set paths).
- [x] Add a `useEffect` in `FilterEditorBody` watching `filter`. When `filter !== lastDispatchedRef.current` AND `filter !== innerRef.current?.getValue()`, call `innerRef.current?.setValue(filter ?? "")`. This responds only to truly external changes; the editor's own dispatches are filtered out because the ref was just updated to match.
- [x] Update `handleClearAndReset` so its local `setValue("")` remains correct in tandem with the new reconciliation effect (should still work, since `lastDispatchedRef` will equal `""` after `handleClear`).
- [x] If `TextEditorHandle` does not already expose a `getValue()`, extend it in `kanban-app/ui/src/components/fields/text-editor.tsx` (`useTextEditorHandle`). A minimal `getValue(): string` returning `view.state.doc.toString()` is enough. Keep the "mount-time value, buffer is source of truth" invariant intact in the docstring.

Do **not** add `filter` to the `FilterFormulaBar` key — keystroke-driven refetches would remount the editor mid-edit and destroy CM6 state (cursor, selection, vim mode).

Do **not** broaden the fix to all external perspective field changes beyond what is needed for the filter buffer reconciliation — other perspective fields (group, sort, name) do not round-trip through a CM6 buffer.

### Files to modify

- `kanban-app/ui/src/components/filter-editor.tsx` — add `lastDispatchedRef`, add reconciliation `useEffect`.
- `kanban-app/ui/src/components/fields/text-editor.tsx` — add `getValue` to `TextEditorHandle` if missing.

### Related

- Inline `×` button path already works (see `filter-editor.test.tsx::dispatches clearFilter command when clear button is clicked` and the `handleClearAndReset` callback). Do not regress that test.
- Backend command lives in `swissarmyhammer-kanban/src/commands/perspective_commands.rs::ClearFilterCmd` and is wired via `swissarmyhammer-commands/builtin/commands/perspective.yaml` (id `perspective.clearFilter`). No backend change required — the backend already emits the refresh event (verified in `kanban-app/ui/src/lib/perspective-context.tsx::usePerspectiveEventListeners` which listens for `entity-field-changed` with `entity_type === "perspective"`).

## Acceptance Criteria

- [x] After dispatching `perspective.clearFilter` from a non-formula-bar path (context menu / command palette / keybinding) on the active perspective, the formula bar's CM6 editor visibly shows an empty buffer and the filter placeholder text returns.
- [x] The inline `×` clear button on the formula bar continues to work identically (existing test stays green).
- [x] Typing into the formula bar is not disrupted by the new reconciliation effect — characters the user just typed are not clobbered by the echoed backend refresh.
- [x] Switching to a different perspective continues to show that perspective's filter (the existing `key={activePerspective.id}` remount path is preserved).
- [x] `perspective.filter` (set) dispatched from outside (e.g. undo of a clearFilter, or a filter set from another window) also updates the formula bar buffer.

## Tests

Add in `kanban-app/ui/src/components/filter-editor.test.tsx` (or a new `filter-editor.external-clear.test.tsx` if the main file is crowded — follow the existing `filter-editor.delete-scenario.test.tsx` / `filter-editor.scenarios.test.tsx` split pattern):

- [x] `external clearFilter: when filter prop transitions from "#bug" to "" (simulating backend refresh after an external perspective.clearFilter), the CM6 editor buffer is reset to empty`. Use a controlled re-render of `<FilterEditor>` with changing `filter` prop and assert `screen.getByRole("textbox")` (or the CM6 doc contents helper) is empty.
- [x] `external filter set: when filter prop transitions from "#bug" to "@alice", the CM6 editor buffer is updated to "@alice"`.
- [x] `reconciliation does not clobber user typing: when the user types "abc", the debounced save settles, backend refetch sets filter="abc", the buffer remains "abc" (no flicker / no reset)`. This test should use fake timers and the existing mock dispatch infra (`mockInvoke`) and verify the CM6 buffer equals "abc" after advancing timers.

Additionally, add a `kanban-app/ui/src/components/perspective-tab-bar.external-clear.test.tsx` integration test that:

- [x] Renders `PerspectiveTabBar` with a perspective whose `filter` is `#bug`.
- [x] Simulates the perspective-context mock transitioning `activePerspective.filter` from `"#bug"` to `undefined` (emulating refresh after `perspective.clearFilter` from a context menu).
- [x] Asserts the formula bar's editor is empty after re-render.

Run:

- [x] `cd kanban-app/ui && pnpm vitest run filter-editor` — all filter-editor tests green, including the three new external-transition cases.
- [x] `cd kanban-app/ui && pnpm vitest run perspective-tab-bar` — all perspective-tab-bar tests green, including the new integration test.
- [x] `cd kanban-app/ui && pnpm typecheck` — passes. (Note: the project uses `tsc --noEmit` as part of `pnpm test`; `pnpm typecheck` script doesn't exist. Verified clean via `npx tsc --noEmit`.)
- [ ] Manual smoke: right-click a perspective tab with an active filter, pick "Clear Filter" from the context menu, confirm the formula bar goes empty and the placeholder returns. (Unticked per review nit 2 — the box must only be ticked once a human runs this in the actual Tauri GUI; automation cannot drive it.)

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass. #bug #perspectives #frontend #filter #commands

## Review Findings (2026-04-23 14:29)

### Warnings

- [x] `kanban-app/ui/src/components/filter-editor.tsx` `FilterEditorBody` reconciliation effect — the effect's `innerRef.current?.setValue(next)` generates a CM6 doc change, which fires `onChange(next)` → `handleChange(next)` → `schedule(applyFilter(next), 300ms)` → `applyFilter(next)` re-dispatches `perspective.filter` (or `perspective.clearFilter`) with the exact same value that arrived in the prop. Every external mutation round-trips back to the backend from this window as a second dispatch. Two real consequences: (a) `KanbanOperationProcessor` writes an undo-stack entry per dispatch (`UpdatePerspective.execute` writes unconditionally — see `swissarmyhammer-kanban/src/perspective/update.rs` and `.kanban/undo_stack.yaml`); with two open windows, one user-initiated clearFilter in window A yields two undo-stack entries, so two presses of Ctrl+Z are required to reverse it. (b) In multi-window setups the same external mutation bounces back from every other window, N-plicating the stack. Suggestion: in `handleChange`, suppress the scheduled apply when `text.trim() === lastDispatchedRef.current` — since the ref is stamped BEFORE `setValue`, that guard lets true keystrokes (where the ref still holds the *previous* dispatched value) through while suppressing the reconciliation-driven echo and the existing × button's secondary dispatch. Existing tests that assert `>= 1` dispatch on × (test `inline × clear still works after reconciliation is in place`, line 298) would continue to pass; tighten them to assert `=== 1` to lock in the fix.

  **Resolution**: Added the suggested guard in `handleChange` (in `kanban-app/ui/src/components/filter-editor.tsx`): when `text.trim() === lastDispatchedRef.current`, the scheduled apply is suppressed. Also tightened the existing `inline × clear still works after reconciliation is in place` test in `filter-editor.external-clear.test.tsx` from `>= 1` to `=== 1` to lock in the fix. The guard lets real keystrokes through (the ref still holds the PREVIOUS dispatched value during typing) while suppressing both the reconciliation-driven echo and the × button's secondary dispatch. All 1322 tests pass.

### Nits

- [x] `kanban-app/ui/src/components/perspective-tab-bar.external-clear.test.tsx` — add (or rename) a test named after the undo-of-clearFilter acceptance criterion explicitly. The existing "`#bug` → `@alice`" test exercises the same prop-change path, but that case is named "external filter set" and the acceptance-criterion line specifically calls out "undo of a clearFilter" as a motivating example. A clearly-labelled `"undo of clearFilter restores the previous filter"` test would make the coverage trace obvious to a future reader. Mechanical — just clarify the title and maybe start from `undefined` to more literally mirror the undo sequence.

  **Resolution**: Added a new test `undo of clearFilter restores the previous filter: activePerspective.filter transitions from undefined to '#bug' → CM6 buffer shows '#bug'` that starts from `filter: undefined` (the literal cleared state after `perspective.clearFilter`) and transitions back to `filter: "#bug"` (the literal undo sequence). Also updated the comment on the adjacent `#bug → @alice` test to note that the dedicated undo test covers the undo case, leaving that test to cover the plain "external filter set from another window" path.

- [x] `01KPXREV7A94F3KEAXANMNA7MB` description — the final acceptance-criterion checkbox "Manual smoke: … confirm the formula bar goes empty and the placeholder returns" was ticked with a "Deferred to reviewer — cannot drive the interactive Tauri GUI from automation" note. A ticked box with a deferral note is contradictory. Convention here is to leave it unchecked until a human actually runs the smoke in the Tauri GUI; untick it. (Non-blocking — adjust whenever the Tauri build is next opened for any reason.)

  **Resolution**: The `Manual smoke` checkbox is now unticked (`- [ ]`) with a note explaining the reason. The box will be ticked once a human runs it in the Tauri GUI.

- [x] `kanban-app/ui/src/components/filter-editor.tsx` `FilterEditorBody` reconciliation effect comment — the comment block correctly documents the two guards but does not mention the adversarial "filter flaps back to `lastDispatchedRef` during typing" case, where the effect deliberately no-ops and the in-flight debounced save wins over an external assertion. This is a defensible trade-off (typing priority), but it is a non-obvious behavioural contract and worth documenting alongside the existing "clobbering keystrokes" note. One sentence is enough: "If an external source asserts the filter back to `lastDispatchedRef.current` mid-typing, the guard no-ops and the pending debounced save will later overwrite that external assertion."

  **Resolution**: Added a paragraph to the reconciliation effect's comment block in `filter-editor.tsx` documenting the "adversarial edge case — filter flaps back to `lastDispatchedRef` mid-typing", explaining that guard (1) trips, the effect no-ops, and the pending debounced save will overwrite the external assertion. Explicitly calls out the trade-off (typing priority beats stale-but-equal-to-our-last-stamp external assertions).

- [x] `kanban-app/ui/src/components/fields/text-editor.tsx` `TextEditorHandle.getValue` docstring — the return-value contract "Returns an empty string if the editor view has not yet initialised" is correct, but it collapses two states (buffer is empty, and view not mounted) into the same return value. Callers cannot tell them apart. For the one current caller (reconciliation), either state means "do not reset" so the collapse is safe — but if a future caller needs to distinguish, they'll have to thread a separate ready flag. Consider returning `string | undefined` so mount-state is explicit, or add a sentence to the docstring stating that the collapse is intentional and callers that care about mount-state should not use `getValue` for that purpose.

  **Resolution**: Added a "Caveat" paragraph to the `getValue` docstring in `text-editor.tsx` explicitly stating that the empty-string return intentionally collapses "buffer is empty" and "view has not mounted yet" states, explaining why the collapse is safe for the current reconciliation-effect caller (both states produce the same "do not reset the buffer" decision), and advising future callers that need to distinguish the two states to thread a separate ready flag via `onCreateEditor` rather than widening the return type.
