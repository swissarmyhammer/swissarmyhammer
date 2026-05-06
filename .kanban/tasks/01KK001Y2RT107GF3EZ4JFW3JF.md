---
position_column: done
position_ordinal: ffff9c80
title: Unified dispatch_command Tauri endpoint with scope chain and undoable flag
---
Replace the current `execute_command` Tauri command with the new `dispatch_command` that uses `CommandInvocation` and the `CommandsRegistry`.

## Scope

- New Tauri command `dispatch_command(cmd: String, scope_chain: Option<Vec<String>>, target: Option<String>, args: Option<Value>)`
  - If `scope_chain` not provided, use the stored scope chain from `set_focus`
  - Builds `CommandContext` with all services from `AppState`
  - Looks up command in `CommandsRegistry`
  - Runs static scope pre-filter
  - Calls `command.available(&ctx)` — returns error if false
  - If `command_def.undoable`: generate transaction ULID, set on EntityContext, execute, log, clear
  - If not undoable: execute without transaction
  - Returns `{ operation_id?, result }`
- New Tauri command `list_available_commands(context_menu: Option<bool>)` — returns available commands for current scope chain, optionally filtered to context_menu commands
- New Tauri command `set_focus(scope_chain: Vec<String>)` — stores current scope chain in AppState
- Remove old `execute_command` Tauri command entirely
- Remove all individual Tauri commands that were bypassing the dispatcher (if any remain)

## Testing

- Test: `dispatch_command` with valid scope chain resolves and executes
- Test: `dispatch_command` with missing scope chain uses stored focus
- Test: `dispatch_command` returns error when command not available
- Test: `dispatch_command` with `undoable: true` command generates operation_id
- Test: `dispatch_command` with `undoable: false` command returns no operation_id
- Test: `list_available_commands` returns only scope-matching, available commands
- Test: `list_available_commands(context_menu: true)` filters to context menu commands
- Test: `set_focus` stores scope chain, subsequent dispatch uses it