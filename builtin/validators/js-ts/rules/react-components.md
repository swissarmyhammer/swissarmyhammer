---
name: react-components
description: Named prop interfaces, Component+ComponentProps naming, no hardcoded field/entity logic
---

# React Components

- **Name every prop interface.** Add an `interface FooProps` above each component. Place this interface next to the component. Do not use anonymous inline object types (`}: { field: FieldDef; value: unknown; ... }`). Add a named interface even for a 2-prop component. The named interface is the documentation.
- **Follow the `Component` + `ComponentProps` naming convention.** For a component named `EntityCard`, name its props interface `EntityCardProps`. Do not name it `Props` or `IEntityCardProps`.
- **Do not hardcode field or entity logic in components.** The UI must work as a metadata interpreter. Components must dispatch on configured properties: `field.editor`, `field.display`, `field.icon`, or `field.sort`. Components must never dispatch on `field.type.kind`, `field.name`, or `entity_type` string comparisons. If a component needs information about a field, declare that information as a property on the field definition. Do not hardcode a check for it.
- **Do not hardcode entity type strings.** Do not write `entityType === "tag"` or `entity_type === "board"`. Put entity-specific behavior in entity or field definitions (YAML). Do not put this behavior in React components.
- **Do not hardcode field name strings.** Do not write `getStr(entity, "name")` or `getStr(entity, "color")`. Use schema-declared properties, such as `mention_display_field` or `search_display_field`.
- **Do not cast field types with `as Record<string, unknown>`.** If you need a property from `field.type`, such as `options`, `entity`, or `derive`, surface it as a top-level field property. Or, handle it in the backend's `effective_*()` methods before it reaches the frontend.
