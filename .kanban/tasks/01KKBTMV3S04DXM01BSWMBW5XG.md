---
position_column: done
position_ordinal: cb80
title: 'SHELL-4: Wire config into ShellSecurityValidator, remove singleton'
---
## What

Replace the `OnceLock` singleton in `security.rs` with fresh config loading per validation call. This is the integration card that makes everything work together.

**Changes to `security.rs`**:
1. Remove `static GLOBAL_VALIDATOR: OnceLock<ShellSecurityValidator>` (line 426)
2. Remove `get_validator()` function (line 429-462)
3. Remove `load_security_policy()` function (line 465-492) — superseded by stacked config
4. Update `validate_command()` to call `load_shell_config()` → `compile()` → `evaluate_command()`
5. Update `validate_working_directory_security()` and `validate_environment_variables_security()` similarly
6. Update `ShellSecurityValidator` to accept `CompiledShellConfig` instead of `ShellSecurityPolicy`

**Changes to `lib.rs`**:
- Remove re-export of `get_validator`
- Add re-export of new config types
- Update `validate_command` signature if needed

**Changes to `swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs`**:
- Update `validate_shell_request()` to use the new API
- Remove any references to the global validator

**Migration path**: The public API (`validate_command`, `validate_working_directory_security`, `validate_environment_variables_security`) keeps the same signatures — they just load config internally now instead of using a singleton. This minimizes downstream breakage.

**Affected files**:
- `swissarmyhammer-shell/src/security.rs` (remove singleton, rewire validation)
- `swissarmyhammer-shell/src/lib.rs` (update re-exports)
- `swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs` (update caller)
- `swissarmyhammer-shell/src/hardening.rs` (update `HardenedSecurityValidator` to use new config)

## Acceptance Criteria
- [ ] No `OnceLock` or `static` validator in `security.rs`
- [ ] `validate_command()` loads fresh config on each call
- [ ] User can edit `~/.shell/config.yaml` and changes take effect on next command
- [ ] All existing security tests pass with new implementation
- [ ] MCP shell tool execution still validates commands correctly
- [ ] `ShellSecurityPolicy` struct either removed or deprecated in favor of `ShellSecurityConfig`

## Tests
- [ ] Existing test: `test_blocked_command_patterns` still passes
- [ ] Existing test: `test_shell_constructs_now_allowed` still passes
- [ ] Existing test: `test_command_length_validation` still passes
- [ ] Existing test: `test_directory_access_validation` still passes
- [ ] Existing test: `test_environment_variable_validation` still passes
- [ ] New test: modify config file between two `validate_command()` calls, second call sees new config
- [ ] `cargo test -p swissarmyhammer-shell` passes
- [ ] `cargo test -p swissarmyhammer-tools` passes