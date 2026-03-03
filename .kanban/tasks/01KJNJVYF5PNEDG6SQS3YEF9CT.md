---
title: Define core field types and serde
position:
  column: done
  ordinal: c6
---
Implement the core data types for `swissarmyhammer-fields` in `src/types.rs`. All types must serialize to/from YAML via serde.

**Types to implement:**

`FieldType` enum — kind + type-specific config:
- `Text { single_line: bool }`
- `Markdown { single_line: bool }`
- `Date`
- `Number { min: Option<f64>, max: Option<f64> }`
- `Color`
- `Select { options: Vec<SelectOption> }`
- `MultiSelect { options: Vec<SelectOption> }`
- `Reference { entity: String, multiple: bool }` — stores entity IDs
- `Computed { derive: String }` — read-only, no stored triple

`SelectOption` struct — `value`, `label` (optional), `color` (optional), `icon` (optional), `order`

`Editor` enum — `Markdown`, `Select`, `MultiSelect`, `Date`, `ColorPalette`, `Number`, `None`

`Display` enum — `Markdown`, `Badge`, `BadgeList`, `Avatar`, `Date`, `ColorSwatch`, `Number`, `Text`

`SortKind` enum — `Alphanumeric`, `OptionOrder`, `Datetime`, `Numeric`

`FieldDef` struct — `id: Ulid`, `name: String`, `description: Option<String>`, `type_: FieldType`, `default: Option<String>`, `editor: Option<Editor>`, `display: Option<Display>`, `sort: Option<SortKind>`, `filter: Option<String>`, `group: Option<String>`, `validate: Option<String>`

`EntityDef` struct — `name: String`, `body_field: Option<String>`, `fields: Vec<String>`

**Key design points:**
- All names/properties snake_case (single_line not singleLine)
- Task body field is `body`, tag description is `description` — no conflict
- `assignees` (plural, multiple: true) not singular assignee
- `validate` stores a JS function body as a string
- serde rename for YAML-friendly keys (e.g., `type_` → `type`, `kind` tag for FieldType)
- Editor/display inference from type (see architecture doc)

**Subtasks:**
- [ ] Implement FieldType enum with serde tagging (including Reference and Computed)
- [ ] Implement SelectOption struct
- [ ] Implement Editor, Display, SortKind enums
- [ ] Implement FieldDef struct with all properties
- [ ] Implement EntityDef struct
- [ ] Add editor/display inference method on FieldDef
- [ ] Write unit tests for YAML round-trip serialization
- [ ] Verify built-in field definitions from architecture doc serialize correctly