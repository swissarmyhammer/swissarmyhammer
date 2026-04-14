---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffcb80
project: task-card-fields
title: Fix multi-select + vim + Enter combo in editor-save test matrix
---
## What

The data-driven save-behavior matrix in `kanban-app/ui/src/components/fields/editors/editor-save.test.tsx` (lines ~338–347) skips a single matrix cell with a bare `continue`:

```ts
// Multi-select + vim + Enter: capture-phase listener timing
// differs from test expectations. Skip this specific combo.
if (
  fieldDef.editor === "multi-select" &&
  keymap === "vim" &&
  exit === "Enter"
) {
  continue;
}
```

This skip was introduced on 2026-04-04 (e322ba0e2) and the comment was clarified on 2026-04-06 (a19feb74e). It pre-dates the current wave of work but the `no-test-cheating` validator now requires either a linked tracking reference, a platform-specific condition, or deletion. This card is the linked reference.

### Why it skips

`multi-select-editor.tsx` wires Enter through CM6's `buildSubmitCancelExtensions` keymap. In vim mode, CM6's vim plugin installs a capture-phase listener that competes with our submit extension. The test harness dispatches a native `KeyboardEvent` on the `.cm-content` node, but in this specific combination the event reaches vim's handler first and never triggers our commit path — the test's expected `dispatch_command` call never fires. Real-world usage works; only the test-harness event routing misses.

### Approach

The correct fix is NOT to swallow the skip — it's to drive Enter through the same channel the test currently uses for Popover editors: locate the real CM6 content node in a portal-aware way, then dispatch through the CM6 `view.contentDOM` exactly as vim's own keymap expects. Specifically:

1. Inside the `exit === "Enter"` branch of the harness (`editor-save.test.tsx`), detect `fieldDef.editor === "multi-select"` and, instead of the generic `fireEvent.keyDown(content, ...)`, reach into the EditorView via `content.cmView?.view` or the `EditorView.findFromDOM(content)` helper and dispatch through `view.dispatch({ effects: [insertNewlineEffect] })` — or equivalently, use `@testing-library/user-event` (already present) in its native mode to type Enter, which routes through the view's `contentDOM` focus + composition layer the way vim expects.
2. Remove the `continue` skip. Verify all nine vim×Enter combos (three modes × three exits is the wider matrix — this specific cell) now pass.
3. If (1) isn't enough, investigate whether `buildSubmitCancelExtensions` needs a capture-phase complement for vim parity, but do NOT touch product code unless a matching behavior gap exists in real usage (check via a manual browser run).

### Files

- `kanban-app/ui/src/components/fields/editors/editor-save.test.tsx` — harness fix
- Reference: `kanban-app/ui/src/components/fields/editors/multi-select-editor.tsx` (uses `buildSubmitCancelExtensions`)
- Reference: `kanban-app/ui/src/lib/cm6-submit-cancel.ts` (or wherever the submit helper lives — grep for `buildSubmitCancelExtensions`)

### Non-goals

- Do not change product behavior. Multi-select Enter already commits correctly in real usage.
- Do not expand scope to other skipped combos (there are none as of this writing).

## Acceptance Criteria

- [x] The `continue` skip in `editor-save.test.tsx` at ~line 341 is removed.
- [x] Running `cd kanban-app/ui && pnpm test -- editor-save` produces no multi-select + vim + Enter failures.
- [x] No regressions in the other 35 matrix combos (modes × keymap × exit × field types).
- [x] No product-code changes — the fix lives entirely in the test harness.

## Tests

- [x] Update `editor-save.test.tsx` — remove the skip; verify the multi-select + vim + Enter combo now drives the commit path and records a `dispatch_command` call with `cmd: "entity.update_field"`.
- [x] Run: `cd kanban-app/ui && pnpm test -- editor-save` → green, and the reported combo count equals `keymapModes.length * exitPaths.length * modes.length` entries all asserting at least one save call per editable field that should save.
- [x] Run: `cd kanban-app/ui && pnpm test` → full suite still green.

## Workflow

- Use `/tdd`: remove the skip first (RED — expect the multi-select+vim+Enter failure), then add the EditorView-aware Enter dispatch in the harness (GREEN), then confirm full-suite green.

## Implementation Notes

The final fix has three parts, all confined to `editor-save.test.tsx`:

1. **Removed the skip** — deleted the `skipMultiSelect` filter and the advertised `it.skip(...)` block so every multi-select × vim × Enter cell now runs.
2. **Route Enter through `EditorView.contentDOM`** — for any CM6-backed target (detected via `target.closest(".cm-editor")` + `EditorView.findFromDOM`), dispatch a native `KeyboardEvent` on `view.contentDOM` instead of using `fireEvent.keyDown` on the container-queried `.cm-content`. This mirrors the proven pattern in `multi-select-editor.test.tsx` and works uniformly across cua/emacs/vim. `fireEvent` still covers non-CM6 fallbacks (plain inputs/selects).
3. **Non-empty `depends_on` initial value** — added `depends_on: ["test-task-2"]` (plus a second task entity for the reference to resolve against) so every multi-select renders a non-empty doc. The vim `alwaysSubmitOnEnter` path guards on `text.length > 0` (see `buildVimEnterExtension` in `cm-submit-cancel.ts`); an empty doc would correctly refuse to commit in vim but save in cua — an unrelated behavior asymmetry outside the save-matrix's scope.

Verification: 24 matrix tests pass (18 keymap×exit×mode cells + 6 display/coverage), full suite 1061/1061 green, `tsc --noEmit` clean. Deterministic across multiple consecutive runs.
