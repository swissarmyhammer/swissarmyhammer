---
assignees:
- claude-code
depends_on:
- 01KNFFHX8XQE2NX9GCJQCEJP76
position_column: done
position_ordinal: ffffffffffffffffffffffffffff8880
title: 'Flatten builtin/ directory: move fields/definitions → definitions/, fields/entities → entities/'
---
## What

The `swissarmyhammer-kanban/builtin/` directory has an asymmetrical layout:

```
builtin/
  actors/          ← top-level
  fields/
    definitions/   ← nested under fields/
    entities/      ← nested under fields/
  views/           ← top-level
```

`views` and `actors` are top-level peers, but `definitions` and `entities` are nested under `fields/`. The correct layout is four top-level siblings:

```
builtin/
  actors/
  definitions/
  entities/
  views/
```

This same asymmetry appears at runtime in `.kanban/` where `open()` creates `fields/definitions/` and `fields/entities/` subdirectories.

### Files to modify

1. **Move directories**:
   - `swissarmyhammer-kanban/builtin/fields/definitions/*.yaml` → `swissarmyhammer-kanban/builtin/definitions/*.yaml`
   - `swissarmyhammer-kanban/builtin/fields/entities/*.yaml` → `swissarmyhammer-kanban/builtin/entities/*.yaml`
   - Delete `swissarmyhammer-kanban/builtin/fields/` (now empty)

2. **`swissarmyhammer-kanban/src/defaults.rs`** — Update `include_dir!` paths:
   - `builtin/fields/definitions` → `builtin/definitions`
   - `builtin/fields/entities` → `builtin/entities`
   - Update doc comments referencing `builtin/fields/`

3. **`swissarmyhammer-kanban/src/context.rs`** — Update runtime on-disk layout:
   - `open()`: change `root.join("fields")` → create `root.join("definitions")` and `root.join("entities")` as peers
   - `build_entity_context()`: change `fields_root.join("definitions")` / `fields_root.join("entities")` → `root.join("definitions")` / `root.join("entities")`
   - `FieldsContext::from_yaml_sources(fields_root, ...)` — the `fields_root` param may need to change (verify what it's used for)
   - Update all tests that reference `fields/definitions` or `fields/entities`

## Acceptance Criteria

- [ ] `builtin/fields/` directory no longer exists
- [ ] `builtin/definitions/`, `builtin/entities/`, `builtin/views/`, `builtin/actors/` are all top-level siblings
- [ ] `.kanban/` runtime layout creates `definitions/` and `entities/` as top-level dirs (not under `fields/`)
- [ ] `cargo test -p swissarmyhammer-kanban` passes
- [ ] Existing boards with the old `fields/` layout are either migrated or gracefully handled

## Tests

- [ ] `swissarmyhammer-kanban/src/defaults.rs` — existing tests `builtin_field_definitions_load`, `builtin_entity_definitions_load` still pass with updated paths
- [ ] `swissarmyhammer-kanban/src/context.rs` — `test_open_builds_fields_context` asserts `definitions/` and `entities/` exist as top-level dirs
- [ ] `swissarmyhammer-kanban/src/context.rs` — `test_open_preserves_customizations` uses new path layout
- [ ] Run `cargo test -p swissarmyhammer-kanban` — all tests pass

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.