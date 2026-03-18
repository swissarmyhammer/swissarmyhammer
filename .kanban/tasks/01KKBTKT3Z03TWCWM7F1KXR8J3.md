---
position_column: done
position_ordinal: c080
title: 'SHELL-2: Stacked config loader using VirtualFileSystem'
---
## What

Build the config loading mechanism that stacks `builtin/shell/config.yaml` → `~/.shell/config.yaml` → `./.shell/config.yaml` using the existing `VirtualFileSystem<ShellConfig>` pattern with `use_dot_directory_paths()`.

**New module**: `swissarmyhammer-shell/src/config.rs` — contains:

1. **`ShellSecurityConfig`** struct (deserializable from YAML):
   ```rust
   struct ShellSecurityConfig {
       deny: Vec<PatternRule>,
       permit: Vec<PatternRule>,
       settings: ShellSettings,
   }
   struct PatternRule {
       pattern: String,
       reason: String,
   }
   struct ShellSettings {
       max_command_length: usize,
       max_env_value_length: usize,
       enable_audit_logging: bool,
   }
   ```

2. **`load_shell_config()`** function:
   - Uses `VirtualFileSystem::<ShellConfig>::new("shell")` with `use_dot_directory_paths()`
   - Adds builtin config via `add_builtin()` (compile-time embedded from `builtin/shell/config.yaml`)
   - Calls `load_all()` — VFS handles precedence (builtin → user → project)
   - Parses each YAML source, then **merges** them: deny lists concatenate, permit lists concatenate, settings from later sources override earlier ones
   - Returns the merged `ShellSecurityConfig`

3. **Embed builtin at compile time**: Add to `swissarmyhammer-shell/build.rs` using `BuiltinGenerator` pattern from `swissarmyhammer-build`, or simply `include_str!()` since it's a single file.

**Key design**: No singleton. `load_shell_config()` is called fresh each time validation runs. The VFS reads from disk each time, so user edits to `~/.shell/config.yaml` or `./.shell/config.yaml` take effect immediately.

**Affected files**:
- `swissarmyhammer-shell/src/config.rs` (new)
- `swissarmyhammer-shell/src/lib.rs` (add `pub mod config`)
- `swissarmyhammer-shell/build.rs` (embed builtin YAML)

## Acceptance Criteria
- [ ] `load_shell_config()` returns merged config from all 3 layers
- [ ] Builtin config is compile-time embedded (no runtime file dependency)
- [ ] User config (`~/.shell/config.yaml`) adds/overrides patterns
- [ ] Project config (`./.shell/config.yaml`) adds/overrides patterns
- [ ] Missing user/project configs are silently skipped (builtin always present)
- [ ] Deny lists from all layers are concatenated (additive)
- [ ] Permit lists from all layers are concatenated (additive)
- [ ] Settings from later layers override earlier layers
- [ ] No global singleton — fresh load on each call

## Tests
- [ ] Unit test: load builtin-only config (no user/project files) returns default patterns
- [ ] Unit test: project config adds a deny pattern, merged result includes it
- [ ] Unit test: project config adds a permit pattern, merged result includes it
- [ ] Unit test: project settings override builtin settings
- [ ] Unit test: missing config files don't cause errors
- [ ] `cargo test -p swissarmyhammer-shell` passes