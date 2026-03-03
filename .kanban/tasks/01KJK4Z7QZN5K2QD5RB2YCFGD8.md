---
title: Add remark plugin for tag pills in display mode
position:
  column: done
  ordinal: b2
---
When viewing a task description (not editing), `#tag` patterns render as colored pill spans via a custom remark plugin for react-markdown.

**New file: ui/src/lib/remark-tags.ts**
- Factory function: `remarkTags(tags: Tag[])` returns a remark plugin
- Plugin walks `text` nodes in the mdast, finds `#tag` patterns, splits into custom `tagReference` node type
- Custom node carries the tag name

**New file: ui/src/components/tag-pill.tsx**
- React component rendering a colored tag pill `<span>`
- Props: `tagId: string`, `tag?: Tag`
- Uses `color-mix(in srgb, #${tag.color} 15%, transparent)` background
- `title` attribute for description hover
- `onDoubleClick` handler (wired to inspector in later card)

**EditableMarkdown integration (editable-markdown.tsx):**
- Pass `remarkTags(tags)` to `remarkPlugins` array alongside `remarkGfm`
- Add `tagReference` to the `components` map, rendering `<TagPill>`
- Only when `tags` prop is provided

**Files:** `ui/src/lib/remark-tags.ts` (new), `ui/src/components/tag-pill.tsx` (new), `ui/src/components/editable-markdown.tsx`

- [ ] Create remark-tags.ts plugin
- [ ] Create TagPill component
- [ ] Wire into EditableMarkdown display mode
- [ ] Visual verification in running app
- [ ] npm run build passes