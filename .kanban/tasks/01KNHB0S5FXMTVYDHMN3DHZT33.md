---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffab80
title: 'Fix CM6 autosave exiting edit mode: onBlur should save-in-place, not commit-and-exit'
---
## What

The CM6 `TextEditor` in `kanban-app/ui/src/components/fields/text-editor.tsx` exits edit mode on every autosave because `onBlur` (line 292) calls `commitAndExit()`, which calls `onCommit(text)`, which triggers `Field.handleCommit()` → `onDone()` → edit mode ends. The user has to re-enter edit mode after every blur/autosave.

The debounced autosave via `onChange` already saves without exiting — it goes through `useDebouncedSave` which calls `updateField` directly without `onDone`. But `onBlur` bypasses this and goes through the commit-and-exit path.

### The fix

**`kanban-app/ui/src/components/fields/text-editor.tsx`**:

1. Change `handleBlur` (line 292) to save-without-exit instead of commit-and-exit. It should call `onChangeRef.current?.(text)` (which feeds the debounced autosave) or flush any pending debounced save, but NOT call `onCommit`.

2. However, `TextEditor` doesn't own the debounced save — `Field` does. `TextEditor` receives `onChange` as a prop (line 85) which feeds into `debouncedOnChange` from `Field`. So the fix is: `handleBlur` should call `onChange(currentText)` to feed the debounce, then flush. But `TextEditor` doesn't have `flush` — it only has `onChange`.

   **Better approach**: Add a `onBlurSave` prop or change `handleBlur` to call `saveInPlace()` (line 194) instead of `commitAndExit()`. `saveInPlace` already exists and calls `onCommit` with the current text — but that still triggers `onDone`.

   **Cleanest fix**: The problem is in `Field.handleCommit()` (line 130 of `field.tsx`) — it ALWAYS calls `onDone()`. Instead, `Field` should provide TWO callbacks to editors:
   - `onCommit(value)` — save AND exit (explicit user action: Escape, Enter-submit)
   - `onChange(value)` — save without exit (autosave, blur, debounced)

   The `onChange` path already exists and works correctly via `debouncedOnChange`. The fix is just in `TextEditor`: change `handleBlur` to use the `onChange` path instead of `onCommit`.

3. **`kanban-app/ui/src/components/fields/text-editor.tsx`** — Change `handleBlur` (line 292-294):
   ```typescript
   const handleBlur = useCallback(() => {
     // Save without exiting edit mode — autosave on blur.
     // onChange feeds the debounced save in Field.
     if (!committedRef.current && editorRef.current?.view) {
       const text = editorRef.current.view.state.doc.toString();
       onChangeRef.current?.(text);
     }
   }, []);
   ```

4. **`kanban-app/ui/src/components/fields/text-editor.tsx`** — Also change `saveInPlace` (line 194) to use `onChange` instead of `onCommit`, so vim insert→normal Escape saves without exiting:
   ```typescript
   const saveInPlace = useCallback(() => {
     if (!editorRef.current?.view) return;
     const text = editorRef.current.view.state.doc.toString();
     if (text !== valueRef.current) {
       onChangeRef.current?.(text);
     }
   }, []);
   ```

5. The `MarkdownEditorAdapter` in `kanban-app/ui/src/components/fields/registrations/markdown.tsx` (line 35) passes `onChange` through, so this will work.

### What stays the same

- `commitAndExit()` still exists for explicit exit actions (CUA Escape, vim normal-Escape, Enter-submit in compact mode)
- `cancelAndExit()` unchanged
- `useDebouncedSave` in `field.tsx` unchanged — it already handles `onChange` correctly

## Acceptance Criteria

- [ ] Blurring the CM6 editor saves content but does NOT exit edit mode
- [ ] Typing and pausing triggers debounced autosave without exiting edit mode
- [ ] Pressing Escape (CUA) or normal-mode Escape (vim) exits edit mode with save
- [ ] Enter in compact/single-line mode exits edit mode with save
- [ ] No data loss — pending changes are always saved before or during exit

## Tests

- [ ] `kanban-app/ui/src/components/fields/text-editor.test.tsx` — add test: blur event triggers onChange, not onCommit
- [ ] `kanban-app/ui/src/components/fields/text-editor.test.tsx` — add test: Escape still triggers onCommit (commit-and-exit)
- [ ] `kanban-app/ui/src/components/fields/editors/editor-save.test.tsx` — verify autosave doesn't call onDone
- [ ] Run `npm test` in kanban-app — all pass

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.
