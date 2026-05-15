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
