---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffdc80
title: Add `log` and `turn_pre/` to AvpConfig::GITIGNORE_CONTENT
---
## What

`AvpConfig::GITIGNORE_CONTENT` in `swissarmyhammer-directory/src/config.rs` (line 110-123) is missing two entries that should be gitignored:

1. **`log`** — the tracing log file at `.avp/log` (see `avp-common/src/context.rs:9` — "Logging is handled by tracing ... writes to .avp/log"). The existing on-disk `.avp/.gitignore` was manually patched to include `log` but the source constant wasn't updated, so fresh `avp init` won't get it.
2. **`turn_pre/`** — the pre-hook sidecar directory (`.avp/turn_pre/` exists on disk but isn't gitignored). Same ephemeral per-turn data as `turn_diffs/`.

**File to modify:**
- `swissarmyhammer-directory/src/config.rs` — update `AvpConfig::GITIGNORE_CONTENT` (line ~110)

**Specific change:**
Add `log` after `*.log` and add `turn_pre/` section alongside the existing `turn_diffs/` entry:

```rust
const GITIGNORE_CONTENT: &'static str = r#"# AVP logs and state
# This file is automatically created by swissarmyhammer-directory

# Log files
*.log
log

# Turn state (ephemeral, per-session)
turn_state/

# Session-scoped sidecar diff files (ephemeral, per-turn)
turn_diffs/

# Session-scoped pre-hook sidecar files (ephemeral, per-turn)
turn_pre/

# Keep validators/ directory (should be committed)
"#;
```

Also update `test_avp_config` (line ~212) to assert the new entries.

## Acceptance Criteria
- [ ] `AvpConfig::GITIGNORE_CONTENT` includes `log` (bare filename, not just `*.log`)
- [ ] `AvpConfig::GITIGNORE_CONTENT` includes `turn_pre/`
- [ ] Existing test `test_avp_config` updated to assert `log` and `turn_pre/` are present
- [ ] All tests in `swissarmyhammer-directory` pass

## Tests
- [ ] Update `swissarmyhammer-directory/src/config.rs::test_avp_config` — add assertions for `log` and `turn_pre/`
- [ ] Run: `cargo test -p swissarmyhammer-directory` — all tests pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.