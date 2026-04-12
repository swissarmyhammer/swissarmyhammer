---
assignees:
- claude-code
depends_on:
- 01KNS11SKBN6FCG2WFDSPC2AVK
- 01KNS12D5B11TDND9QABCY5HAC
- 01KNS12X336WM3ETPW0F2V3G07
- 01KNS903T77DCWAH339AD550K5
- 01KP0KZZ9VDQJVNK15JQAY4BKH
position_column: todo
position_ordinal: ae80
project: kanban-mcp
title: 'kanban-cli: wire serve/init/deinit/doctor into existing command tree'
---
## What

Add the four lifecycle commands to the **existing** command tree in `kanban-cli/src/main.rs`, alongside the schema-driven noun/verb subcommands, `open`, and `merge`. Same pattern — just more hardcoded `clap::Command` entries in the builder.

Key changes to `main.rs`:
1. Add `mod serve; mod registry; mod doctor;` declarations
2. Add four new `clap::Command` entries to the existing builder — `serve`, `init`, `deinit`, `doctor` — right next to where `open` and `merge` are added today
3. In the existing `match matches.subcommand()`, add arms for the four new subcommands:
   - `"serve"` → `serve::run_serve().await`
   - `"init"` → build `InitRegistry` via `registry::register_all`, run init with scope from target arg
   - `"deinit"` → same registry, run deinit
   - `"doctor"` → `doctor::run_doctor(verbose)`

The `init`/`deinit` subcommands take an optional positional `[TARGET]` with values `project` (default), `local`, `user` — model this with `clap::Arg::new("target").value_parser(["project", "local", "user"]).default_value("project")`.

The `doctor` subcommand takes `--verbose` / `-v`.

The existing `open`, `merge`, and schema-driven noun-verb commands must continue to work unchanged.

## Acceptance Criteria
- [ ] `kanban serve` exits cleanly when stdin closes (MCP EOF)
- [ ] `kanban init` prints registration results and exits 0
- [ ] `kanban deinit` prints removal results and exits 0
- [ ] `kanban doctor` prints table and exits 0/1/2
- [ ] `kanban task list` (existing schema command) still works
- [ ] `kanban open .` (existing open command) still works
- [ ] `kanban --help` lists serve, init, deinit, doctor alongside the schema commands

## Tests
- [ ] `cargo test -p kanban-cli` passes (all existing + new tests)
- [ ] Integration test: `kanban doctor` exits 0 or 1
- [ ] Integration test: `kanban --help` lists all four new subcommands
