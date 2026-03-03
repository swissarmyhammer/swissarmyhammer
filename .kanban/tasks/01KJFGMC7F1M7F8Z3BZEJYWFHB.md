---
title: Implement CLI integration via clap subcommands
position:
  column: done
  ordinal: a8V
---
Create src/cli.rs and wire into src/main.rs for dual-mode binary (GUI or CLI).

main.rs logic: inspect std::env::args() — if CLI subcommand present, dispatch via clap and exit. Otherwise launch Tauri GUI.

CLI subcommands (src/cli.rs):
- init <name> — initialize a board in cwd
- board — show board status (JSON or formatted)
- list [--column <col>] — list tasks
- gui — explicitly launch the GUI (default when no subcommand)

CLI commands use KanbanContext + KanbanOperationProcessor directly, same as Tauri commands but output to stdout.

lib.rs: run() fn that builds tauri::Builder with managed state and invoke_handler, called from main when in GUI mode.

Depends on: Tauri commands card.
Verify: binary runs with --help, init subcommand creates .kanban/, gui mode opens window.