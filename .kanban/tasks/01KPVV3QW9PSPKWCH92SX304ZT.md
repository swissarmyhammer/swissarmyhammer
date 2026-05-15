---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffce80
title: 'FilterEditor: sync CM6 buffer to external perspective.filter changes'
---
## What

The formula-bar `FilterEditor` does not reflect changes to `activePerspective.filter` that originate outside its own CM6 buffer. When `perspective.clearFilter` fires from the context menu or command palette — or when `perspective.filter` is dispatched from anywhere other than the editor itself — the backend updates, the cards re-render correctly, but the formula bar keeps showing the stale filter text.

Root cause is in `kanban-app/ui/src/components/fields/text-editor.tsx` — `TextEditorProps.value` is documented as captured at mount only ("Initial buffer value. Subsequent changes to this prop do NOT reset the document"). `FilterEditorBody` in `kanban-app/ui/src/components/filter-editor.tsx` passes `filter` straight to `<TextEditor value={filter} />`, so after mount the CM6 buffer and the prop diverge on any external update.

The `×` button works today only because it calls `innerRef.current?.setValue("")` imperatively in `handleClearAndReset`. That is not a solution — it is a hard-coded special case for one UI path. Context-menu `perspective.clearFilter`, command-palette `perspective.filter`, and cross-window `entity-field-changed` events all go through the prop path and are broken.

**Do not** add per-command branches ("if clear-filter then reset"). The fix must be generic: the formula bar is a view of `perspective.filter`, and whenever that value changes from any source other than this editor's own typing, the buffer must resync.

### Approach

In `kanban-app/ui/src/components/filter-editor.tsx`:

- [x] Track the last value this editor has emitted (via `onChange` → `applyFilter`) in a ref — call it `lastDispatchedRef`. Update it inside `applyFilter` immediately before the `dispatchFilter` / `dispatchClearFilter` call lands.
- [x] Add a `useEffect` in `FilterEditorBody` keyed on the `filter` prop: if `filter !== lastDispatchedRef.current`, call `innerRef.current?.setValue(filter ?? "")`. `TextEditorHandle.setValue` already guards with `if (view.state.doc.toString() === text) return` (text-editor.tsx inside `useTextEditorHandle`), so echoed-back dispatches and no-op renders are cheap.
- [x] Drop the explicit `innerRef.current?.setValue("")` reset in `handleClearAndReset` — the new sync effect handles both the local × button and the backend-originated clears through the same path. The × button keeps its role of cancelling debounce and dispatching `perspective.clearFilter`; the buffer reset follows from the prop update like any other external change.
- [x] Preserve the `[filter-diag]` console.warn instrumentation pattern used throughout the file (`[filter-diag] sync EXTERNAL`, `[filter-diag] sync SKIP_SELF`) so the generic sync is as observable as the rest of the pipeline.

### Why this is generic

Any source that mutates `perspective.filter` — context menu, palette, rename/delete cascades, cross-window sync via `entity-field-changed` (see `kanban-app/ui/src/lib/perspective-context.tsx`) — flows through `usePerspectivesFetch.refresh()` → new `PerspectiveDef[]` → `activePerspective.filter` prop change → the single sync effect. No callsite needs to know the buffer exists.

### Non-goals / don't-do

- Do not add a `key={activePerspective.filter}` remount on `FilterFormulaBar` — that would lose cursor and focus on every keystroke round-trip.
- Do not touch `TextEditor` itself. Its mount-once-value semantics are load-bearing for other consumers (see file-level docstring in text-editor.tsx). The sync policy belongs in `FilterEditor`, the caller that wants it.
- Do not add per-command logic in the effect. `lastDispatchedRef` is the full filter; equality to it is the only signal needed.
- Group-by has the same shape (`GroupPopoverButton` on the active tab), but the group selector is a popover with its own controlled state — out of scope for this task.

## Acceptance Criteria

- [x] Dispatching `perspective.clearFilter` from outside the `FilterEditor` (context menu, palette) clears the formula bar's visible text. Verified manually and in a browser-mode test.
- [x] Dispatching `perspective.filter` from outside the editor updates the formula bar to the new expression. Verified manually and in a test.
- [x] User typing with autosave round-trip does NOT clobber in-flight input. The ref-based echo suppression keeps the CM6 buffer stable across the prop → state → prop cycle.
- [x] The `×` button still clears the filter and closes the editor. Its observable behavior is unchanged even though the explicit `setValue("")` call is gone.
- [x] No `if (command === "clearFilter")` or similar hard-coded branches anywhere. One code path covers every external update.

## Tests

- [x] `kanban-app/ui/src/components/filter-editor.external-clear.test.tsx` — test: render with `filter="#bug"`, re-render with `filter=""`, assert the CM6 buffer is empty. Mimics `perspective.clearFilter` arriving via refreshed perspective state.
- [x] `kanban-app/ui/src/components/filter-editor.external-clear.test.tsx` — test: render with `filter=""`, re-render with `filter="@alice"`, assert the CM6 buffer shows `@alice`. Mimics `perspective.filter` dispatched from the command palette.
- [x] `kanban-app/ui/src/components/filter-editor.external-clear.test.tsx` — test: simulate user typing `abc`, let the debounced `perspective.filter` dispatch fire, then let the parent re-render with the echoed-back value. Assert the buffer still reads `abc` and the cursor/editor state is not reset. Proves `lastDispatchedRef` suppresses self-echo.
- [x] `kanban-app/ui/src/components/filter-editor.external-clear.test.tsx` — × clear test rewritten to assert the buffer reset rides on the prop round-trip rather than an imperative `setValue("")`; `RoundTripParent` in `filter-editor.delete-scenario.test.tsx` updated to also echo `perspective.clearFilter` back into the `filter` prop, matching production wiring.
- [x] Run `cd kanban-app/ui && npx vitest run filter-editor` — 67 tests pass.
- [x] Run `cd kanban-app/ui && npx tsc --noEmit` — no type errors.
- [x] Run `cd kanban-app/ui && npm test` — 2085 tests pass.
- [x] Run `cargo check --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace --no-fail-fast` — all green, 13482 Rust tests pass.

## Workflow

- Use `/tdd` — write the three new failing tests first (external clear, external set, self-echo suppression), then implement `lastDispatchedRef` + sync effect until green.
- After tests pass, verify in the running app: start the kanban app, set a filter on a perspective, right-click and pick `perspective.clearFilter` from the context menu, confirm the formula bar empties alongside the cards.
- Keep the `[filter-diag]` instrumentation consistent with the rest of the file — this diagnostic channel is in active use.