# Plan 2 — Command Service (engine)

**Kanban project:** `command-service` · **Tier 0** · **Depends on:** the merged
`operation_tool!` macro (plugin-arch); `store-service` for the txn mechanism;
`command-events` for action-event emission (soft — execute can land first and
wire the emit when the notification surface exists).

The Command MCP **engine** — not the specific commands. A single `command`
operation tool with verbs register/list/execute/available/unregister/schema,
override-stack semantics, and callback marshalling across the plugin boundary.

## Tasks

| Kanban id | Title | depends_on | Acceptance (one-liner) |
| --------- | ----- | ---------- | ---------------------- |
| `01KS36KNHH9BPC82MFMGTY3T5J` | Design operation structs + registration payload schema | — | New `swissarmyhammer-command-service` crate; 6 `#[operation]` structs; `RegisterCommand` payload mirrors every YAML field (keys/menu/scope/params/undoable/…); payload round-trips all 12 YAMLs without loss. |
| `01KS36MCQECBSW48YG7YXQM9N3` | Service skeleton + registry data structures (override stack) | op-structs | `CommandService` rmcp handler; `CommandRegistry` with per-caller dedupe + stack fallback + `purge_caller`; debounced `commands/changed`. |
| `01KS36N2YMN0VTSHXN8M555KSW` | register + unregister verbs (callback markers) | skeleton | `register` stores `available`/`execute` callback markers + CallerId; `unregister` pops that caller's entry; caller isolation. |
| `01KS36NMBH23RR2HSYNHX9AZK2` | list + schema verbs | register/unregister | `list` returns top-of-stack, filterable by scope/category/id_prefix; `schema` returns a command's params; overridden entries hidden. |
| `01KS36P9C8CFT5HMQWY2WCA9ZE` | execute + available verbs (callback round-trip + latency budget) | list/schema | execute rechecks available (unless force) and runs the callback; available enforces 5ms/50ms budget. NO txn/emit here (Tier-0-clean, no store/notification dep) — those are the follow-up. |
| `01KS613VPH2G4ZWKZPGW9ZCJAA` | execute transaction bracketing + `commands/executed` action event | execute/available, `store-service`, notification surface (`01KS5G3AKZ`) | wraps execute: opens/closes a `txn` (one undo group), emits `commands/executed {id,ctx,result,txn,origin}` correlated to the `store/changed` events; write-nothing commands still emit; errors still close the txn. |
| `01KS36PZK9K6PHTRB9M7YPWTF2` | Host bootstrap + ledger-driven auto-cleanup | execute/available | `commands` exposed in-process at bootstrap; plugin unload purges its registrations; override re-emergence after unload. |
| `01KS36QGEVVP064EKW0JDGD94B` | SDK helpers: `ensureServices` + `registerCommands` | bootstrap; plugin-arch idempotency `01KS371KNY4YARZ67KWVSXPDFP` | `ensureServices(this, [...])` idempotently registers needed servers; `registerCommands(this, [...])` batches; path sugar `this.commands.command.command.register()` works. |

## Key decisions baked in

- One operation tool, `op`-dispatched; `_meta` tree from `operation_tool!`.
- **Override stack**: most-recent registration wins; per-caller dedupe;
  unload re-emerges the prior. (The headline command-service.md scenario.)
- **`execute` is the command-as-unit boundary**: opens a `txn`, runs the
  callback, closes it, emits `commands/executed`. Empty-write commands yield an
  empty (free) undo group and still emit the action event. **Split across two
  tasks**: the verb + callback round-trip lands first (Tier-0-clean); the
  txn-bracket + `commands/executed` emit is a follow-up (`01KS613VPH2G4ZWKZPGW9ZCJAA`)
  that hard-depends on `store-service` + the notification surface.
- SDK convention: a plugin's `load()` does `ensureServices(...)` then
  `registerCommands(...)`. Relies on plugin-arch idempotent registration.

## Cross-check

`kanban list tasks --filter '$command-service'` → expect exactly these 8 tasks.
