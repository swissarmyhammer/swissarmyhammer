---
assignees:
- claude-code
depends_on:
- 01KM0DBV4WE1N98MN80MTNZQPB
position_column: done
position_ordinal: ffffffff8f80
title: Flatten execute/ subfolder into shell/
---
Remove the `execute/` nesting layer. Move `execute/mod.rs` content into `shell/mod.rs` and place `description.md` directly in `shell/`. Follow the `code_context` tool pattern where everything is flat under the tool module.

Current structure:
```
shell/
├── mod.rs              # thin wrapper, register_shell_tools()
├── state.rs
└── execute/
    ├── mod.rs          # 4600 lines - all the real code
    └── description.md
```

Target structure:
```
shell/
├── mod.rs              # merged: register + ShellExecuteTool + dispatcher + operations
├── state.rs
└── description.md      # moved up from execute/
```

This must happen BEFORE the per-operation extraction (Card 1) so that subsequent cards extract into `shell/list_processes/mod.rs` not `shell/execute/list_processes/mod.rs`.

**Steps:**
1. Move `execute/description.md` to `shell/description.md`
2. Merge `execute/mod.rs` content into `shell/mod.rs` (combine the thin wrapper with the real implementation)
3. Update `description()` to use `include_str!("description.md")` instead of `get_tool_description("shell", "execute")`
4. Delete `execute/` directory
5. Update any imports referencing `shell::execute::`

**Verify**: `cargo nextest run -p swissarmyhammer-tools`