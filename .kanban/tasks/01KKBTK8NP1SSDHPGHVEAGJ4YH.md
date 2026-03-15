---
position_column: done
position_ordinal: b1
title: 'SHELL-1: Create ShellConfig DirectoryConfig and builtin/shell/config.yaml'
---
## What

Create the foundational config type and default YAML that replaces the hardcoded `ShellSecurityPolicy::default()`.

**New file**: `builtin/shell/config.yaml` — the builtin defaults, currently hardcoded in `security.rs:126-155`. YAML format with `permit` and `deny` sections containing regex patterns.

**New code in `swissarmyhammer-directory/src/config.rs`**: Add `ShellConfig` implementing `DirectoryConfig` with `DIR_NAME = ".shell"`. This enables `ManagedDirectory::<ShellConfig>::from_user_home()` (→ `~/.shell/`) and `from_git_root()` (→ `./.shell/`).

**Config YAML schema**:
```yaml
# Shell security configuration
deny:
  - pattern: 'rm\s+-rf\s+/'
    reason: "Destructive recursive delete from root"
  - pattern: 'sudo\s+'
    reason: "Privilege escalation"
  - pattern: 'sed\s+.*'
    reason: "Use edit tools instead"
  # ... all current defaults from security.rs

permit:
  # Patterns here override deny — evaluated first
  - pattern: 'sed --version'
    reason: "Allow version check"

settings:
  max_command_length: 4096
  max_env_value_length: 1024
  enable_audit_logging: true
```

**Affected files**:
- `builtin/shell/config.yaml` (new)
- `swissarmyhammer-directory/src/config.rs` (add ShellConfig)
- `swissarmyhammer-shell/Cargo.toml` (add serde_yaml dep if needed)

## Acceptance Criteria
- [ ] `builtin/shell/config.yaml` exists with all current default patterns from `security.rs:126-155`
- [ ] `ShellConfig` implements `DirectoryConfig` with DIR_NAME `.shell`
- [ ] Config YAML parses into a Rust struct with `deny`, `permit`, and `settings` sections
- [ ] Each pattern entry has `pattern` (regex string) and `reason` (human-readable explanation)
- [ ] Existing tests still pass — no behavior change yet

## Tests
- [ ] Unit test: `ShellConfig::DIR_NAME == ".shell"` in `swissarmyhammer-directory/src/config.rs`
- [ ] Unit test: parse `builtin/shell/config.yaml` into the config struct
- [ ] Unit test: all regex patterns in the YAML compile successfully
- [ ] `cargo test -p swissarmyhammer-directory` passes
- [ ] `cargo test -p swissarmyhammer-shell` passes