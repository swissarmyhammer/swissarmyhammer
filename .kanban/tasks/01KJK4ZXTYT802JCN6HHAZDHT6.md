---
title: Add hover tooltips and tag inspector popover
position:
  column: done
  ordinal: b4
---
Hovering over a `#tag` in CM6 shows description tooltip. Double-clicking opens a Tag Inspector popover for rename, color, and description editing. Same inspector accessible from TagPill in display mode.

**New file: ui/src/lib/cm-tag-tooltip.ts**
- Export `tagHoverTooltip(tags: Tag[])` returning a CM6 `hoverTooltip` extension
- On hover over `.cm-tag-mark` decoration range, look up tag, show description if non-empty
- Return `null` for tags with no description (no empty bubble)

**New file: ui/src/components/tag-inspector.tsx**
- Popover component (Radix Popover or custom positioned div)
- Three fields:
  1. Tag name (editable input) — rename triggers backend RenameTag
  2. Description (small textarea) — saves via UpdateTag
  3. Color palette (12 swatches from TAG_PALETTE + custom picker) — saves via UpdateTag
- Anchored to the tag's DOM rect
- Dismisses on click outside or Escape

**CM6 double-click handler:**
- Add `EditorView.domEventHandlers({ dblclick })` to tag decoration plugin
- Detect click on `.cm-tag-mark`, extract tag name, open inspector

**TagPill integration:**
- Wire `onDoubleClick` on TagPill component to open same inspector

**Tauri commands:**
- Add `rename_tag` command in `swissarmyhammer-kanban-app/src/commands.rs` delegating to RenameTag operation
- Add `update_tag` command if not already exposed

**Files:** `ui/src/lib/cm-tag-tooltip.ts` (new), `ui/src/components/tag-inspector.tsx` (new), `ui/src/lib/cm-tag-decorations.ts` (add dblclick), `ui/src/components/tag-pill.tsx` (add dblclick), `ui/src/components/editable-markdown.tsx`, `src/commands.rs`

- [ ] Create cm-tag-tooltip.ts hover extension
- [ ] Create tag-inspector.tsx popover component
- [ ] Add double-click handler to CM6 decorations
- [ ] Add double-click handler to TagPill
- [ ] Add rename_tag and update_tag Tauri commands
- [ ] Wire tooltip extension into EditableMarkdown
- [ ] Visual verification in running app
- [ ] npm run build passes