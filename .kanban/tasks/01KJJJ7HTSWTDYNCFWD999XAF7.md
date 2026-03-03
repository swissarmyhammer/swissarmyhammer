---
title: Build EditableMarkdown component to replace EditableText everywhere
position:
  column: done
  ordinal: b7
---
Create a new component that replaces `EditableText` for ALL inline editing in the app — task titles, column names, and descriptions. Uses CodeMirror 6 for editing and react-markdown for display in ALL modes.

**Two modes, one component:**
- **`multiline={false}` (default):** Single-line editing for titles and column names. CodeMirror configured with single-line behavior (Enter commits, no line wrapping). Display mode renders markdown via react-markdown (so `**bold**` in a title renders bold).
- **`multiline={true}`:** Multi-line editing for descriptions. CodeMirror with full markdown language support. Display mode renders via react-markdown with remark-gfm.

**Both modes render markdown in display.** Every field — titles, column names, descriptions — shows rendered markdown when not editing.

**Interactive task list checkboxes (multiline display mode):**
Clicking a rendered checkbox toggles it WITHOUT entering edit mode. The implementation:
1. `react-markdown` with `remark-gfm` renders `- [ ]` as unchecked and `- [x]` / `- [X]` as checked checkboxes
2. Override the `input` component: on checkbox click, find the Nth checkbox occurrence in the source markdown string (match by index — the Nth `<input>` in the rendered output corresponds to the Nth `- [ ]` or `- [x]` pattern in the source)
3. Toggle: replace `- [ ]` with `- [x]` or `- [x]`/`- [X]` with `- [ ]` at that position in the source string
4. Call `onCommit(updatedMarkdown)` immediately — this saves without entering edit mode
5. The checkbox click event must `stopPropagation()` so it doesn't trigger the click-to-edit behavior

**Click-to-cursor positioning (preserve existing optimization):**
The current `EditableText` uses `document.caretRangeFromPoint(e.clientX, e.clientY)` to resolve the click position to a character offset, so clicking in the middle of text places the cursor there instead of at the end. This MUST be preserved:
- On click, capture character offset via `caretRangeFromPoint`
- When CM6 editor mounts, set cursor position via `view.dispatch({ selection: { anchor: Math.min(offset, doc.length) } })`
- Fallback: if offset is null, place cursor at end of document

**Behavior (both modes):**
- **Display mode (default):** react-markdown rendered content. Clicking enters edit mode with cursor at click position. Clicking a checkbox toggles it without entering edit mode.
- **Edit mode:** CodeMirror 6 editor, auto-focuses at click position. Blur commits, Escape cancels.
- **Empty state:** Placeholder in muted italic.

**Component API** (superset of EditableText):
```tsx
interface EditableMarkdownProps {
  value: string;
  onCommit: (value: string) => void;
  className?: string;
  inputClassName?: string;
  multiline?: boolean;
  placeholder?: string;
}
```

**CodeMirror setup:**
- Single-line mode: minimal extensions, Enter key commits (custom keymap), no markdown language mode
- Multiline mode: `markdown({ base: markdownLanguage, codeLanguages: languages })` extension
- Both modes: minimal theme (no line numbers, no gutter, transparent bg, auto-height)
- **Keymap mode from `useKeymap()` context** — CM6 Compartment hot-swaps CUA/Vim/Emacs
- Vim/Emacs extensions loaded before other keymaps (highest precedence)

**react-markdown setup (display mode, both single-line and multiline):**
- `remark-gfm` plugin for GFM support
- Single-line: render inline markdown (bold, italic, code, etc.)
- Multiline: full block-level markdown (lists, headings, code blocks, tables)
- Styled via className prop to match existing aesthetics

**Files:**
- `ui/src/components/editable-markdown.tsx` (new)
- `ui/src/components/editable-markdown.test.tsx` (new)

**Tests:**
- Single-line: renders markdown in display, click to edit with CM6, Enter commits, Escape cancels
- Multiline: renders markdown HTML in display, CM6 editor on click, blur commits
- Checkbox toggle: clicking checkbox toggles `- [ ]` ↔ `- [x]` in source and calls onCommit without entering edit mode
- Click-to-cursor: clicking in middle of text positions cursor correctly in editor
- Both: placeholder when empty, no commit if unchanged
- Keymap context integration

## Checklist
- [ ] Create EditableMarkdown with single-line and multiline modes
- [ ] Render ALL display text through react-markdown (titles too, not just descriptions)
- [ ] Implement checkbox toggle: find Nth checkbox in source markdown, toggle `[ ]` ↔ `[x]`, call onCommit, stopPropagation
- [ ] Preserve caretRangeFromPoint click-to-cursor positioning for CM6
- [ ] Wire CodeMirror 6 for both modes (single-line: Enter commits; multiline: markdown lang)
- [ ] Consume useKeymap() context for keymap mode via CM6 Compartment
- [ ] Style to match existing aesthetics (className/inputClassName props)
- [ ] Write tests for both modes + checkbox toggle
- [ ] Verify tests pass