---
position_column: todo
position_ordinal: b7
title: Unified FieldDisplay dispatcher
---
Create `ui/src/components/fields/field-display.tsx` — a single dispatcher that routes on `field.display` (from YAML config) with a `mode` parameter (`compact` for grid, `full` for inspector).

Dispatches:
- `text` → TextDisplay (truncated in compact, full in full)
- `markdown` → MarkdownDisplay (truncated in compact, ReactMarkdown+GFM in full)
- `badge` → BadgeDisplay (colored badge from SelectOption)
- `badge-list` → BadgeListDisplay (TagPill list for tags, badge list for references)
- `number` → NumberDisplay (right-aligned tabular-nums)
- `date` → DateDisplay (formatted date)
- `color-swatch` → ColorSwatchDisplay (circle + hex in compact, larger in full)
- `avatar` → AvatarDisplay (placeholder for now)

Interface:
```tsx
interface FieldDisplayProps {
  field: FieldDef;
  value: unknown;
  entity: Entity;
  mode: "compact" | "full";
}
export function FieldDisplay({ field, value, entity, mode }: FieldDisplayProps)
```

- [ ] Create field-display.tsx with FieldDisplay dispatcher
- [ ] Move existing cell display components (TextCell, BadgeCell, etc.) into this file or import them, adding mode parameter
- [ ] Refactor CellDispatch to delegate to FieldDisplay with mode="compact"
- [ ] Refactor inspector FieldDispatch read-only paths to delegate to FieldDisplay with mode="full"
- [ ] Handle special cases: computed fields with derive (progress bar), body field tag decorations
- [ ] Run tests