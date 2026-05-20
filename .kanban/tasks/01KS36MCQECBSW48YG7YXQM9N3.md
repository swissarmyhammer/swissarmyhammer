---
assignees:
- claude-code
depends_on:
- 01KS36KNHH9BPC82MFMGTY3T5J
position_column: todo
position_ordinal: '8180'
project: command-service
title: Service skeleton + registry data structures (override stack)
---
## What

Build the in-process rmcp server for the `command` operation tool. No verb logic yet — just the skeleton, the registry data structures, and the unified dispatch that routes to per-verb handlers. Override-stack semantics live entirely in this layer.

Files:
- `crates/swissarmyhammer-command-service/src/service.rs` — `CommandService` struct (the `rmcp::ServerHandler`), holds the registry. Uses the operation-tool macro from `swissarmyhammer-operations-macros` to emit the `command` tool with auto-attached `_meta`.
- `crates/swissarmyhammer-command-service/src/registry.rs` — `CommandRegistry` with override-stack semantics:
  - `HashMap<String /* id */, Vec<StackEntry>>` where each `StackEntry` is `{ caller: CallerId, registration: CommandRegistration, registered_at: Instant }`
  - `push(caller, reg)` — if `(id, caller)` already exists, replace it in place (no duplicate per caller); else append
  - `pop_caller(caller, id)` — remove that caller's entry; falls back to next-most-recent
  - `active(id)` — top of stack (most recent)
  - `purge_caller(caller)` — drop every entry registered by this caller (used on plugin unload)
  - `list(filter)` — return only top-of-stack entries, filtered
- `crates/swissarmyhammer-command-service/src/notifications.rs` — debounced `notifications/commands/changed` emitter (100ms, flushed on `flush()` boundary which the platform calls on plugin load/unload)

Stack semantics (from command-service.md): re-registration by the same caller replaces that caller's entry rather than pushing a duplicate; on caller unload, all that caller's entries pop and the next-most-recent re-emerges.

Verb handlers in `service.rs` are stubs (`todo!()`) for now — they get filled in by subsequent tasks. Dispatch reads `arguments["op"]`, matches the verb string, deserializes the rest into the matching operation struct, calls the stub.

## Acceptance Criteria
- [ ] `CommandService` registers as an `rmcp::ServerHandler` and `tools/list` returns one tool named `command` with full `_meta` operations tree
- [ ] `CommandRegistry::push` enforces the per-caller dedupe rule (B pushes `foo` twice → still one B entry, on top)
- [ ] `CommandRegistry::pop_caller` followed by `active` returns the previous entry (the stack-fallback rule)
- [ ] `purge_caller` removes every entry from one caller in a single call
- [ ] Notifications are debounced to ~100ms and a `flush()` call drains pending notifications immediately

## Tests
- [ ] `crates/swissarmyhammer-command-service/tests/registry_stack.rs` — exercises push/pop_caller/active/purge directly. Includes the architecture-doc scenario: host registers `core.archive`; plugin A overrides; plugin B overrides; B unloads → A active; A unloads → host active.
- [ ] `crates/swissarmyhammer-command-service/tests/registry_dedupe.rs` — same caller registering the same id twice → one entry on the stack, not two
- [ ] `crates/swissarmyhammer-command-service/tests/notifications_debounce.rs` — 5 rapid register calls in one caller produce one notification; a `flush()` boundary forces immediate emission
- [ ] `crates/swissarmyhammer-command-service/tests/server_handler_smoke.rs` — `tools/list` on the rmcp service returns the `command` tool and its `_meta` operations tree has all six verbs under noun `command`
- [ ] `cargo test -p swissarmyhammer-command-service` passes

## Workflow
- Use `/tdd` — write the registry test first (it's pure logic, fast), then build to it. The rmcp wiring follows from a working registry.

Depends on the operation-struct task and on the `_meta` generator + operation-tool macro from the plugin-arch project.