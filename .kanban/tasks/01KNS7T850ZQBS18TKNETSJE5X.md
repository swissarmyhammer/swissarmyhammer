---
assignees:
- claude-code
position_column: todo
position_ordinal: 7f80
title: 'Bug: Virtual tags (#READY, #BLOCKED, #BLOCKING) not decorated in CM6 editors'
---
## What

Virtual tags like `#READY`, `#BLOCKED`, `#BLOCKING` are not styled as tag pills
in CM6 editors (e.g. the filter formula bar). Real tags like `#bug` get colored
pill decorations; virtual tags render as plain text.

### Root cause

The decoration system in `cm-mention-decorations.ts` scans for `#slug` patterns
where slugs come from a `colors` Map (line 56: `const slugs = Array.from(colors.keys())`).
This map is built by `buildColorMap` in `use-mention-extensions.ts`, which only
iterates over real tag entities from the entity store. Virtual tag slugs (READY,
BLOCKED, BLOCKING) are never added to the color map.

Virtual tags are only wired into the **autocomplete** path (via
`buildVirtualTagSearch` wrapping the base search), not the **decoration** path.

### Fix

In `useMentionExtensions` (`kanban-app/ui/src/hooks/use-mention-extensions.ts`),
when `includeVirtualTags` is true and processing the `#` prefix mentionable type,
merge virtual tag entries into the `colorMap` before passing it to
`decoInfra.extension(colorMap)`. Also merge into `metaMap` for tooltip support.

Specifically, inside the `for (const md of mentionData)` loop (around line 189),
after building `md.colorMap`, check if `includeVirtualTags && md.prefix === "#"`,
and if so, add each `VIRTUAL_TAG_SLUGS` entry with `VIRTUAL_TAG_COLOR` to the
map. This is the same color already used in autocomplete results.

### Files to modify

- `kanban-app/ui/src/hooks/use-mention-extensions.ts` — merge virtual tag
  slugs + color into `colorMap` and `metaMap` before passing to decoration
  and tooltip extensions (inside the `useMemo` at line 184)

## Acceptance Criteria

- [ ] `#READY`, `#BLOCKED`, `#BLOCKING` render as colored pills in the filter
      formula bar CM6 editor (same styling as real tags like `#bug`)
- [ ] Virtual tags are decorated with `VIRTUAL_TAG_COLOR` (`7c3aed`)
- [ ] Real tag decorations are unaffected
- [ ] Editors without `includeVirtualTags: true` (e.g. task description fields)
      do NOT decorate virtual tag slugs
- [ ] Tooltips work on virtual tags when hovering

## Tests

- [ ] `kanban-app/ui/src/hooks/__tests__/use-mention-extensions.test.ts` — add
      test verifying that when `includeVirtualTags: true`, the returned extensions
      include virtual tag slugs in the decoration color map
- [ ] Run: `cd kanban-app/ui && npx vitest run src/hooks/__tests__/use-mention-extensions.test.ts`

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.