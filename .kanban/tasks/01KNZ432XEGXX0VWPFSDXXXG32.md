---
assignees:
- wballard
position_column: done
position_ordinal: ffffffffffffffffffffffffffffff8980
project: pill-via-cm6
title: 'TextViewer component: read-only CM6 mount'
---
## What

Create a minimal, read-only CM6 component that other consumers can use to render any text (plain or markdown) with CM6 extensions. This is the foundation for migrating `MarkdownDisplay`, `BadgeDisplay`, and `BadgeListDisplay` away from React markdown/pill rendering in the cards that follow.

**Why a new component instead of extending `TextEditor`:** `TextEditor` is built around editing — imperative focus handle, vim mode, commit/cancel callbacks, submit refs, uncontrolled doc management. Adding a read-only branch would bloat it. `TextViewer` is a completely separate, much simpler component that shares no runtime concerns with the editor path.

**Files to create:**
- `kanban-app/ui/src/components/text-viewer.tsx` — new component.

**Props:**
```ts
interface TextViewerProps {
  /** Document content; fully controlled — re-mounts (or reconfigures) when text changes */
  text: string;
  /** CM6 extensions to attach (e.g. mention decorations, markdown language, checkbox plugin) */
  extensions?: Extension[];
  /** Optional className for the wrapping div */
  className?: string;
}
```

**Implementation notes:**
- Use `@uiw/react-codemirror` the same way `TextEditor` does, with:
  - `editable={false}`
  - `readOnly={true}` (via extensions: `EditorState.readOnly.of(true)` and `EditorView.editable.of(false)`)
  - `basicSetup={{ lineNumbers: false, foldGutter: false, highlightActiveLine: false, highlightActiveLineGutter: false, indentOnInput: false, bracketMatching: false, autocompletion: false }}`
  - No keymap extension, no submit/cancel, no onChange, no vim machinery
  - Same `shadcnTheme` as TextEditor for visual consistency
- Wrap the CodeMirror component in `memo` so upstream re-renders don't rebuild the view when `text` and `extensions` are referentially stable.
- `className` defaults to `"text-sm"` to match `TextEditor` visual baseline, but callers can override.

**Edge cases:**
- Empty `text`: render nothing (not even an empty CM6 mount — return `null` or a placeholder span). Callers like `MarkdownDisplay` already handle the empty case in their own wrapper.
- Very short single-line text (e.g. one pill for a scalar reference field): still works — just shows one line, no gutters or chrome.

**Out of scope:**
- Markdown checkbox interaction (goes in the MarkdownDisplay migration card).
- The mention extension plumbing itself (`useMentionExtensions` already exists).

## Acceptance Criteria
- [ ] New component `TextViewer` at `components/text-viewer.tsx`
- [ ] Renders passed `text` in a CM6 editor with `editable=false` / `readOnly=true`
- [ ] Applies caller-provided `extensions` array
- [ ] No keymap, no gutters, no line numbers, no autocompletion
- [ ] Empty `text` returns no output
- [ ] Re-rendering parent with same props doesn't cause CM6 reconstruction (memoized)

## Tests
- [ ] `kanban-app/ui/src/components/text-viewer.test.tsx` (new) — render with plain text, assert the text appears in the DOM
- [ ] Render with an empty string, assert nothing (or a known placeholder) is rendered
- [ ] Render with a simple CM6 decoration extension (e.g. a test decoration that adds a marker class to a specific range) and assert the marker class appears in the rendered DOM — proves extensions are wired
- [ ] Render, then re-render with same props; assert the CM6 EditorView instance is the same (via a ref) — proves memoization
- [ ] Run: `bun test text-viewer` — all pass

## Workflow
- Use `/tdd` — start with the simplest test (render plain text, assert it appears), then the extension-wiring test, then the memoization test.
