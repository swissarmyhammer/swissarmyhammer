# Command Service

The Command service is an MCP server that ships with the platform. It is built on top of the [plugin architecture](./plugin-architecture.md) — the platform itself has no concept of "the command service"; it is a registered server like any other.

The service exists for two reasons: it is needed by the host UI (command palette, hotkey dispatch, menus), and it doubles as a worked example of building a non-trivial service on the platform. Every pattern it uses — operation-mode dispatch, callbacks across the host/plugin boundary, structured per-verb metadata, override stacks — is available to any other service.

## What the service does

Commands lets clients (plugins, the user via UI, external agents) register named, user-invocable actions with two-phase semantics:

- `available(ctx)` — synchronous precondition check
- `execute(ctx)` — async action

The command palette, hotkey dispatch, menu system, and agent integrations all invoke commands through this server. Override semantics let plugins replace built-ins.

## MCP surface (operation-based)

Commands is an **operation-based** server. It declares the operations capability in its `ServerCapabilities` at initialize time:

```json
{
  "capabilities": {
    "experimental": {
      "io.swissarmyhammer/operations": { "version": "1" }
    },
    "tools": { "listChanged": false }
  }
}
```

It exposes one tool — `command` (the noun class) — with a verb set that
covers both collection-wide operations (register, list) and operations
on a specific command id (execute, available, schema, unregister).
Verbs that act on a specific command take `id` as a required property
of their input, like any other parameter.

| Verb         | Input (required fields)              | Output           | Purpose                               |
| ------------ | ------------------------------------ | ---------------- | ------------------------------------- |
| `register`   | full command definition (incl. `id`) | `RegisterOutput` | Add a command to the registry         |
| `list`       | optional filters                     | `[CommandSummary]` | Enumerate active commands           |
| `unregister` | `{ id }`                             | `void`           | Remove a command                      |
| `available`  | `{ id, ctx }`                        | `AvailableOutput` | Check whether a command can run      |
| `execute`    | `{ id, ctx, force? }`                | `ExecuteOutput`  | Run a command                         |
| `schema`     | `{ id }`                             | JSON schema      | Return the input schema for a command |

TS surface:

```ts
await this.commands.command.register({ id: "my.foo", name: "Foo", /* … */ });
const all = await this.commands.command.list({});

await this.commands.command.execute({ id: "my.foo", ctx });
await this.commands.command.unregister({ id: "my.foo" });
const ok = await this.commands.command.available({ id: "my.foo", ctx });
const schema = await this.commands.command.schema({ id: "my.foo" });
```

Command ids commonly contain dots (`myplugin.archive_stale`), but
that's irrelevant here — the id is a string property in the args
object, never a JS path segment.

Dispatch:
- `register`: `call("commands", "command", verb="register", { id: "my.foo", … })`
- `execute`:  `call("commands", "command", verb="execute",  { id: "my.foo", ctx })`

Wire (the verb folds into the arguments map at the MCP boundary because
MCP itself has no verb concept; every other parameter is already a
property of the arguments map):
- `tools/call("command", { verb: "register", id: "my.foo", … })`
- `tools/call("command", { verb: "execute",  id: "my.foo", ctx })`

Registry changes notify via `notifications/commands/changed`, debounced ~100ms and flushed on plugin load/unload boundaries. The shape of `tools/list` does not change — `notifications/tools/list_changed` is never fired for command registration.

## Why operation-based

Commands fits the operation pattern naturally:

- All verbs work on the same noun class (a command), so they share the
  same tool entry and the metadata stays grouped by purpose.
- Adding a new verb (say, `disable`) is one new entry in the tool's
  verb map; the tool list itself doesn't grow.
- The structured per-verb metadata makes for clean codegen and CLI
  generation: each verb is a typed `(input, output)` pair end-to-end.

A flat alternative — `commands.register`, `commands.execute`, etc., each as its own MCP tool — would work and the SDK would dispatch it fine, but the surface grows linearly with verbs and tool-list bookkeeping has more ceremony. Operation-based is the right shape when there's one noun and many verbs.

## Registration pattern (callbacks across the boundary)

```ts
this.commands.command.register({
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

`available` and `execute` are functions in the registration payload. The SDK's callback primitive strips them, registers ids locally, sends opaque markers in the registration call. When `commands.command.execute({ id: "stale.archive", ctx })` is invoked later, the server emits callback invocations back to the registering plugin's isolate to run `available` then `execute`.

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

`commands.command.execute({ id, ... })` is callable by anyone with access to the `commands` server. The invoker doesn't gain capabilities by invoking, since the command runs in the *registering* plugin's isolate. The integrity concern (a malicious plugin synthesizing a context to invoke a destructive command) is local to the command's effects, which is the command author's responsibility.

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

- A fixed MCP tool surface with structured per-verb operation metadata — declared via attributes on the Rust impl, consumed directly for codegen and dispatch.
- Callbacks across the boundary for `available` / `execute` — the universal callback primitive.
- Override stack semantics — internal data structure in the service's Rust impl.
- Soft latency budgets — service-internal concern.

None of it requires platform changes. The same toolkit is available to any service the host or a plugin wants to build.
