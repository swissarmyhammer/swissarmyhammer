---
assignees:
- claude-code
depends_on:
- 01KM5JYX09RBFTQFD2T715FAEW
position_column: todo
position_ordinal: '8380'
title: Consolidate TagPill into MentionPill as unified pill component
---
## What
Merge TagPill functionality into MentionPill so there's one pill component for all mentionable entity types (#tag, @actor, ^task). TagPill currently has features MentionPill lacks — add them to MentionPill, then replace all TagPill usage.

### Changes to `mention-pill.tsx`:
- Add `taskId?: string` prop (for tag remove command)
- Add description tooltip (from TagPill's tooltip implementation)
- Entity resolution: add slugified matching for task titles (import `slugify`)
- Include `mention_display_field` from schema for smarter field matching
- Add "Remove Tag" context menu command when `entityType === "tag"` and `taskId` is set
- Add inner component pattern with `useContextMenu` (from TagPill)

### Migration — replace TagPill imports:
- `entity-card.tsx` — change `TagPill` → `MentionPill` with `entityType="tag"`, `prefix="#"`, `taskId`
- `badge-list-display.tsx` — change to `MentionPill`
- `editable-markdown.tsx` — remove special case for tag vs non-tag in `mentionComponents` builder

### DependencyPills:
- Could optionally use `MentionPill` internally for the pill rendering, but keep the directional logic (blocked_by=amber, blocks=muted, ⊳/⊲ prefixes) as-is. This is a stretch goal, not required.

### Delete:
- `tag-pill.tsx` — fully replaced by MentionPill
- `remark-tags.ts` — if it's a tag-specific remark plugin that's now handled by generic `remark-mentions.ts`
- Update `tag-pill.test.tsx` → test MentionPill instead

## Acceptance Criteria
- [ ] Only one pill component exists (`MentionPill`)
- [ ] `#tag` pills still show tooltip, context menu (inspect + remove), colors
- [ ] `@actor` pills still show context menu (inspect), colors
- [ ] `^task` pills show context menu (inspect), resolve by slugified title
- [ ] All pill rendering uses the same styled span (no duplication)
- [ ] TagPill is deleted, no remaining imports

## Tests
- [ ] Update `tag-pill.test.tsx` → `mention-pill.test.tsx` with cases for tag, actor, task
- [ ] Verify tag context menu still has "Remove Tag" when taskId is provided
- [ ] Verify `#`, `@`, `^` prefixes all render correctly