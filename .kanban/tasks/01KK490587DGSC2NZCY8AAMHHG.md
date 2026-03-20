---
position_column: done
position_ordinal: ffffe380
title: Wire @actor into EditableMarkdown and remark display
---
Wire @actor (and all mentionable entity types) into EditableMarkdown and remark display.

## Key insight: markdown fields support ALL mentionable types
The body/description field doesn't know about specific entity types. Instead:
1. Schema context provides `getMentionableTypes()` → all entity types with `mention_prefix`
2. EditableMarkdown registers CM6 extensions for **each** mentionable type
3. Each prefix gets its own facet, completion source, and decorations — no collision
4. `#bug @alice` in body → tag + actor decorations, each resolved independently

## Changes
- `ui/src/components/editable-markdown.tsx` — replace hardcoded tag-only logic with generic loop over mentionable types from schema context. For each: register decorations (cached map), autocomplete (async search_mentions), tooltips.
- New `ui/src/components/actor-pill.tsx` — renders `@name` pill (or generic mention pill)
- `ui/src/components/entity-inspector.tsx` — no longer needs to manually pass tags/actors; EditableMarkdown reads mentionable types from schema context directly

## Subtasks
- [ ] EditableMarkdown reads mentionable types from schema context
- [ ] Loop over mentionable types to register CM6 extensions per prefix
- [ ] Create generic MentionPill component (replaces hardcoded TagPill for display)
- [ ] Update remark plugin to handle multiple prefixes
- [ ] Simplify inspector — remove manual tag/actor prop threading
- [ ] Run `npm test`