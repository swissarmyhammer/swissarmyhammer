---
position_column: done
position_ordinal: ef80
title: '[Info] useSchemaOptional and useInspectOptional are well-designed for palette decoupling'
---
Positive observation: The new `useSchemaOptional` (schema-context.tsx) and `useInspectOptional` (inspect-context.tsx) hooks return stub/null when no provider is present. This allows the CommandPalette to work in both full-app context (with schema and inspect) and in test fixtures without requiring all providers.\n\nThe command-palette test suite demonstrates this — `renderPalette` (command mode) works without SchemaProvider/InspectProvider, while `renderSearchPalette` wraps with EntityStoreProvider and InspectProvider when testing search mode.\n\nNo action needed."