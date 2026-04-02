---
assignees:
- claude-code
depends_on:
- 01KN4NG67RE9YSA4J3Q25YM98R
- 01KN4NGQY7NMEZ0HGDN10RVMWH
position_column: todo
position_ordinal: '8380'
title: 8. CM6 inline JS filter expression editor
---
## What

Add a CodeMirror 6 inline JavaScript editor for perspective filter expressions, triggered from the perspective tab bar.

**Files to create:**
- `kanban-app/ui/src/components/filter-editor.tsx` — CM6 JS editor in a popover

**Files to modify:**
- `kanban-app/ui/package.json` — add `@codemirror/lang-javascript` dependency
- `kanban-app/ui/src/components/perspective-tab-bar.tsx` — add filter icon/button that opens the editor

**Approach:**
- Filter icon (funnel) on the active perspective tab, highlighted when a filter is active
- Click opens a popover/dropdown containing the CM6 editor
- CM6 configured with:
  - `@codemirror/lang-javascript` for syntax highlighting + autocomplete
  - Single-line or few-line mode (compact)
  - Existing `shadcnTheme` and `keymapExtension` from `cm-keymap.ts`
  - Submit/cancel via Enter/Escape (reuse `buildSubmitCancelExtensions` pattern)
- On submit: `backendDispatch({ cmd: "perspective.filter", args: { filter: value, perspective_id } })`
- Clear button: `backendDispatch({ cmd: "perspective.clearFilter", args: { perspective_id } })`
- Shows placeholder text like `(entity) => entity.Status !== "Done"`
- Error feedback: if evaluateFilter throws, show red border / error message

**CM6 is already in the project** (`@uiw/react-codemirror`, `@codemirror/lang-markdown`). Pattern follows `text-editor.tsx`.

## Acceptance Criteria
- [ ] Filter icon visible on active perspective tab
- [ ] Icon highlighted/colored when filter is active
- [ ] Click opens popover with CM6 JS editor
- [ ] JS syntax highlighting works
- [ ] Enter saves filter to perspective via backend command
- [ ] Escape cancels without saving
- [ ] Clear button removes filter
- [ ] Invalid expressions show error feedback

## Tests
- [ ] `kanban-app/ui/src/components/filter-editor.test.tsx` — renders, submit saves, cancel reverts, clear removes filter
- [ ] `pnpm test` from `kanban-app/ui/` passes