---
assignees:
- claude-code
depends_on:
- 01KNS10MMDVZG731XKM390C682
position_column: todo
position_ordinal: aa80
project: kanban-mcp
title: 'kanban-cli: create cli.rs with Serve/Init/Deinit/Doctor subcommands'
---
## What

Create `kanban-cli/src/cli.rs` defining a structured CLI with four new subcommands alongside the existing schema-driven operations.

Model exactly on `shelltool-cli/src/cli.rs`. The new `Commands` enum should include:

```rust
pub enum Commands {
    Serve,
    Init { target: InstallTarget },
    Deinit { target: InstallTarget },
    Doctor { verbose: bool },
    // ... existing open/merge handled separately in main.rs
}
```

Include `InstallTarget` enum (`Project`, `Local`, `User`) identical to shelltool-cli.

The top-level `Cli` struct uses `clap::Parser` with `name = "kanban"`, version, and about text.

Note: the existing schema-driven subcommands (`task add`, `column list`, etc.) and `open`/`merge` are handled by `main.rs`'s `allow_external_subcommands`. The new `cli.rs` only defines the lifecycle subcommands — `main.rs` dispatches to them before falling through to schema commands.

## Acceptance Criteria
- [ ] `kanban-cli/src/cli.rs` exists with `Cli`, `Commands`, `InstallTarget`
- [ ] `cargo check -p kanban-cli` passes
- [ ] `kanban serve --help` shows the serve subcommand
- [ ] `kanban init --help` and `kanban deinit --help` show target options

## Tests
- [ ] `kanban-cli/src/cli.rs` has unit tests verifying `Commands` variants exist and `InstallTarget` `Display` impl works
- [ ] Test file: `kanban-cli/src/cli.rs` in `#[cfg(test)]` module

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.
