---
position_column: done
position_ordinal: c980
title: 'SHELL-3: Permit/deny evaluation engine with permit-first semantics'
---
## What

Replace the current `check_blocked_patterns()` in `security.rs` with a new evaluation engine that supports both permit and deny patterns, with permit evaluated first.

**New function** in `swissarmyhammer-shell/src/config.rs` or `security.rs`:

```rust
pub fn evaluate_command(command: &str, config: &ShellSecurityConfig) -> Result<(), ShellSecurityError>
```

**Evaluation order**:
1. Check `permit` patterns first — if any permit pattern matches, the command is **allowed** immediately (short-circuit)
2. Check `deny` patterns — if any deny pattern matches, return `BlockedCommandPattern` error with the `reason` from the matching rule
3. If no deny pattern matches, the command is allowed (default-allow)

This gives users an escape hatch: if the builtin blocks `sed`, a project `.shell/config.yaml` can add `permit: [{pattern: 'sed\s+-i', reason: "Project uses sed for build scripts"}]` to unblock it for that project.

**Also**: compile all regex patterns once per config load (not per command). The `ShellSecurityConfig` should have a compiled form:

```rust
struct CompiledShellConfig {
    deny: Vec<CompiledRule>,   // (Regex, reason)
    permit: Vec<CompiledRule>, // (Regex, reason)
    settings: ShellSettings,
}
```

**Affected files**:
- `swissarmyhammer-shell/src/security.rs` (replace `check_blocked_patterns`, update `validate_command`)
- `swissarmyhammer-shell/src/config.rs` (add `CompiledShellConfig`, `evaluate_command`)

## Acceptance Criteria
- [ ] Permit patterns are evaluated before deny patterns
- [ ] A permit match short-circuits — command is allowed even if a deny pattern also matches
- [ ] Deny match returns error with the `reason` string from the YAML config
- [ ] Default (no match) is allow
- [ ] Regex patterns are compiled once when config is loaded, not per-command
- [ ] Invalid regex patterns in config produce a clear error at load time, not at validation time

## Tests
- [ ] Unit test: command matching a deny pattern is blocked
- [ ] Unit test: command matching both permit and deny is allowed (permit wins)
- [ ] Unit test: command matching neither is allowed
- [ ] Unit test: permit-only config allows everything
- [ ] Unit test: deny error includes the reason string from config
- [ ] Unit test: invalid regex in config produces compile-time error
- [ ] `cargo test -p swissarmyhammer-shell` passes