---
position_column: done
position_ordinal: cf80
title: Add impl_empty_initializable! macro and wire into McpTool
---
## What

Follow the `impl_empty_doctorable!` pattern: add a convenience macro for tools that don't need custom lifecycle, then add `Initializable` as a supertrait on `McpTool`.

The trait has default empty impls for `init()`, `deinit()`, `start()`, `stop()` — so the macro only needs to provide `name()` and `category()`. Tools opt into lifecycle methods by overriding defaults.

**Files:**
- EDIT: `swissarmyhammer-tools/src/mcp/tool_registry.rs` — add `Initializable` to `McpTool` supertrait list (`McpTool: Doctorable + Initializable + Send + Sync`), add `impl_empty_initializable!` macro
- EDIT: Every tool that currently uses `impl_empty_doctorable!` also needs `impl_empty_initializable!`: `ShellExecuteTool`, `JsTool`, `GitChangesTool`, `KanbanTool`, `CodeContextTool`, and any others
- EDIT: Tools with custom `Doctorable` impls (if any) also need `Initializable` impls
- EDIT: Test mock tools in `tool_registry.rs` tests

**The macro:**
```rust
macro_rules! impl_empty_initializable {
    ($tool_type:ty) => {
        impl swissarmyhammer_common::lifecycle::Initializable for $tool_type {
            fn name(&self) -> &str {
                <Self as $crate::mcp::tool_registry::McpTool>::name(self)
            }
            fn category(&self) -> &str { "tools" }
            // init, deinit, start, stop all have default empty impls on the trait
        }
    };
}
```

**Depends on:** Card 1 (Initializable trait)

## Acceptance Criteria
- [ ] `McpTool: Doctorable + Initializable + Send + Sync` compiles
- [ ] `impl_empty_initializable!` macro defined and exported
- [ ] All existing tools compile with the new supertrait
- [ ] Test mock tools updated

## Tests
- [ ] `cargo check -p swissarmyhammer-tools` compiles clean
- [ ] `cargo test -p swissarmyhammer-tools` passes
- [ ] `cargo check -p swissarmyhammer-cli` compiles clean