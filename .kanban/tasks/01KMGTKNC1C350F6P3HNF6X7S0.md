---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffff8480
title: Fix typing lag in body CM6 editor — switch to uncontrolled mode while editing
---
## What

`EditableMarkdown` (`kanban-app/ui/src/components/editable-markdown.tsx`) passes `value={draft}` to `@uiw/react-codemirror`, making it a controlled component. Every keystroke triggers: `onChange` → `setDraft(val)` → re-render → CM6 calls `view.state.doc.toString()` to compare with the new `value` prop. This is O(n) per keystroke where n = document length, causing noticeable lag on longer task bodies.

Additionally, these props are unstable (recreated every render), potentially triggering CM6 reconfiguration on each keystroke:
- `basicSetup={{ lineNumbers: false, ... }}` — fresh object literal every render (line 394)
- `onBlur={commitAndExit}` — recreated every keystroke because `commitAndExit` depends on `draft` (line 390)
- `onChange={(val) => setDraft(val)}` — inline arrow function every render (line 389)

**Fix approach (1 file: `editable-markdown.tsx`):**
1. **Remove `value` prop from `<CodeMirror>` while editing** — go uncontrolled. The editor already reads from `editorRef.current.view.state.doc.toString()` at commit time (line 188). Drop `draft` state entirely and remove the `value` prop. Initialize CM6 with the initial value only.
2. **Memoize `basicSetup`** — hoist to a module-level constant or wrap in `useMemo` with empty deps.
3. **Stabilize `onBlur`** — use a ref-based pattern (already done for `commitAndExitRef`, just pass `commitAndExitRef.current` via a stable wrapper).
4. **Stabilize `onChange`** — either remove it (uncontrolled) or wrap in `useCallback`.

**File to modify:**
- `kanban-app/ui/src/components/editable-markdown.tsx`

## Acceptance Criteria
- [ ] Typing in the body CM6 editor has no perceptible lag on a task with 50+ lines of markdown
- [ ] Commit on blur still works (saves content to backend)
- [ ] Commit on Escape (vim) / Enter (CUA) still works
- [ ] Checkbox toggle still works in display mode
- [ ] Mention autocomplete still works (triggers on typing `#` or `@`)

## Tests
- [ ] Update `kanban-app/ui/src/components/editable-markdown.test.tsx`: add a test that types multiple characters rapidly and asserts no CM6 reconfiguration (spy on `view.dispatch` calls, count should equal keystroke count — no extra reconciliation dispatches)
- [ ] Existing tests in `editable-markdown.test.tsx` still pass
- [ ] Existing tests in `editor-save.test.tsx` still pass (blur/Enter/Escape commit paths)
- [ ] Run `cd kanban-app/ui && npx vitest run` — all tests pass #performance