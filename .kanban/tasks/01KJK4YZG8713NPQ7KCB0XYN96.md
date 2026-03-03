---
title: Add CM6 tag decoration plugin for inline colored pills in editor
position:
  column: done
  ordinal: b1
---
When editing a task description in CodeMirror 6, `#tag` patterns render as colored pill decorations inline.

**New file: ui/src/lib/cm-tag-decorations.ts**
- Export `tagDecorationPlugin(tags: Tag[])` returning a CM6 `ViewPlugin`
- Plugin scans visible ranges for tag regex (JS version of the Rust regex)
- Creates `Decoration.mark()` with class `cm-tag-mark` and inline `style` for `--tag-color`
- Updates decorations on doc changes and viewport changes

**CSS (in theme or global):**
```css
.cm-tag-mark {
  background: color-mix(in srgb, var(--tag-color) 15%, transparent);
  border-radius: 3px;
  padding: 1px 4px;
}
.cm-tag-mark .cm-tag-hash { opacity: 0.5; }
```

**EditableMarkdown integration (editable-markdown.tsx):**
- Accept new optional prop `tags?: Tag[]`
- In multiline edit mode, add tag decoration plugin to extensions
- Memoize plugin creation

**Threading:**
- TaskDetailPanel passes `board.tags` to EditableMarkdown for description field

**Files:** `ui/src/lib/cm-tag-decorations.ts` (new), `ui/src/components/editable-markdown.tsx`, `ui/src/components/task-detail-panel.tsx`

- [ ] Create cm-tag-decorations.ts with ViewPlugin
- [ ] Add CSS for .cm-tag-mark
- [ ] Wire into EditableMarkdown multiline mode
- [ ] Thread tags prop through TaskDetailPanel
- [ ] Visual verification in running app
- [ ] npm run build passes