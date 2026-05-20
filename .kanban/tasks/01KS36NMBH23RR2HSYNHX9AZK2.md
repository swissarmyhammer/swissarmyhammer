---
assignees:
- claude-code
depends_on:
- 01KS36N2YMN0VTSHXN8M555KSW
position_column: todo
position_ordinal: '8380'
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

Returned shape: `Vec<CommandSummary>` — a public projection of `CommandRegistration` minus the callback markers (callers don't need those). Includes id, name, description, category, scope, keys, menu, context_menu, tab_button, undoable, visible, params. The palette/menu/hotkey systems all consume this.

`schema command` returns one command's `params` array (and a stable `CommandSchema` wrapper for forward compat). Returns `UnknownCommand` if the id isn't registered.

## Acceptance Criteria
- [ ] `list` with no filters returns one entry per active command, top-of-stack only (overridden entries hidden)
- [ ] `list { scope: "entity:task" }` returns only commands whose `scope` field is empty (global) or contains `"entity:task"`
- [ ] `list { category: "Cleanup" }` returns only commands with matching category
- [ ] `list { id_prefix: "task." }` returns only commands whose id starts with `task.`
- [ ] Filters compose (intersection): `list { scope: "entity:task", category: "Cleanup" }` returns commands matching both
- [ ] `schema` for a registered command returns its `params` array
- [ ] `schema` for an unregistered id returns `UnknownCommand` error
- [ ] Neither verb invokes any callbacks (no isolate round-trips)

## Tests
- [ ] `crates/swissarmyhammer-command-service/tests/list_filter.rs` — register 8 commands across two scopes and two categories; assert every filter combination returns the expected subset
- [ ] `crates/swissarmyhammer-command-service/tests/list_override_hidden.rs` — A registers `foo`; B overrides `foo`; `list` returns one entry, and it's B's
- [ ] `crates/swissarmyhammer-command-service/tests/schema_returns_params.rs` — register a command with `params: [{name:"task", from:"scope_chain", entity_type:"task"}]`; `schema` returns that exact array
- [ ] `crates/swissarmyhammer-command-service/tests/schema_unknown.rs` — `schema { id: "does.not.exist" }` returns a structured error
- [ ] `cargo test -p swissarmyhammer-command-service` passes

## Workflow
- Use `/tdd` — list/schema are pure reads; write the assertions first, register fixtures via the service's own `register` verb (not by poking the registry directly — that proves the full path works).