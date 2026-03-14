---
position_column: done
position_ordinal: h3
title: Pervasive unsafe field access via `as string` / `as string[]` casts
---
Throughout the UI components, entity fields are accessed via unchecked type assertions: `entity.fields.title as string`, `entity.fields.tags as string[]`, `entity.fields.order as number`, etc. If the backend sends unexpected data (null, missing field, wrong type), these will silently produce `undefined` or runtime errors.

Consider adding a small set of typed accessor helpers like:
```ts
function getStr(entity: Entity, field: string): string
function getStrList(entity: Entity, field: string): string[]
function getNum(entity: Entity, field: string, fallback: number): number
```

This mirrors the Rust side which already has `entity.get_str()` and `entity.get_string_list()`. Files affected: board-view.tsx, column-view.tsx, task-card.tsx, task-detail-panel.tsx, tag-inspector.tsx, tag-pill.tsx, nav-bar.tsx. #warning