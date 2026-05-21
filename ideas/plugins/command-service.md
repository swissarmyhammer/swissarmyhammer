# Command Service

The Command service is an MCP server that ships with the platform. It is built on top of the [plugin architecture](./plugin-architecture.md) — the platform itself has no concept of "the command service"; it is a registered server like any other.

The service exists for two reasons: it is needed by the host UI (command palette, hotkey dispatch, menus), and it doubles as a worked example of building a non-trivial service on the platform. Every pattern it uses — an operation tool with `op` dispatch, callbacks across the host/plugin boundary, the `_meta` operations tree, override stacks — is available to any other service.

## What the service does

Commands lets clients (plugins, the user via UI, external agents) register named, user-invocable actions with two-phase semantics:

- `available(ctx)` — synchronous precondition check
- `execute(ctx)` — async action

The command palette, hotkey dispatch, menu system, and agent integrations all invoke commands through this server. Override semantics let plugins replace built-ins.

## MCP surface

The Command service exposes a single **operation tool**, `command`. A
plugin (or the host) registers the server under the name `commands`, so
it is reached as `this.commands`. The tool bundles one noun — `command`
— with the verbs register, list, execute, available, unregister, and
schema.

Like every operation tool, it describes itself in the tool's `_meta`
under `io.swissarmyhammer/operations` — the noun → verb → parameters
tree (see *Service Consumption* and *Operation tools and
`swissarmyhammer-operations`* in the
[plugin architecture](./plugin-architecture.md)):

```jsonc
{
  "name": "command",
  "_meta": {
    "io.swissarmyhammer/operations": {
      "command": {
        "register":   { "op": "register command",
                        "description": "Add a command to the registry",
                        "parameters": { "id":   { "type": "string", "required": true },
                                        "name": { "type": "string", "required": true } /* … */ } },
        "list":       { "op": "list command",
                        "description": "Enumerate active commands",
                        "parameters": { /* optional filters */ } },
        "execute":    { "op": "execute command",
                        "description": "Run a command",
                        "parameters": { "id":    { "type": "string",  "required": true },
                                        "ctx":   { "type": "object",  "required": true },
                                        "force": { "type": "boolean", "required": false } } },
        "available":  { "op": "available command",
                        "description": "Check whether a command can run",
                        "parameters": { "id":  { "type": "string", "required": true },
                                        "ctx": { "type": "object", "required": true } } },
        "unregister": { "op": "unregister command",
                        "description": "Remove a command",
                        "parameters": { "id": { "type": "string", "required": true } } },
        "schema":     { "op": "schema command",
                        "description": "Return a command's input schema",
                        "parameters": { "id": { "type": "string", "required": true } } }
      }
    }
  }
}
```

| Verb         | Required parameters      | Purpose                          |
| ------------ | ------------------------ | -------------------------------- |
| `register`   | full command definition  | Add a command to the registry    |
| `list`       | — (optional filters)     | Enumerate active commands        |
| `execute`    | `id`, `ctx` (`force?`)   | Run a command                    |
| `available`  | `id`, `ctx`              | Check whether a command can run  |
| `unregister` | `id`                     | Remove a command                 |
| `schema`     | `id`                     | Return a command's input schema  |

Calls are plain `tools/call` with an `op` selector — `"<verb> command"`:

```text
tools/call("command", { op: "register command", id: "my.foo", name: "Foo", … })
tools/call("command", { op: "execute command",  id: "my.foo", ctx })
```

The TS surface — the direct `op` form is always available; the SDK also
offers the `<noun>.<verb>` path sugar built from the `_meta` tree:

```ts
// direct op form:
await this.commands.command({ op: "register command", id: "my.foo", name: "Foo" /* … */ });
await this.commands.command({ op: "execute command",  id: "my.foo", ctx });

// path-sugar form — server "commands", tool "command", noun "command":
await this.commands.command.command.execute({ id: "my.foo", ctx });
```

`id` is an ordinary parameter; command ids freely contain dots
(`myplugin.archive_stale`) because they are string values, never path
segments.

Registry changes notify via `notifications/commands/changed`, debounced ~100ms and flushed on plugin load/unload boundaries. The tool list itself does not change — `notifications/tools/list_changed` is never fired for command registration.

## Why an operation tool

Commands fits the operation-tool shape naturally:

- All verbs work on the same noun (a command), so they live in one
  tool and one `_meta` subtree; the tool list does not grow as verbs
  are added.
- Adding a verb (say, `disable`) is one new operation struct — one new
  `(verb, noun)` pair — picked up by `generate_mcp_schema` and the
  `_meta` generator automatically.
- The `_meta` noun → verb → parameters tree gives clean codegen and
  CLI generation with no per-verb bookkeeping.

A flat alternative — `register`, `execute`, etc. each as its own MCP tool — would work and the SDK would dispatch it fine, but the tool list grows linearly with verbs. An operation tool is the right shape when there is one noun and many verbs.

## Registration pattern (callbacks across the boundary)

```ts
this.commands.command({
  op: "register command",
  id: "stale.archive",
  name: "Archive stale cards on this board",
  category: "Cleanup",
  available: (ctx) =>
    ctx.location?.kind === "board" || { ok: false, reason: "Open a board first" },
  execute: async (ctx) => {
    /* … */
  },
});
```

`available` and `execute` are functions in the registration payload. The SDK's callback primitive strips them, registers ids locally, sends opaque markers in the registration call. When `execute command` is later invoked for `stale.archive`, the server emits callback invocations back to the registering plugin's isolate to run `available` then `execute`.

## Synchronous `available`

The palette evaluates every command's `available` on open; hotkey dispatch needs sub-ms response. The service contracts `available` as synchronous, returning `boolean | { ok: false, reason: string }`. The reason form powers tooltips for grayed-out entries.

Async preconditions are handled by event-driven caching: the plugin subscribes to whatever changes the precondition, maintains a cached flag, returns it synchronously. The server enforces a soft latency budget (~5ms warn, ~50ms force-false) so misbehaving commands can't tank the palette.

## `execute` re-checks `available`

The server invokes `available` immediately before `execute` and rejects with `CommandUnavailableError` if it returns false, unless `execute` is called with `{ force: true }`. Agents that have already verified can skip the recheck.

## Override stack semantics

Command ids live in a global namespace within the service. Any caller can register any id, including overriding built-ins. The most recent registration is active; registrations form a stack per id:

```
core.archive:
  ├── host registration              (built-in, registered at startup)
  ├── plugin-a registration          (override)
  └── plugin-b registration          ← active
```

Plugin B unloads → A's override re-emerges. A unloads → built-in re-emerges. Within a single registering caller, re-registration of the same id replaces that caller's entry on the stack rather than pushing a duplicate.

This is a service-specific design — the platform doesn't require it. Other services that want different semantics (collision rejection, namespacing) implement them in their own `register` tool.

## Invocation is open

The `execute command` operation is callable by anyone with access to the `commands` server. The invoker doesn't gain capabilities by invoking, since the command runs in the *registering* plugin's isolate. The integrity concern (a malicious plugin synthesizing a context to invoke a destructive command) is local to the command's effects, which is the command author's responsibility.

## Context

```ts
interface Context {
  // No specific fields required by the platform — Context is service-defined.
  // For commands, the host populates relevant snapshot data:
  location?: unknown;            // wherever the user is, supplied by host
  selection?: unknown;
  invocation: "palette" | "hotkey" | "menu" | "api" | "agent";
  modifiers?: { shift?: boolean; alt?: boolean; ctrl?: boolean; meta?: boolean };
}
```

`Context` is part of the Command service contract, not the platform. Other services define their own context shape or none at all.

## Namespace convention for ids

By convention, commands are id'd with the registering plugin's name as a prefix (`myplugin.archive_stale`). The Command service treats this as a soft convention, not enforced — the override stack handles collisions explicitly. Overriding a built-in or another plugin's command is something the registering plugin does deliberately and that other code can detect by inspecting the override stack.

## Platform primitives used

Every non-trivial behavior of the Command service is built on platform primitives:

- A single operation tool whose `_meta` operations tree is generated from `#[operation]` structs by `swissarmyhammer-operations` — consumed directly for codegen, CLI generation, and SDK path sugar.
- Callbacks across the boundary for `available` / `execute` — the universal callback primitive.
- Override stack semantics — internal data structure in the service's Rust impl.
- Soft latency budgets — service-internal concern.

None of it requires plugin-platform changes. The one piece that is new work — generating the `_meta` operations tree and auto-attaching it to operation tools — lands in `swissarmyhammer-operations`, and benefits every operation tool, not just this service. The same toolkit is available to any service the host or a plugin wants to build.

## Implementation plan

This section summarizes the work to ship the Command service and migrate the
existing system onto it. The authoritative, decomposed plan lives on the
kanban board across **seven layered projects** (`store-service`,
`command-service`, `entity-service`, `command-backends`, `builtin-commands`,
`command-events`, `command-cutover`); a frozen, reviewable mirror with per-task
IDs is in
[`command-service-plans/`](./command-service-plans/README.md). This is the
readable overview.

### What we are replacing

Today commands are declared in **12 YAML files / 62 commands** loaded by the
`swissarmyhammer-commands` Rust crate, dispatched through a `Command` trait
(`available()` + `execute(ctx)`):

- **Kanban-domain** (`crates/swissarmyhammer-kanban/builtin/commands/`, 7 files,
  29 commands): `task.yaml` (3), `column.yaml` (1), `attachment.yaml` (2),
  `tag.yaml` (1), `view.yaml` (1), `file.yaml` (4), `perspective.yaml` (17).
- **Platform-shell** (`crates/swissarmyhammer-commands/builtin/commands/`,
  5 files, 33 commands): `entity.yaml` (8), `ui.yaml` (10, includes
  `window.new`), `app.yaml` (9), `settings.yaml` (3), `drag.yaml` (3).

The kanban app's frontend dispatches via a `useDispatchCommand` hook against
the Rust registry, and reaches host capabilities through ~36 inline
`#[tauri::command]` handlers.

### Target architecture (decisions)

- **Built-ins become plugins on disk.** Every formerly-YAML command ships as a
  builtin TypeScript plugin under `builtin/plugins/`. The host has no
  command-specific code; built-ins ride the same path as user plugins.
- **UI metadata travels on the registration payload.** `keys`, `menu`,
  `contextMenu`, `tabButton`, `scope`, `params`, `undoable`, `visible` are
  fields on the `register command` call — single source of truth, read by the
  palette / hotkey / menu systems via `list command`.
- **Cut-over, not transitional.** The Command service, the builtin plugins, and
  the frontend dispatcher land together; the YAML loader and the
  `swissarmyhammer-commands` crate are deleted in the same change set.
- **Tauri commands become MCP servers.** Window/app handlers split into `window`
  and `app` in-process MCP servers; the frontend talks only MCP (except the
  `mcp_call` / `mcp_subscribe` transport itself, which stays Tauri).
- **Plugins register the services they depend on.** A command-registering
  plugin's `load()` calls `ensureServices(this, ["commands", ...])` before
  `registerCommands(...)`. This relies on **idempotent server registration**
  (a `swissarmyhammer-plugin` change): registering the same `(name, source)`
  is a no-op so any number of plugins can declare the same dependency; only a
  *different* source under an existing name errors with `ServerNameTaken`.

### Backend services (where the work actually happens)

A command's `execute` callback doesn't do the work itself — it calls a backend
MCP server. Investigation of the current code found that **only `kanban` exists
today**; the rest are state held in in-process Rust contexts
(`UIState`, `PerspectiveContext`/`ViewsContext`, `StoreContext`, `PasteMatrix`)
that the commands reach via `ctx.require_extension::<T>()` with no MCP surface.
Making the commands work as plugins therefore requires exposing those contexts
as MCP servers. Per the "fewer, consolidated servers" decision:

| Server | Status | Backs |
| ------ | ------ | ----- |
| `entity` | **new** | **generic** face over the entity kernel: get/list/add/update/delete + archive/unarchive + clipboard `cut/copy/paste` + **search** for *any* type (wraps `EntityContext`/`EntityCache` + `PasteMatrix` + `EntitySearchIndex`) |
| `kanban` | exists, unchanged surface | **domain** face over the SAME kernel: keeps ALL its ops (`add/update/delete/get` task/column/tag/project/actor, `move task`, `next/complete`, `assign`, `tag/untag`, board lifecycle); generic CRUD passes through to the kernel. **Nothing removed** |
| `views` | **new** | `perspective.*` + `view.set` (wraps `PerspectiveContext`/`ViewsContext`) |
| `ui_state` | **new** | `ui.*` (minus setFocus) + `settings.keymap.*` + `drag.*` + `app.command/palette/search/dismiss` (wraps the relocated `UIState`) |
| `window` | **new** | `window.new` + `file.*` board lifecycle + `attachment.open/reveal` (wraps tauri `AppHandle` + OS file ops) |
| `app` | **new** | `app.quit/about/help` only — genuine app-shell actions |
| `store` | **new** | **`undo`/`redo`** (unified stack), transaction grouping, per-item history; store-scoped ops take a `store` param (wraps the shared `StoreContext`) |
| `focus` | **new** (spatial-nav project) | `ui.setFocus` + spatial nav (wraps `SpatialRegistry`/`SpatialState`) |

**Kernel + two faces:** `EntityContext` is the entity kernel; `entity` is the
generic, type-agnostic face (incl. search), and `kanban` is the domain face that
**keeps its full operation surface** and delegates generic CRUD to the kernel.
Nothing is removed from kanban; `entity` is additive. Search is an entity
capability, not a separate server.

**Undo/redo is cross-cutting, and lives on its own `store` server — not `app`.**
`store.undo`/`store.redo` operate on the single unified stack (the
`single-changelog` / `StoreContext` kernel, one `Arc<StoreContext>` shared by
`kanban`/`views`/`store`). Every tracked write — entity edits via `kanban`,
perspective changes via `views`, clipboard/archive via `kanban` — pushes onto
that one stack, so `store.undo` reverts the last entry/group regardless of which
server produced it. The `app.undo`/`app.redo` *commands* route to
`store.undo`/`store.redo`. Note `undoable` (the YAML flag) is **declarative
metadata**, not the undo gate — undo is recorded by the store layer on every
tracked write.

Two relocations are forced by the cut-over (which deletes `swissarmyhammer-commands`):
`UIState` (and `window_info` if used) must move to surviving crates before that
deletion — owned by the `ui_state` and `window` server tasks respectively.

### Builtin plugin catalog (7 plugins, 62 commands)

`ensureServices` lists every server a plugin needs — `commands` to register into,
plus each backend its callbacks invoke.

| Plugin dir | Commands | `ensureServices` | Source YAML(s) |
| ---------- | -------: | ---------------- | -------------- |
| `task-commands` | 3 | `[commands, kanban]` | task.yaml |
| `kanban-misc-commands` | 5 | `[commands, kanban, entity, window, views]` | column, attachment, tag, view |
| `file-commands` | 4 | `[commands, window]` | file.yaml |
| `perspective-commands` | 17 | `[commands, views]` | perspective.yaml |
| `entity-commands` | 8 | `[commands, entity]` | entity.yaml |
| `ui-commands` | 10 | `[commands, ui_state, window, focus]` | ui.yaml (incl. `window.new`) |
| `app-shell-commands` | 15 | `[commands, app, ui_state, store]` | app, settings, drag |

A checked-in `crates/swissarmyhammer-command-service/tests/baseline/plugins.yaml`
captures this catalog with each command's full metadata. A drift test reads the
source YAML files and fails CI if the catalog and the YAML ever disagree, so no
command is silently dropped during the port. The cut-over end-to-end test
(`full_baseline_e2e.rs`) runs every catalogued command through the service and
asserts it produces the same effect as the YAML-driven version did.

### Events: how changes reach the UI and agents

A command's effects reach every dependent through **MCP notifications**, not
Tauri-specific events — so the webview and AI agents subscribe to the same
stream. Four planes, all carrying a `txn` (transaction correlation) and
`origin` (provenance):

1. **`store/changed {store,item,op,changes?,txn,origin}`** — data, for every
   stored thing (entities carry field-level `changes`; views/perspectives are
   reload-item until `single-changelog` unifies diff formats).
2. **`commands/executed {id,ctx,result,txn,origin}`** — the semantic action
   plane reactive (Obsidian-style) plugins subscribe to.
3. **registry/lifecycle** — `commands/changed`, `tools/list_changed`, board/plugin lifecycle.
4. **ephemeral** — `ui_state/changed`, `store/undo_changed`.

The Command service opens a `txn` around each `execute`; every store write
stamps both its undo `group_id` and its change events with it, so a command's N
changes form one undo group and one atomic UI batch. **Undo == edit downstream**:
undo/redo emit the same events as a forward edit (derived from the byte
transition), so the existing entity/field reload reducer is reused — only its
source changes (Tauri → MCP), plus `txn` batching.

### Build order — seven layered plans

See [`command-service-plans/`](./command-service-plans/README.md) for the full
per-task breakdown. Tiers (parallel within a tier):

- **Tier 0 (foundational):** `store-service` (shared substrate + `store` MCP),
  `command-service` (the engine: verbs, override stack, callbacks, SDK helpers,
  execute's txn-bracket).
- **Tier 1:** `entity-service` (generic `entity` MCP — CRUD/clipboard/archive/
  search), `command-backends` (`views`, `ui_state`, `window`, `app`).
- **Tier 2:** `builtin-commands` (catalog + 7 plugins + frontend dispatch),
  `command-events` (notification surface + undo/redo propagation + frontend
  subscription).
- **Tier 3 (terminal):** `command-cutover` (Tauri `invoke()` migration + delete
  `swissarmyhammer-commands` and all 12 YAMLs; `cargo build/test --workspace` +
  the full-baseline e2e are the gate).

Prerequisites in other projects: `plugin-arch` — idempotent server registration
(so plugins can `ensureServices` the same server) + the already-merged
`operation_tool!` macro; `spatial-nav` — a `focus` MCP server for `ui.setFocus`.
