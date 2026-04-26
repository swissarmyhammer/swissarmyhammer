---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffa380
title: Add YAML persistence to UIState
---
## What

UIState currently lives only in memory. Add save/load to a YAML config file so it persists across app restarts.

### Config path
Use `~/.config/sah/kanban-app/config.yaml` (the existing path). UIState takes ownership of this file — AppConfig currently owns it but we'll migrate away from AppConfig incrementally.

### Approach
- Add `config_path: Option<PathBuf>` to UIState (set at construction, None for tests)
- Add `pub fn load(path: &Path) -> Self` — deserialize from YAML, fallback to defaults
- Add `pub fn save(&self) -> io::Result<()>` — serialize current state to YAML
- All mutation methods that return `UIStateChange` should auto-save after mutation (debounced or immediate — start with immediate, optimize later)
- The YAML schema should be forward-compatible: unknown fields ignored on load, not lost on save (use `serde(flatten)` or a separate approach)

### Files to modify
- `swissarmyhammer-commands/src/ui_state.rs` — add persistence, load/save, config_path
- `swissarmyhammer-commands/Cargo.toml` — may need `serde_yaml_ng` dependency

### What NOT to do
- Do NOT remove AppConfig yet — that's a later card
- Do NOT change the config file path yet — use the same path AppConfig uses
- Do NOT migrate any state from AppConfig into UIState yet — just add the persistence mechanism

## Acceptance Criteria
- [ ] `UIState::load(path)` reads YAML and populates state
- [ ] `UIState::save()` writes current state to YAML
- [ ] Mutations auto-persist
- [ ] Missing file → default state (no error)
- [ ] Existing tests still pass (they use `UIState::new()` with no path)

## Tests
- [ ] Round-trip: create UIState, mutate, save, load from same path, verify state matches
- [ ] Load from missing file returns defaults
- [ ] Load from malformed YAML returns defaults (not a crash)
- [ ] `cargo nextest run -p swissarmyhammer-commands` passes