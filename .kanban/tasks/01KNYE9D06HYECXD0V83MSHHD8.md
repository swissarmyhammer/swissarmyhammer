---
assignees:
- claude-code
position_column: todo
position_ordinal: b280
title: Inject search paths into yaml_loader::load_lsp_servers
---
swissarmyhammer-lsp/src/yaml_loader.rs:64-74

The hardcoded rust-analyzer fallback branch in `load_lsp_servers()` is physically unreachable from tests: the function checks three hardcoded paths, the third of which uses `env!("CARGO_MANIFEST_DIR")` — a compile-time constant that always resolves to the real workspace `builtin/lsp` directory with 12 YAML files present. No runtime mechanism can redirect that path, so coverage of lines 64-74 is stuck at 0%.

**Approach:** split the current `load_lsp_servers()` into two functions:

```rust
/// Load LSP server specs from the given candidate paths, falling back
/// to a hardcoded rust-analyzer entry if no YAML configs are found.
pub(crate) fn load_lsp_servers_from(paths: &[&Path]) -> Vec<OwnedLspServerSpec> {
    // existing logic, but iterate `paths` instead of the hardcoded list
}

/// Public API — uses the current hardcoded path list.
pub fn load_lsp_servers() -> Vec<OwnedLspServerSpec> {
    let builtin = Path::new(env!("CARGO_MANIFEST_DIR")).join("builtin/lsp");
    let user = /* current ~/.config path */;
    let project = /* current project path */;
    load_lsp_servers_from(&[&builtin, &user, &project])
}
```

Then tests can call `load_lsp_servers_from(&[&tempdir])` with an empty dir to reliably exercise lines 64-74.

**Constraints to verify:**
- No observable public API change (only adds a `pub(crate)` helper).
- The 16 existing yaml_loader tests should still pass unchanged.
- The 2 shape-locking tests added in commit 9f35d2acc can be upgraded to exercise the actual fallback execution rather than just asserting the expected shape.

**Coverage impact:** unlocks deterministic coverage of yaml_loader.rs:64-74 (currently 0%).

#coverage-gap