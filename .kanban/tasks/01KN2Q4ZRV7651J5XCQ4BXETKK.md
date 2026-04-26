---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffff8d80
title: 'PERSP-1: Perspective data types and YAML serialization'
---
## What

Create `swissarmyhammer-kanban/src/perspective/mod.rs` and `swissarmyhammer-kanban/src/perspective/types.rs` with:

- `Perspective` struct: `id` (String/ULID), `name`, `view`, `fields` (Vec<PerspectiveFieldEntry>), `filter` (Option<String>), `group` (Option<String>), `sort` (Vec<SortEntry>)
- `PerspectiveFieldEntry` struct: `field` (String/ULID), `caption` (Option<String>), `width` (Option<u32>), `editor` (Option<String>), `display` (Option<String>), `sort_comparator` (Option<String>)
- `SortDirection` enum: `Asc`, `Desc` — serializes as lowercase
- `SortEntry` struct: `field` (String/ULID), `direction` (SortDirection)

All types derive `Serialize, Deserialize, Debug, Clone`. Filter and group are opaque JS function strings — backend stores them, doesn't evaluate.

Follow the YAML format from the spec in `ideas/kanban/app-architecture.md` lines 769-790.

## Acceptance Criteria
- [ ] `Perspective` round-trips through serde_yaml_ng matching spec YAML format
- [ ] Minimal perspective (name + view only, empty fields/sort) round-trips
- [ ] Per-field overrides (caption, width, editor, display, sort_comparator) all optional and round-trip
- [ ] SortDirection serializes as "asc"/"desc" lowercase
- [ ] Filter/group stored as raw Option<String>

## Tests
- [ ] `perspective_yaml_round_trip` — full perspective with all fields populated
- [ ] `perspective_minimal_round_trip` — name + view only
- [ ] `field_entry_all_overrides` — PerspectiveFieldEntry with every override
- [ ] `field_entry_minimal` — field ULID only
- [ ] `sort_direction_serde` — asc/desc serialization
- [ ] `filter_group_as_strings` — JS function strings round-trip
- [ ] Run: `cargo test -p swissarmyhammer-kanban perspective::types`