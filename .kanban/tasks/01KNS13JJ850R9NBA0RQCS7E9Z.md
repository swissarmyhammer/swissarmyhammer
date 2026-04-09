---
assignees:
- claude-code
depends_on:
- 01KNS1154TG90CFZCHCPK5PMNS
- 01KNS11SKBN6FCG2WFDSPC2AVK
- 01KNS12D5B11TDND9QABCY5HAC
- 01KNS12X336WM3ETPW0F2V3G07
- 01KNS903T77DCWAH339AD550K5
position_column: todo
position_ordinal: ae80
project: kanban-mcp
title: 'kanban-cli: wire main.rs — route lifecycle commands to handlers'
---
## What

Refactor `kanban-cli/src/main.rs` to route lifecycle commands while preserving all existing behavior.

Key changes:
1. Add `mod cli; mod serve; mod registry; mod doctor;` declarations
2. Parse lifecycle commands from `cli::Commands` BEFORE the schema-driven clap tree — use a pre-parse check on `std::env::args()` to intercept `serve`/`init`/`deinit`/`doctor` before building the full schema command tree
3. Dispatch `Commands::Serve` → `serve::run_serve().await`
4. Dispatch `Commands::Init { target }` → `registry::register_all` + `InitRegistry::run_all_init`
5. Dispatch `Commands::Deinit { target }` → `registry::register_all` + `InitRegistry::run_all_deinit`
6. Dispatch `Commands::Doctor { verbose }` → `doctor::run_doctor(verbose)`

The existing `open`, `merge`, and schema-driven noun-verb commands must continue to work unchanged.

## Acceptance Criteria
- [ ] `kanban serve` exits cleanly when stdin closes (MCP EOF)
- [ ] `kanban init` prints registration results and exits 0
- [ ] `kanban deinit` prints removal results and exits 0
- [ ] `kanban doctor` prints table and exits 0/1/2
- [ ] `kanban task list` (existing schema command) still works
- [ ] `kanban open .` (existing open command) still works

## Tests
- [ ] Integration test in `kanban-cli/tests/cli.rs`: `kanban doctor` exits 0 or 1
- [ ] Integration test: `kanban --help` lists all four new subcommands

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.
