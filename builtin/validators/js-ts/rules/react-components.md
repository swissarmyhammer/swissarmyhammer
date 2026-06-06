---
name: react-components
description: Named prop interfaces, Component+ComponentProps naming, no hardcoded field/entity logic
severity: warn
---

# React Components

- **Named prop interfaces.** Every component gets a `interface FooProps` co-located above it. No anonymous inline object types (`}: { field: FieldDef; value: unknown; ... }`). Even for 2-prop components — the named interface is the documentation.
- **`Component` + `ComponentProps` naming convention.** `EntityCard` gets `EntityCardProps`, not `Props` or `IEntityCardProps`.
- **No hardcoded field/entity logic in components.** The UI is a metadata interpreter. Components dispatch on configured properties (`field.editor`, `field.display`, `field.icon`, `field.sort`) — never on `field.type.kind`, `field.name`, or `entity_type` string comparisons. If a component needs to know something about a field, that information must be a declared property on the field definition, not a hardcoded check.
- **No hardcoded entity type strings.** Don't write `entityType === "tag"` or `entity_type === "board"`. Entity-specific behavior belongs in entity/field definitions (YAML), not React components.
- **No hardcoded field name strings.** Don't write `getStr(entity, "name")` or `getStr(entity, "color")`. Use schema-declared properties like `mention_display_field`, `search_display_field`, or equivalent.
- **No `as Record<string, unknown>` casts on field types.** If you need a property from `field.type` (like `options`, `entity`, `derive`), it should be surfaced as a top-level field property or handled by the backend's `effective_*()` methods before reaching the frontend.
