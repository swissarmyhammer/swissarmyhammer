---
depends_on:
- 01KKC8CZA1Z3288KXDYNYRZR1C
position_column: done
position_ordinal: ffffffffffb780
title: 'STATUSLINE-7: Init/deinit hooks'
---
## What
Hook statusline installation into `sah init` and removal into `sah deinit`. When init runs, it adds the statusLine config to Claude Code settings. When deinit runs, it removes it.

Key files:
- `swissarmyhammer-cli/src/commands/install/settings.rs` — add `merge_statusline()` and `remove_statusline()` helpers
- `swissarmyhammer-cli/src/commands/install/init.rs` — call `install_statusline()` after other init steps
- `swissarmyhammer-cli/src/commands/install/deinit.rs` — call `uninstall_statusline()` during deinit

### settings.rs additions
```rust
pub fn merge_statusline(settings: &mut Value) -> bool {
    // Merge {\"statusLine\": {\"type\": \"command\", \"command\": \"sah statusline\"}}
    // Returns true if changed
}

pub fn remove_statusline(settings: &mut Value) -> bool {
    // Remove the \"statusLine\" key entirely
    // Returns true if changed
}
```

### init.rs addition
After `install_deny_bash()`, call:
```rust
install_statusline()?;
```
Which reads `.claude/settings.json` via `settings::read_settings`, calls `merge_statusline`, writes back.

### deinit.rs addition
After `uninstall_deny_bash()`, call:
```rust
uninstall_statusline()?;
```
Which reads, calls `remove_statusline`, writes back.

Both should be idempotent and handle missing files gracefully.

## Acceptance Criteria
- [ ] `sah init` adds `statusLine` to `.claude/settings.json`
- [ ] `sah init` is idempotent (running twice doesn't duplicate)
- [ ] `sah deinit` removes `statusLine` from `.claude/settings.json`
- [ ] `sah deinit` is idempotent (running on already-clean settings is no-op)
- [ ] Other settings in `.claude/settings.json` are preserved

## Tests
- [ ] Unit test: merge_statusline into empty settings
- [ ] Unit test: merge_statusline idempotent (already present)
- [ ] Unit test: merge_statusline preserves other keys
- [ ] Unit test: remove_statusline when present
- [ ] Unit test: remove_statusline when absent (no-op)
- [ ] Unit test: roundtrip merge then remove
- [ ] `cargo test -p swissarmyhammer-cli`