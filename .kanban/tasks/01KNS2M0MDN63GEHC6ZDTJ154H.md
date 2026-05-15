---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffdb80
title: 'Fix formula bar focus: clicking filter icon or bar area must focus the CM6 editor'
---
## What

Three click targets in the perspective formula bar do nothing when they should focus the CM6 filter editor:

1. **Filter icon on the active tab** (`FilterFocusButton`) — calls `filterEditorRef.current?.focus()`, which should work but the ref chain has a gap (see below).
2. **Filter icon inside `FilterFormulaBar`** — rendered as a decorative `<Filter aria-hidden="true">` with no click handler.
3. **Formula bar container div** — `FilterFormulaBar`'s outer div has no `onClick`, so clicking the padding area or the icon does nothing.

### Root cause: broken ref chain in `FilterFormulaBar`

`FilterFormulaBar` (`perspective-tab-bar.tsx`) currently forwards the parent's `ref` directly to `FilterEditor`:

```tsx
// Current — ref is passed straight through; FilterFormulaBar has no local handle
<FilterEditor ref={ref} filter={filter ?? ""} perspectiveId={perspectiveId} />
```

This means the formula bar's own container cannot call `focus()` because it has no reference to the editor — only the outer `filterEditorRef` in `PerspectiveTabBar` holds the handle. Adding an `onClick` to the container requires a local ref.

### Fix: give `FilterFormulaBar` its own `FilterEditorHandle` ref

**`kanban-app/ui/src/components/perspective-tab-bar.tsx` — `FilterFormulaBar` component:**

1. Create a local `editorRef = useRef<FilterEditorHandle>(null)` inside `FilterFormulaBar`.
2. Use `useImperativeHandle` to forward focus through to the outer `filterEditorRef`:
   ```tsx
   useImperativeHandle(ref, () => ({
     focus() { editorRef.current?.focus(); },
   }), []);
   ```
3. Pass the local `editorRef` to `<FilterEditor ref={editorRef} ...>`.
4. Add `onClick={() => editorRef.current?.focus()}` to the container div.
5. Add `cursor-text` class to the container so the cursor signals the area is an editable input.

**`FilterFocusButton` (same file):** No change needed — it already calls `filterEditorRef.current?.focus()`, which now routes through `FilterFormulaBar.useImperativeHandle` → `editorRef.current?.focus()` → `FilterEditor.useImperativeHandle` → `EditorView.focus()`.

**The filter icon** inside `FilterFormulaBar` becomes clickable automatically because its click event bubbles to the container's `onClick`.

### Imports needed

Add `useImperativeHandle` to the import from `"react"` in `perspective-tab-bar.tsx` (it's not currently imported).

## Acceptance Criteria
- [ ] Clicking the Filter icon button on the active perspective tab focuses the formula bar CM6 editor
- [ ] Clicking anywhere in the formula bar area (including the filter icon and padding) focuses the CM6 editor
- [ ] The formula bar container shows `cursor-text` to signal it is editable
- [ ] The ref chain `filterEditorRef → FilterFormulaBar.useImperativeHandle → editorRef → FilterEditor.useImperativeHandle → EditorView.focus()` is complete

## Tests
- [ ] `perspective-tab-bar.test.tsx` — "filter button click focuses formula bar editor": after clicking `button[aria-label="Filter"]`, the `data-testid="filter-editor"` container's `.cm-editor` should have DOM focus or the CM6 view should be focused (use `document.activeElement` assertion or spy on `filterEditorRef.current.focus`)
- [ ] `perspective-tab-bar.test.tsx` — "clicking formula bar container focuses editor": fireEvent.click on the formula bar's container div triggers focus
- [ ] Run: `cd kanban-app/ui && npx vitest run src/components/perspective-tab-bar.test.tsx` — all tests pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.