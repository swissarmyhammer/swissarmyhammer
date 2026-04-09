---
assignees:
- claude-code
depends_on:
- 01KNS1154TG90CFZCHCPK5PMNS
- 01KNS11SKBN6FCG2WFDSPC2AVK
- 01KNS12D5B11TDND9QABCY5HAC
- 01KNS12X336WM3ETPW0F2V3G07
position_column: todo
position_ordinal: ae80
project: kanban-mcp
title: 'kanban-cli: wire main.rs — dispatch Serve/Init/Deinit/Doctor with file logging'
---
## What

Refactor `kanban-cli/src/main.rs` to integrate the new lifecycle subcommands while preserving all existing behavior.

Key changes:
1. Add `mod cli; mod serve; mod registry; mod doctor;` declarations
2. Parse lifecycle commands from `cli::Commands` BEFORE the schema-driven clap tree — use a pre-parse check on `std::env::args()` to intercept `serve`/`init`/`deinit`/`doctor` before building the full schema command tree (same early-arg-check pattern the banner uses now, or restructure to parse `Cli` first and fall through to schema commands)
3. Set up file-based tracing to `.kanban/log`, falling back to stderr — use the `FileWriterGuard` pattern from `shelltool-cli/src/main.rs`. Log dir is `".kanban"`, log file is `log` (no extension): `log_dir.join("log")`. Add `.kanban/.gitignore` with a `log` entry to keep the file out of git.
4. Dispatch `Commands::Serve` → `serve::run_serve().await`
5. Dispatch `Commands::Init { target }` → `registry::register_all` + `InitRegistry::run_all_init`
6. Dispatch `Commands::Deinit { target }` → `registry::register_all` + `InitRegistry::run_all_deinit`
7. Dispatch `Commands::Doctor { verbose }` → `doctor::run_doctor(verbose)`
8. Create `kanban-cli/.mcp.json` registering `kanban serve` for local dev use (mirrors `shelltool-cli/.mcp.json`):
   ```json
   {
     "mcpServers": {
       "kanban": {
         "command": "kanban",
         "args": ["serve"]
       }
     }
   }
   ```

The existing `open`, `merge`, and schema-driven noun-verb commands must continue to work unchanged.

## Acceptance Criteria
- [ ] `kanban serve` exits cleanly when stdin closes (MCP EOF)
- [ ] `kanban init` prints registration results and exits 0
- [ ] `kanban deinit` prints removal results and exits 0
- [ ] `kanban doctor` prints table and exits 0/1/2
- [ ] `kanban task list` (existing schema command) still works
- [ ] `kanban open .` (existing open command) still works
- [ ] `kanban-cli/.mcp.json` exists with `kanban serve` registered
- [ ] `.kanban/.gitignore` contains `log` to exclude the runtime log file

## Tests
- [ ] Integration test in `kanban-cli/tests/cli.rs`: `kanban doctor` exits 0 or 1 (not 2 in CI)
- [ ] Integration test: `kanban --help` lists all four new subcommands

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.
