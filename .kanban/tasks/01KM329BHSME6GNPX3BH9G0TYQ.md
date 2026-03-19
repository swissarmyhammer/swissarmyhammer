---
assignees:
- claude-code
position_column: todo
position_ordinal: '80'
title: Switch AppConfig persistence from JSON to YAML
---
## What
Change `AppConfig` load/save in `kanban-app/src/state.rs` from `serde_json` to `serde_yaml_ng`. Rename `config.json` → `config.yaml`. Handle migration: if `config.yaml` doesn't exist but `config.json` does, read the JSON and write it back as YAML.

**Files:**
- `kanban-app/src/state.rs` — `AppConfig::load()`, `AppConfig::save()`, `CONFIG_FILE_NAME` constant, `config_file_path()` docstring
- `kanban-app/src/state.rs` tests — update serialization roundtrip tests (lines ~1210-1460) to use `serde_yaml_ng` instead of `serde_json`

**Note:** `serde_yaml_ng` is already a workspace dep in `kanban-app/Cargo.toml`.

## Acceptance Criteria
- [ ] Config saves as `config.yaml` in YAML format
- [ ] On first launch after upgrade, existing `config.json` is migrated to `config.yaml`
- [ ] All existing config fields survive the migration losslessly
- [ ] `config.json` is NOT deleted (leave it for safety, just stop reading it once yaml exists)

## Tests
- [ ] Update `test_window_boards_persists_through_serialization` and siblings to use serde_yaml_ng
- [ ] Add `test_config_json_migration` — write a JSON config, verify `load()` reads it and produces valid AppConfig
- [ ] `cargo nextest run -p kanban-app` passes