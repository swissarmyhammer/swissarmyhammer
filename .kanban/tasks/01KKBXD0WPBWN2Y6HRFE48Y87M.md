---
position_column: done
position_ordinal: f980
title: Rewire init.rs to use InitRegistry
---
## What

Replace the monolithic `install()` function in `init.rs` with an `InitRegistry`-based approach. This is the final wiring card — after all components and tool impls have been extracted.

The new `install()` becomes:
```rust
pub fn install(target: InstallTarget) -> Result<(), String> {
    let scope: InitScope = target.into();
    let mut registry = InitRegistry::new();
    
    // System components
    register_system_components(&mut registry);
    
    // Tool components (lightweight instances, no MCP server)
    register_tool_components(&mut registry);
    
    let results = registry.run_all_init(&scope);
    display_results(&results);
    
    check_for_errors(&results)
}
```

Similarly for `deinit.rs`.

**Files:**
- EDIT: `swissarmyhammer-cli/src/commands/install/init.rs` — full rewrite to registry pattern
- EDIT: `swissarmyhammer-cli/src/commands/install/deinit.rs` — full rewrite to registry pattern
- EDIT: `swissarmyhammer-cli/src/cli.rs` — add `From<InstallTarget> for InitScope`

## Acceptance Criteria
- [ ] `init.rs::install()` uses `InitRegistry` with zero hardcoded steps
- [ ] `deinit.rs::uninstall()` uses `InitRegistry::run_all_deinit()`
- [ ] All system + tool components registered
- [ ] Priority ordering respected
- [ ] `is_applicable()` filtering works per scope
- [ ] Results displayed to user with status messages
- [ ] Errors from any component halt or report clearly
- [ ] Behavior identical to original monolithic version

## Tests
- [ ] `cargo test -p swissarmyhammer-cli` passes
- [ ] Manual: `sah init` — identical output and side effects as before
- [ ] Manual: `sah deinit` — identical cleanup as before
- [ ] Manual: `sah init --target local` — only applicable components run
- [ ] Manual: `sah init --target user` — only applicable components run