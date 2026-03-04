---
position_column: todo
position_ordinal: c6
title: Remove `filter` and `group` from FieldDef
---
These properties belong to perspectives, not field definitions. They are stored but never used in runtime logic. Remove them to align with the spec.

## Affected files

- `swissarmyhammer-fields/src/types.rs` — Remove fields + serde annotations (lines 116-119). Remove from all FieldDef struct literals in 7 test functions.
- `swissarmyhammer-fields/src/context.rs` — Remove from 2 test helpers (lines 415-416, 732-733)
- `swissarmyhammer-fields/src/compute.rs` — Remove from 2 test helpers (lines 112-113, 128-129)
- `swissarmyhammer-fields/src/validation.rs` — Remove from 1 test helper (lines 221-222)
- `swissarmyhammer-kanban/src/defaults.rs` — Remove from 3 test struct literals (lines 348-349, 378-379, 428-429)

## Test YAML strings in types.rs

Remove `filter:` and `group:` lines from embedded YAML in:
- `built_in_status_field_serializes_correctly` (lines 484-485)
- `built_in_tags_computed_field` (line 518)
- `built_in_depends_on_reference_field` (line 595)

## Builtin YAML definitions

Remove `filter: substring` from:
- `swissarmyhammer-kanban/builtin/fields/definitions/tags.yaml` (line 9)
- `swissarmyhammer-kanban/builtin/fields/definitions/depends_on.yaml` (line 10)

## Checklist

- [ ] Remove `filter` and `group` fields from FieldDef struct
- [ ] Remove from all FieldDef struct literals in types.rs tests
- [ ] Remove from context.rs, compute.rs, validation.rs test helpers
- [ ] Remove from defaults.rs test struct literals
- [ ] Remove from embedded test YAML strings
- [ ] Remove from builtin YAML definition files
- [ ] Run `cargo test -p swissarmyhammer-fields -p swissarmyhammer-kanban`