---
position_column: done
position_ordinal: ffffdf80
title: 'Generic mention system (parameterize #tag into @actor)'
---
Refactor the #tag CM6 infrastructure into a generic mention system parameterized by prefix character, with backend-driven autocomplete. Then create @actor wrappers.

## Architecture
- **Decorations/tooltips**: use a cached slug→color map pushed from backend via Tauri events (synchronous CM6 requirement)
- **Autocomplete**: async Tauri command `search_mentions(entity_type, query)` — CM6 natively supports async completion sources
- **Prefix→entity resolution**: driven by entity type metadata (`mention_prefix`, `mention_display_field`) from schema context

## New files
- `ui/src/lib/mention-finder.ts` — `findMentionsInText(text, prefix, slugs)` returning `MentionHit[]`
- `ui/src/lib/cm-mention-decorations.ts` — generic ViewPlugin parameterized by prefix, CSS class, color facet
- `ui/src/lib/cm-mention-autocomplete.ts` — generic async completion source: takes prefix + Tauri search function, debounces calls to backend
- `ui/src/lib/cm-mention-tooltip.ts` — generic hover tooltip by prefix
- `ui/src/lib/remark-mentions.ts` — generic remark AST transform by prefix

## Refactored files (preserve existing API as thin wrappers)
- `ui/src/lib/tag-finder.ts` → delegates to `mention-finder.ts` with prefix `#`
- `ui/src/lib/cm-tag-decorations.ts` → delegates to generic
- `ui/src/lib/cm-tag-autocomplete.ts` → delegates to generic (async via search_mentions)
- `ui/src/lib/cm-tag-tooltip.ts` → delegates to generic
- `ui/src/lib/remark-tags.ts` → delegates to generic

## Also: CM6 font fix
- `ui/src/lib/cm-keymap.ts` — add `fontFamily: "inherit"` to `.cm-content` in minimalTheme

## Subtasks
- [ ] Fix CM6 font: add fontFamily inherit to minimalTheme
- [ ] Create generic mention-finder with tests
- [ ] Create generic cm-mention-decorations (sync, cached map)
- [ ] Create generic cm-mention-autocomplete (async, Tauri backend)
- [ ] Create generic cm-mention-tooltip
- [ ] Create generic remark-mentions
- [ ] Refactor tag files to delegate (preserve API)
- [ ] Create actor wrappers
- [ ] All existing tag tests pass unchanged
- [ ] Run `npm test`