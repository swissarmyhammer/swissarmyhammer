---
assignees:
- claude-code
depends_on:
- 01KS36N2YMN0VTSHXN8M555KSW
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffb080
project: command-service
title: Implement `list` + `schema command` verbs
---
## What

Implement the read-only verbs: `list command` (palette population) and `schema command` (return one command's input schema).

Files:
- `crates/swissarmyhammer-command-service/src/service.rs` — `list` and `schema` arms

`list command` returns active (top-of-stack) entries, with optional filters:
- `scope` — match against the entry's `scope` field (e.g., filter `scope: "entity:task"` returns only commands whose `scope` chain accepts `entity:task`)
- `category` — exact match
- `id_prefix` — startsWith match (so the palette can do incremental search server-side if it wants; the palette itself does its own fuzzy match)

Returned shape: `Vec<CommandMetadata>` — the existing public projection of `CommandRegistration` minus the callback markers (callers don't need those). Includes id, name, description, category, scope, keys, menu, context_menu, tab_button, undoable, visible, params. The palette/menu/hotkey systems all consume this. (Note: the task spec called this `CommandSummary` — the existing `CommandMetadata` type IS that projection; adding a duplicate would be unnecessary duplication.)

`schema command` returns one command's `params` array wrapped in a new `CommandSchema` struct for forward compat. Returns `UnknownCommand` if the id isn't registered.

## Acceptance Criteria
- [x] `list` with no filters returns one entry per active command, top-of-stack only (overridden entries hidden)
- [x] `list { scope: "entity:task" }` returns only commands whose `scope` field is empty (global) or contains `"entity:task"`
- [x] `list { category: "Cleanup" }` returns only commands with matching category
- [x] `list { id_prefix: "task." }` returns only commands whose id starts with `task.`
- [x] Filters compose (intersection): `list { scope: "entity:task", category: "Cleanup" }` returns commands matching both
- [x] `schema` for a registered command returns its `params` array
- [x] `schema` for an unregistered id returns `UnknownCommand` error
- [x] Neither verb invokes any callbacks (no isolate round-trips)

## Tests
- [x] `crates/swissarmyhammer-command-service/tests/list_filter.rs` — register 8 commands across two scopes and two categories; assert every filter combination returns the expected subset
- [x] `crates/swissarmyhammer-command-service/tests/list_override_hidden.rs` — A registers `foo`; B overrides `foo`; `list` returns one entry, and it's B's
- [x] `crates/swissarmyhammer-command-service/tests/schema_returns_params.rs` — register a command with `params: [{name:"task", from:"scope_chain", entity_type:"task"}]`; `schema` returns that exact array
- [x] `crates/swissarmyhammer-command-service/tests/schema_unknown.rs` — `schema { id: "does.not.exist" }` returns a structured error
- [x] `cargo test -p swissarmyhammer-command-service` passes

## Workflow
- Use `/tdd` — list/schema are pure reads; write the assertions first, register fixtures via the service's own `register` verb (not by poking the registry directly — that proves the full path works).

## Implementation Notes
- Added `CommandSchema { id, params }` to `src/types.rs` (re-exported from `lib.rs`) as the stable wrapper for `schema command` responses.
- `handle_list` returns `{ ok: true, commands: Vec<CommandMetadata> }` and applies the three filters as an intersection. Filter logic factored into `list_filter_matches` to keep the handler small.
- `handle_schema` returns `{ ok: true, schema: CommandSchema }` on success or maps to `CommandError::UnknownCommand` (`invalid_params` is mapped to `internal_error` per existing convention — see `command_error_to_mcp`).
- Scope-filter semantics: a command's `scope: None` or `scope: Some(vec![])` is "global" and matches every scope filter; otherwise the filter string must appear in the scope vec.