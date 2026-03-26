---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffd80
title: 'warning: no test for CLI tools subcommand parsing (enable/disable flags)'
---
swissarmyhammer-cli/src/cli.rs (around line 532-543) and swissarmyhammer-cli/src/commands/tools/mod.rs\n\nThe `cli.rs` tests cover many subcommands (serve, init, deinit, doctor, validate, model, etc.) but there is no test for:\n- `sah tools` (no subcommand — list)\n- `sah tools enable shell git`\n- `sah tools disable kanban`\n- `sah tools --global enable shell`\n- `sah tools enable unknown_tool` (should reject)\n\nWithout these tests, a future refactor of the `ToolsSubcommand` enum or the `handle_tools_command` argument extraction in `main.rs` can silently break CLI parsing.\n\nSuggestion: Add tests using `Cli::try_parse_from_args` to verify:\n1. `tools` with no subcommand parses as `Commands::Tools { global: false, subcommand: None }`\n2. `tools enable shell` parses correctly with names vec\n3. `tools --global disable kanban` sets the `global` flag\n\nVerification: `cargo nextest run --package swissarmyhammer-cli` passes with new tests.\n\n#review-finding #review-finding