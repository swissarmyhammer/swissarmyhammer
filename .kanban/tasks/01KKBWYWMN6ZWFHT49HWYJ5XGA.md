---
position_column: done
position_ordinal: c0
title: Add tool-specific Initializable impls (shell, code-context)
---
## What

Implement real `Initializable` for tools that have lifecycle needs. These replace the `impl_empty_initializable!` for those specific tools.

**Shell tool:**
- `init()`: Owns the "deny Bash" logic (currently hardcoded in `init.rs::install_deny_bash()`). Adds `Bash` to `.claude/settings.json` `permissions.deny`.
- `deinit()`: Removes `Bash` from `permissions.deny`.
- Priority: 25 (after project structure, before skills)
- No `start()`/`stop()` — shell has no background work.

**Code-context tool:**
- `init()`: Creates `.code-context/` directory, adds it to `.gitignore` if not present. Does NOT start LSP or indexing.
- `deinit()`: Removes `.code-context/` directory.
- `start()`: Runs background indexing, LSP supervisor, file watcher. This is the code currently in `initialize_code_context()` (which card A moved to `ServerHandler::initialize()`). This card replaces that hardcoded call with a proper `Initializable::start()` impl.
- `stop()`: Shuts down LSP supervisor, stops file watcher.
- Priority: 22 (after project structure)

**`init` and `start` are separate concerns:**
- `init` = project setup, runs during `sah init`, filesystem only
- `start` = runtime background work, runs during `sah serve` when the server explicitly iterates tools

**Files:**
- EDIT: `swissarmyhammer-tools/src/mcp/tools/shell/mod.rs` — replace `impl_empty_initializable!` with real impl (init/deinit only)
- EDIT: `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs` — replace `impl_empty_initializable!` with real impl (init/deinit/start/stop)
- EDIT: `swissarmyhammer-tools/src/mcp/server.rs` — replace hardcoded `initialize_code_context()` call in `ServerHandler::initialize()` with tool registry iteration calling `start()`
- EDIT: `swissarmyhammer-cli/src/commands/install/init.rs` — register tool initializers alongside system components

**Depends on:** Card 2 (macro/supertrait), Card A (constructor fix provides the intermediate state)

## Acceptance Criteria
- [ ] `ShellExecuteTool` implements `Initializable` with init/deinit (deny-bash)
- [ ] `CodeContextTool` implements `Initializable` with init/deinit/start/stop
- [ ] `install_deny_bash()` removed from monolithic init.rs
- [ ] Hardcoded `initialize_code_context()` in `ServerHandler::initialize()` replaced by tool `start()` iteration
- [ ] `sah init` creates `.code-context/`, denies Bash — no LSP, no indexing
- [ ] `sah serve` + client connect — code-context `start()` fires, LSP/indexing begin
- [ ] `sah deinit` cleans up both

## Tests
- [ ] `cargo test -p swissarmyhammer-tools` passes
- [ ] `cargo test -p swissarmyhammer-cli` passes
- [ ] Manual: `sah init` — creates `.code-context/`, denies Bash, no background work
- [ ] Manual: `sah serve` + connect — background indexing/LSP start
- [ ] Manual: `sah deinit` — removes `.code-context/`, restores Bash permission