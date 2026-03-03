---
title: Install CodeMirror 6 and react-markdown dependencies
position:
  column: done
  ordinal: b5
---
Install the npm packages needed for markdown editing, rendering, and keybinding modes in the Tauri app's UI.

**Packages to install (dependencies):**
- `@uiw/react-codemirror` — React wrapper for CodeMirror 6
- `@codemirror/lang-markdown` — Markdown language support
- `@codemirror/language-data` — Language data for fenced code block highlighting
- `react-markdown` — Renders markdown as React components (no dangerouslySetInnerHTML)
- `remark-gfm` — GitHub Flavored Markdown plugin (tables, strikethrough, task lists)
- `@replit/codemirror-vim` — Vim keybinding mode for CM6
- `@replit/codemirror-emacs` — Emacs keybinding mode for CM6

**Working directory:** swissarmyhammer-kanban-app/ui/
**Verify:** `npm install` succeeds, `npm run build` still passes

## Checklist
- [ ] npm install all listed packages
- [ ] Verify build still works
- [ ] Verify types resolve (no TS errors)