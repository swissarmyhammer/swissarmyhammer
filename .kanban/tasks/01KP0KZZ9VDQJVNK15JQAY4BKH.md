---
assignees:
- claude-code
depends_on:
- 01KNS10MMDVZG731XKM390C682
position_column: done
position_ordinal: ffffffffffffffffffffffffffffff9580
project: kanban-mcp
title: 'kanban-cli: create cli.rs with lifecycle subcommands for build.rs'
---
## What

Create `kanban-cli/src/cli.rs` — a minimal, self-contained CLI definition for the lifecycle subcommands only. This file exists primarily so `build.rs` can `#[path = "src/cli.rs"]`-include it for doc/manpage/completion generation, matching the `shelltool-cli` pattern exactly.

Model on `shelltool-cli/src/cli.rs`. Only `clap` + `std` dependencies (no `swissarmyhammer-*` imports) so it compiles in the build script context.

```rust
#[derive(Parser)]
#[command(name = "kanban", version, about = "...")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Serve,
    Init { target: InstallTarget },
    Deinit { target: InstallTarget },
    Doctor { verbose: bool },
}

#[derive(ValueEnum, Clone)]
pub enum InstallTarget { Project, Local, User }
```

The schema-driven noun/verb commands (`task add`, `board init`, etc.) are NOT in this file — they're built dynamically in `main.rs` via `cli_gen`. This file only defines the four lifecycle commands.

`main.rs` uses this file for the lifecycle subcommand definitions but still builds the full command tree dynamically (schema + lifecycle + open + merge) for runtime dispatch.

## Acceptance Criteria
- [x] `kanban-cli/src/cli.rs` exists with `Cli`, `Commands`, `InstallTarget`
- [x] Only depends on `clap` + `std` (build.rs-compatible)
- [x] `cargo check -p kanban-cli` passes

## Tests
- [x] Unit tests verifying `Commands` variants parse and `InstallTarget` `Display` works
- [x] Test file: `kanban-cli/src/cli.rs` in `#[cfg(test)]` module
