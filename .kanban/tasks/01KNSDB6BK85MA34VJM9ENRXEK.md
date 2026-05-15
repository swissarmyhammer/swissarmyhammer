---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffe880
title: 'Bug: Filter bar syntax highlighting paints tags green, overriding mention decoration colors'
---
## What

Tags in the filter formula bar CM6 editor show green text (`#085`) instead of
the entity's actual color. The green comes from the Lezer syntax highlighting
layer fighting with the mention decoration layer.

### Root cause

Two systems apply `color` to the same text span, and syntax highlighting wins:

1. **Filter grammar syntax highlighting** — `highlight.ts` maps
   `Tag: t.tagName`. In Lezer's tag hierarchy, `tagName` inherits from
   `typeName`. The `defaultHighlightStyle` (included via `basicSetup` from
   `@uiw/codemirror-extensions-basic-setup`) maps `typeName` → `#085` (green).
   This applies a `color: #085` rule via a generated class.

2. **Mention decoration** — `cm-mention-decorations.ts` applies
   `color: var(--tag-color, #888)` via the `.cm-tag-pill` class.

The syntax highlighting class wins because CM6 applies syntax highlighting
spans **inside** decoration marks, so the inner `<span>` from syntax
highlighting overrides the outer `<span>` from the decoration mark.

### Fix

Remove the `Tag: t.tagName` mapping from the filter grammar's `highlighting`
configuration in `kanban-app/ui/src/lang-filter/highlight.ts`. Tags get their
visual treatment from the mention decoration system (colored pills). The
syntax highlighting is redundant for tags and actively harmful because it
overrides the entity-specific color.

The `Mention: t.variableName` mapping has the same class of issue (it maps
to `#00f` blue in the default style) and should also be removed for the same
reason — actor mentions should be colored by their mention decorations, not
the syntax highlighting.

Keep `Ref`, operator, keyword, and paren mappings — those don't conflict with
mention decorations.

### File to modify

- `kanban-app/ui/src/lang-filter/highlight.ts` — remove `Tag` and `Mention`
  from the `styleTags` mapping so syntax highlighting doesn't override mention
  decoration colors

## Acceptance Criteria

- [ ] `#bug` in the filter bar renders with the tag's entity color (from the
      color map), not green (`#085`)
- [ ] `@alice` in the filter bar renders with the actor's entity color, not
      blue (`#00f`)
- [ ] Operators (`&&`, `||`, `!`) still get syntax highlighting
- [ ] Keywords (`and`, `or`, `not`) still get syntax highlighting
- [ ] Parentheses still get syntax highlighting
- [ ] Unknown tags (not in the color map) render as plain unstyled text (no
      green, no pill — correct behavior since they aren't real entities)

## Tests

- [ ] `kanban-app/ui/src/lang-filter/__tests__/highlight.test.ts` — update
      tests: `#bug` should NOT receive `tok-typeName` class; `@alice` should
      NOT receive `tok-variableName` class; operators/keywords should still
      receive their classes
- [ ] Run: `cd kanban-app/ui && npx vitest run src/lang-filter/__tests__/highlight.test.ts`

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.