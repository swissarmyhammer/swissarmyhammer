---
position_column: done
position_ordinal: h0
title: Replace typed Task/Tag/Column interfaces with schema-driven Entity rendering.
---
#ui

The frontend has hardcoded TypeScript interfaces (Task, Tag, Column, Board) that mirror entity fields as typed properties. Entities are dynamic bags of fields defined by the schema — the UI should use `entity_type` as the discriminant and FieldDef[] for rendering, not OO types.

## Design

One type for all entities:
```typescript
interface Entity {
  entity_type: string  // 'task', 'tag', 'column', 'board', etc.
  id: string
  fields: Record<string, unknown>
}
```

The UI knows what KIND of entity it's showing (entity_type = 'task' for board cards, 'column' for lanes, etc.) but accesses field values through the fields bag, not typed properties. Field metadata (name, editor, display, sort) comes from `get_entity_schema(entity_type)`.

## What changes

### Remove
- `interface Task { id, title, description, position, tags, ... }` 
- `interface Tag { id, name, color, ... }`
- `interface Column { id, name, order, ... }`
- All typed property access like `task.title`, `tag.color`, `column.name`

### Keep
- `entity_type` as the string discriminant — the board knows it renders 'task' entities as cards and 'column' entities as lanes
- `FieldDef`, `EntityDef`, `EntitySchema` — the schema types that describe what fields an entity type has
- `Entity` as the single generic type

### Field access pattern
```typescript
// Instead of: task.title
entity.fields[titleFieldDef.name]

// Instead of: field_name: "title" 
field_name: fieldDef.name
```

### Schema loading
- App loads entity schemas on startup via `get_entity_schema`
- Schema context provides field defs to all components
- Components look up field defs by entity_type, not by hardcoded names

## This blocks
- Phase 3 Grid (column generation from field registry)
- Custom user fields
- Field rename/add/delete in the UI
- Correct field-level undo/redo"