---
assignees:
- claude-code
depends_on:
- 01KN2Q67HEFFEM63KJ5V006ASD
position_column: done
position_ordinal: ffffffffffffffffff9a80
title: 'PERSP-7: Perspective YAML command definitions + Command trait impls'
---
## What

Define perspective commands in YAML and implement their Command trait handlers. The spec (lines 893-926) defines these commands:

**New YAML file: `swissarmyhammer-commands/builtin/commands/perspective.yaml`:**
```yaml
- id: perspective.load
  name: Load Perspective
  params:
    - name: name
      from: args
  keys: {}

- id: perspective.save
  name: Save Perspective
  params:
    - name: name
      from: args
  keys: {}

- id: perspective.delete
  name: Delete Perspective
  params:
    - name: name
      from: args
  keys: {}

- id: perspective.filter
  name: Set Filter
  keys: {}

- id: perspective.clearFilter
  name: Clear Filter
  keys: {}

- id: perspective.group
  name: Set Group
  keys: {}

- id: perspective.clearGroup
  name: Clear Group
  keys: {}
```

**New file: `swissarmyhammer-kanban/src/commands/perspective_commands.rs`:**
- `LoadPerspectiveCmd` — reads perspective by name, returns full config (view + fields + filter/group/sort)
- `SavePerspectiveCmd` — creates/updates perspective from current state snapshot (args: name, view, fields, filter, group, sort)
- `DeletePerspectiveCmd` — deletes by name
- `SetFilterCmd` — updates active perspective's filter (args: filter string)
- `ClearFilterCmd` — clears active perspective's filter
- `SetGroupCmd` — updates active perspective's group (args: group string)
- `ClearGroupCmd` — clears active perspective's group

Each implements the `Command` trait. Load/Save/Delete delegate to the CRUD operations from PERSP-4. Filter/group commands update the perspective in-place.

**Register in `swissarmyhammer-kanban/src/commands/mod.rs`:**
- Add all 7 commands to `register_commands()` map
- Add `mod perspective_commands;`

**Update `swissarmyhammer-commands/src/registry.rs`:**
- Add `perspective.yaml` to `builtin_yaml_sources()`

## Acceptance Criteria
- [x] `perspective.yaml` loaded as builtin YAML source
- [x] All 7 commands registered in `register_commands()`
- [x] LoadPerspectiveCmd returns full perspective config
- [x] SavePerspectiveCmd creates new or updates existing perspective
- [x] DeletePerspectiveCmd removes perspective
- [x] Filter/group commands update perspective in-place
- [x] Commands appear in command palette (visible: true)

## Tests
- [x] `test_perspective_yaml_parses` — all 7 defs parse without error
- [x] `test_load_perspective_cmd` — execute returns perspective JSON
- [x] `test_save_perspective_cmd` — creates perspective via dispatch
- [x] `test_delete_perspective_cmd` — removes perspective
- [x] `test_filter_cmd` — sets filter on perspective
- [x] `test_clear_filter_cmd` — clears filter
- [x] Run: `cargo test -p swissarmyhammer-kanban commands::perspective`