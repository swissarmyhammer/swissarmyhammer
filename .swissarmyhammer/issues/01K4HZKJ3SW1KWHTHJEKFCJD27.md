# Complete swissarmyhammer-config Crate Independence

## Problem

The `swissarmyhammer-config` crate exists but `swissarmyhammer-tools` still imports config through the main crate:

- `swissarmyhammer::config::Config`

This suggests the config crate may not be complete or properly exposed.

## Solution

Ensure `swissarmyhammer-config` is a complete, standalone crate that provides all configuration functionality without depending on the main crate.

## Files Using Config

- `swissarmyhammer-tools/src/mcp/utils.rs`
- `swissarmyhammer-tools/src/mcp/tools/issues/show/mod.rs`
- `swissarmyhammer-tools/src/mcp/tools/issues/mark_complete/mod.rs`

## Tasks

1. Review current `swissarmyhammer-config` crate completeness
2. Ensure `Config` struct and all config functionality is available
3. Move any remaining config code from main crate if needed
4. Update `swissarmyhammer-tools` imports to use `swissarmyhammer_config::` directly
5. Remove config re-export from main crate

## Acceptance Criteria

- [ ] `swissarmyhammer-config` crate is fully independent
- [ ] `Config` and all config types available without main crate
- [ ] All imports updated to use `swissarmyhammer_config::` directly
- [ ] Configuration loading and parsing works independently
- [ ] All tests pass