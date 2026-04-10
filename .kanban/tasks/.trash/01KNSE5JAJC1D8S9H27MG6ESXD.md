---
assignees:
- claude-code
position_column: todo
position_ordinal: '8e80'
project: code-context-cli
title: Add `code-context lsp install` command for easy LSP server installation
---
## What

Add an `Install` variant to the existing `LspCommands` enum in `code-context-cli/src/cli.rs` and implement the install logic so users can run `code-context lsp install` to automatically install all missing LSP servers for their detected project types.

### Current state

- `code-context lsp status` already exists — it calls the `lsp status` MCP op which uses `swissarmyhammer_lsp::registry::all_servers()` and `swissarmyhammer_tools::mcp::tools::code_context::doctor::run_doctor()` to report installed/missing LSP servers.
- Each `OwnedLspServerSpec` in `swissarmyhammer-lsp/src/types.rs` carries an `install_hint` field (e.g. `\"Install rust-analyzer: rustup component add rust-analyzer\"`, `\"Install typescript-language-server: npm install -g typescript-language-server typescript\"`).
- The existing `builtin/skills/lsp/SKILL.md` tells Claude Code agents to run `lsp status`, present results, ask permission, then run install commands via `shell`. But there's no CLI command for users to do this directly.

### Files to modify

1. **`code-context-cli/src/cli.rs`** — Add `Install` variant to `LspCommands`:
   ```rust
   /// Install missing LSP servers for detected project types
   Install {
       /// Install a specific server by name (e.g. \"rust-analyzer\")
       #[arg(long)]
       server: Option<String>,
       /// Skip confirmation prompt and install immediately
       #[arg(long, short = 'y')]
       yes: bool,
   }
   ```

2. **`code-context-cli/src/doctor.rs`** — Add `pub fn install_lsp_servers(server_filter: Option<&str>, auto_confirm: bool) -> i32` that:
   - Calls `run_doctor(cwd)` to get `DoctorReport`
   - Filters `lsp_servers` to only those with `installed == false`
   - If `server_filter` is `Some`, further filter to matching server name
   - If no missing servers, print message and return 0
   - If `auto_confirm` is false, print the install commands and prompt for confirmation (read stdin)
   - For each missing server, parse `install_hint` to extract the shell command (the part after the colon in e.g. `\"Install rust-analyzer: rustup component add rust-analyzer\"`)
   - Run each install command via `std::process::Command::new(\"sh\").args([\"-c\", &cmd])`
   - Report success/failure for each
   - Re-run `run_doctor(cwd)` and print updated status
   - Return 0 if all installs succeeded, 1 if any failed

3. **`code-context-cli/src/main.rs`** — Wire `Commands::Lsp { command: LspCommands::Install { server, yes } }` to call `doctor::install_lsp_servers(server.as_deref(), yes)`

4. **`code-context-cli/src/ops.rs`** — No change needed (lsp install is not an MCP op, it's a CLI-only command)

### Install hint parsing

The `install_hint` field follows the pattern `\"Install <name>: <command>\"`. Extract the command after `\": \"`. If the format doesn't match, fall back to printing the raw hint and asking the user to run it manually.

## Acceptance Criteria

- [ ] `code-context lsp install --help` shows the install subcommand with `--server` and `--yes` flags
- [ ] `code-context lsp install` detects missing LSP servers, prompts for confirmation, and runs install commands
- [ ] `code-context lsp install --server rust-analyzer` installs only the named server
- [ ] `code-context lsp install --yes` skips the confirmation prompt
- [ ] If all servers are already installed, prints a message and exits 0
- [ ] After installation, re-runs status check and prints updated table
- [ ] Failed installs report the error and exit 1

## Tests

- [ ] `test_lsp_install_parses` — `Cli::try_parse_from([\"code-context\", \"lsp\", \"install\"])` succeeds in `code-context-cli/src/cli.rs`
- [ ] `test_lsp_install_with_server_flag` — `--server rust-analyzer` parses correctly
- [ ] `test_lsp_install_with_yes_flag` — `--yes` / `-y` parses correctly
- [ ] `test_parse_install_hint` — unit test for extracting command from `\"Install rust-analyzer: rustup component add rust-analyzer\"` → `\"rustup component add rust-analyzer\"`
- [ ] `test_parse_install_hint_no_colon` — fallback when hint doesn't contain `\": \"`
- [ ] Run `cargo test -p code-context-cli` — all pass

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.