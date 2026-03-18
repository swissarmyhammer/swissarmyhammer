---
position_column: done
position_ordinal: ffffff9680
title: MultiSelectEditor uses unsafe type cast for computed field detection
---
swissarmyhammer-kanban-app/ui/src/components/fields/editors/multi-select-editor.tsx:51-53\n\nThe computed field detection uses `(field.type as Record<string, unknown>).derive` which is a raw cast bypassing TypeScript's type system. If the FieldDef type discriminant shape ever changes, this silently breaks at runtime instead of failing at compile time.\n\nSuggestion: Use a type guard or check the discriminant properly:\n```ts\nconst isComputedTags =\n  field.type.kind === \"computed\" &&\n  \"derive\" in field.type &&\n  field.type.derive === \"parse-body-tags\";\n```\nOr better, add a `derive` field to the computed variant in the TypeScript FieldType union if it doesn't already exist." #review-finding #warning