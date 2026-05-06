---
assignees:
- wballard
depends_on:
- 01KNZ3ZX03HSEYVAJFGEFTC2ZE
position_column: done
position_ordinal: ffffffffffffffffffffffffffffff8c80
project: pill-via-cm6
title: Extend MentionMeta with displayName (facet refactor)
---
## What

Prepare the CM6 decoration facet to carry display names so a widget can render `$Display Name` (clipped) instead of `$slug`. Today, `createMentionDecorations` receives only a `Map<slug, colorHex>`; the widget needs the raw display name too.

**Files to modify:**
- `kanban-app/ui/src/lib/cm-mention-tooltip.ts` — extend `MentionMeta` to add `displayName: string`. Keep `color` and `description`. Tooltips keep working unchanged.
- `kanban-app/ui/src/hooks/use-mention-extensions.ts` — `buildMetaMap` already builds `{color, description}`; add `displayName: raw` (the un-slugified display name). Rename to reflect that the map is no longer just a tooltip thing — e.g. `buildMentionMetaMap`. `buildColorMap` can go away; its only caller is `createMentionDecorations`, which will switch to metaMap in the next card.
- `kanban-app/ui/src/lib/cm-mention-decorations.ts` — change `colorsFacet` to a `metaFacet` of type `Map<string, MentionMeta>`. Update `decorateLine` / `buildDecorations` / the extension entry point to read color from `meta.get(slug).color`. Rendering behavior is unchanged — this card only refactors the facet shape.

**Shared type location:** `MentionMeta` currently lives in `cm-mention-tooltip.ts` because it was only used for tooltips. Move it to a new shared file `kanban-app/ui/src/lib/mention-meta.ts` so decorations and tooltips both import from the same place without a circular dependency.

**No behavior change.** This card is a pure refactor of the facet plumbing. Visual output of existing editors must be identical before and after.

## Acceptance Criteria
- [ ] `MentionMeta` type lives in `lib/mention-meta.ts` with fields `{ color: string; displayName: string; description?: string }`
- [ ] `cm-mention-tooltip.ts` imports `MentionMeta` from the new location
- [ ] `cm-mention-decorations.ts` uses `metaFacet: Facet<Map<string, MentionMeta>>` instead of `colorsFacet`
- [ ] `use-mention-extensions.ts` builds one `metaMap` per entity type (with `displayName`) and passes it to both decoration and tooltip extensions
- [ ] Existing visual output (colored mark pills) is identical — snapshot tests pass without updates

## Tests
- [ ] Update `kanban-app/ui/src/hooks/__tests__/use-mention-extensions.test.ts` — assert that for an entity type with display field `name`, the produced metaMap entry has `displayName === rawName` (not slugified) for a sample entity
- [ ] Add a direct unit test for `cm-mention-decorations.ts` at `kanban-app/ui/src/lib/cm-mention-decorations.test.ts` — instantiate an EditorView with the extension, feed a doc containing a known mention, and verify the decoration is emitted at the correct position with the correct color attribute (proves the refactored facet still wires color through)
- [ ] Run: `bun test use-mention-extensions cm-mention-decorations` — all pass
- [ ] Smoke: run `bun run dev` and visually confirm tag/actor/task/project pills in existing editors still render with their colors

## Workflow
- Use `/tdd` — start with the failing unit test for `cm-mention-decorations.ts`, watch it fail, implement the facet refactor to make it pass. Then update `use-mention-extensions.ts` to emit the new map shape. Then run the full test suite for mention-related files.