# swissarmyhammer Plugin Architecture

## Overview

The plugin platform does two things and only two things:

1. Lets plugins **register MCP servers** with the host.
2. Lets plugins **consume any registered MCP server** through a generic
   dispatcher on `this`.

Everything else is built on top. The kanban data layer, navigation, the
command palette, entity CRUD, views, perspectives, agent capabilities —
all of these are MCP servers registered by some party (host, plugin, or
extension package). The plugin platform has no concept of "the navigation
service" or "the command service." It has a registry of MCP servers and a
dispatcher to them.

This commitment makes the architecture small. The architecture doc is
small as a result.

The platform ships as one new workspace crate plus changes to five
existing ones — see *Crates* immediately below.

## Crates

Building this platform is **one new crate and changes to five existing
crates**. Every claim in this section is reflected in the relevant
detailed section, cross-referenced below.

### New crate

- **`swissarmyhammer-plugin`** — the plugin platform itself. Owns the
  `Plugin` base class and SDK, the `ServerRegistry`, the `Dispatcher`,
  the `McpServer` trait and its three transport implementations
  (`InProcessServer` wrapping `rmcp` handlers, `CliServer`,
  `UrlServer`), the deno_core runtime integration, the per-plugin
  ledger, the `TypesEmitter` codegen, and the hot-reload machinery. It
  depends on `swissarmyhammer-directory` for plugin discovery. Host
  binaries (the Tauri kanban app, the TUI, headless) gain a dependency
  on it; nothing else is asked of them.

### Modified crates

- **`swissarmyhammer-operations`** — gains a `_meta`-tree generator
  that builds the `io.swissarmyhammer/operations` noun → verb →
  parameters value from a `&[&dyn Operation]` slice, alongside the
  existing `generate_mcp_schema`. This is the only place operation
  discovery metadata is produced. (See *Operation tools and
  `swissarmyhammer-operations`*.)
- **`swissarmyhammer-operations-macros`** — gains the operation-tool
  macro that declares an MCP tool from its operation set and
  auto-attaches the generated `_meta` to the tool definition, so
  operation tools are self-describing by construction and the `_meta`
  cannot drift from the operation structs. (The existing `#[operation]`
  and `#[param]` macros already live here.)
- **`swissarmyhammer-tools`** — its MCP server bootstrap
  (`crates/swissarmyhammer-tools/src/mcp/server.rs`) gains the
  `expose_rust_module` calls that hand the existing in-process tools
  (`files`, `kanban`, `code_context`, `git`, `shell`, …) to the plugin
  registry. The tools keep their `McpTool` / `ToolRegistry` home in
  this crate; only the exposure glue is new. (See *Rust — in-process
  Rust module*.)
- **`swissarmyhammer-directory`** — gains the stack-aware `Watcher<C>`
  (plus a shared `async-watcher` plumbing helper) that drives plugin
  discovery and hot reload across the builtin → user → project layers.
  (See *Plugin Discovery*.)
- **`swissarmyhammer-js`** — converted from `rquickjs` (QuickJS-NG) to
  `deno_core`, so the workspace has exactly one JavaScript engine. Its
  public API (`JsState::global()`, `set` / `get`) is preserved, so its
  one consumer — `swissarmyhammer-fields`' `ValidationEngine` — is
  unchanged. The existing dedicated-worker-thread + mpsc-channel model
  carries over directly: `deno_core::JsRuntime` is single-threaded and
  wants exactly that pattern. (See *Runtime: deno_core*.)

No other crate changes. The platform deliberately consumes the
existing operation infrastructure rather than forking it.

## Principles

1. **The plugin base class is register/unregister + generic dispatch.**
   That's the API. New service capabilities don't require platform changes;
   they're new MCP servers.
2. **MCP servers are the source of truth for their tools.** Servers
   exist as separate artifacts — URL endpoints, CLI binaries, or
   in-process Rust modules using `rmcp`. Plugins reference them by
   source; they never describe what's in them. The platform discovers
   tool metadata by calling `tools/list` on the connected server.
   Schemas, descriptions, verb enumerations live on the server, once.
3. **No hard-coded service or tool list.** The base class, SDK, and
   dispatcher know nothing about specific server or tool names. Every
   call is a plain MCP `tools/call(tool, arguments)` — one tool name,
   one arguments map. The platform never parses `arguments`.
   - **Flat tool:** one tool, one operation. `this.foo.bar(args)` →
     `tools/call("bar", args)`.
   - **Operation tool:** one tool bundling many `(verb, noun)`
     operations behind an `op` argument — the pattern
     `swissarmyhammer-operations` already generates for `files`,
     `kanban`, `code_context`, etc. `this.foo.bar.<noun>.<verb>(args)`
     → `tools/call("bar", { op: "<verb> <noun>", ...args })`. `op` is
     an ordinary argument key; the tool's own handler reads it. The
     SDK builds the `op` string from the path purely as ergonomic
     sugar (see *Service Consumption*).
4. **One contract, one runtime.** All plugins run in deno_core embedded
   in the host's main Rust process — same engine across hosts. Plugin
   source is portable.
5. **In-process dispatch where possible.** Host-implemented Rust servers
   dispatch in-process with no serialization. Plugin-registered URL and
   CLI sources pay one MCP-protocol round-trip per call. External agents
   reach any server through the same transport adapters.
6. **Host owns lifecycle bookkeeping.** Every registration a plugin makes
   is tracked by the host and auto-disposed on unload. Plugins don't manage
   cleanup; they just register things.

## Plugin Base Class

The entire API surface of `Plugin`:

```ts
export abstract class Plugin {
  // Lifecycle (both optional in subclasses):
  load(): Promise<void>;
  unload(): Promise<void>;

  // Server lifecycle — point the platform at an MCP server that already
  // exists. Three source kinds; pick one. The plugin never describes the
  // server's tools — the platform queries them from the server itself
  // via `tools/list`.
  register(name: string, source: ServerSource): void;
  unregister(name: string): void;

  // Dynamic dispatch for any registered server. Every leaf call is a
  // plain MCP tools/call:
  //   Flat tool:       this.foo.bar(input)        → tools/call("bar", input)
  //   Operation tool:  this.foo.bar.task.add(input)
  //                      → tools/call("bar", { op: "add task", ...input })
  readonly [server: string]: ServerDispatcher;

  // Convenience: scoped logger and mid-session disposable tracker
  log: Logger;
  track(d: Disposable): Disposable;
}

type ServerSource =
  | { url:  string; headers?: Record<string, string> }                     // HTTP
  | { cli:  string[]; env?: Record<string, string>; cwd?: string }         // stdio subprocess
  | { rust: string };                                                       // host-exposed Rust module, by id
```

That's the whole thing. There is no `this.kanban`, no `this.commands`, no
`this.navigation` defined anywhere in the SDK — those names work because
servers with those names happen to be registered, and the Proxy on `this`
dispatches to them.

`track(disposable)` is a convenience for mid-session cleanup, not a
requirement. The host tracks every registration a plugin makes (servers,
callbacks, anything that lives across calls) and auto-disposes on unload.

## Service Consumption

Once a server is registered, plugins call its tools through `this`. Tools
come in two shapes — flat and operation tools — and the SDK supports both
through the same generic Proxy. Which shape applies to a given tool is
determined by the tool's own `inputSchema`, never by anything baked into
the SDK.

### Flat tools

One MCP tool, one operation. TS surface is `this.<server>.<tool>(args)`,
`args` a single object.

```ts
// Server "weather" exposes flat tools "current" and "forecast":
const now    = await this.weather.current({ city: "Austin" });
const next3  = await this.weather.forecast({ city: "Austin", days: 3 });
```

Wire (MCP): `tools/call("current", { city: "Austin" })` on the
`weather` connection — the platform passes the tool name and the
arguments map straight through.

### Operation tools

Most tools the host ships are **operation tools**: a single MCP tool
that bundles many `(verb, noun)` operations behind an `op` argument.
This is the shape `swissarmyhammer-operations` already produces for
`files`, `kanban`, `code_context`, `git`, `shell`, and the rest — one
tool name, an `op` string like `"add task"`, and the operation's
parameters flat alongside it. The plugin platform inherits that shape;
it does not define a new one.

An operation tool describes itself through the **`_meta`** field on its
`Tool` definition. `_meta` is the MCP-standard extension point
([MCP 2025-11-25](https://modelcontextprotocol.io/specification/2025-11-25/basic/index#_meta)):
a reserved key/value map for metadata, where each key is a
reverse-DNS-prefixed name. The operation metadata lives under the key
`io.swissarmyhammer/operations` — a valid, non-reserved `_meta` key
(reserved keys are only those whose second prefix label is
`modelcontextprotocol` or `mcp`).

The value under that key is a **noun → verb → parameters** tree — the
structure that actually reflects how operations are organized:

```jsonc
{
  "name": "kanban",
  "description": "Kanban board operations",
  "inputSchema": {
    // Plain JSON Schema, kept flat for validation and Claude API
    // compatibility: the `op` selector plus the union of all parameters.
    "type": "object",
    "additionalProperties": true,
    "properties": {
      "op":     { "type": "string",
                  "enum": ["add task", "move task", "init board" /* … */] },
      "title":  { "type": "string" },
      "id":     { "type": "string" },
      "column": { "type": "string" }
      // …
    }
  },
  "_meta": {
    "io.swissarmyhammer/operations": {
      // noun → verb → { op string, description, parameters }
      "task": {
        "add": {
          "op": "add task",
          "description": "Create a new task",
          "parameters": {
            "title":       { "type": "string",  "required": true,  "description": "Task title" },
            "description": { "type": "string",  "required": false, "description": "Task body" }
          }
        },
        "move": {
          "op": "move task",
          "description": "Move a task to a column",
          "parameters": {
            "id":     { "type": "string", "required": true, "description": "Task id" },
            "column": { "type": "string", "required": true, "description": "Target column" }
          }
        }
        // … one entry per verb on `task` …
      },
      "board": {
        "init": {
          "op": "init board",
          "description": "Initialize a new board",
          "parameters": { "name": { "type": "string", "required": true, "description": "Board name" } }
        }
        // …
      }
      // … one entry per noun …
    }
  }
}
```

A tool is an operation tool **iff its `_meta` carries the
`io.swissarmyhammer/operations` key**. The three levels — noun, then
verb, then parameters — are exactly what a CLI generator, the SDK
sugar, and codegen each need, with no flattening to undo.

This `_meta` does not exist in the workspace yet. Producing it is a
change to `swissarmyhammer-operations` and its macros — see *Operation
tools and `swissarmyhammer-operations`*.

### Calling an operation tool

The wire call is an ordinary `tools/call`: the tool name, and an
arguments map whose `op` key selects the operation. Every other
parameter is a flat sibling of `op` — there is no separate verb field
and no noun in the tool name. This is how operation tools are invoked
today (`FilesTool::execute` reads `arguments["op"]` and matches it);
the plugin platform does not change it.

```text
tools/call("kanban", { op: "add task",  title: "Fix login bug" })
tools/call("kanban", { op: "move task", id: "t_12", column: "doing" })
```

The TS surface is sugar over that single call. The SDK reads the
`io.swissarmyhammer/operations` `_meta` tree and lets the plugin author
spell the operation as a `noun.verb` path:

```ts
// server "board" exposes the operation tool "kanban":
await this.board.kanban.task.add({ title: "Fix login bug" });
await this.board.kanban.task.move({ id: "t_12", column: "doing" });
await this.board.kanban.board.init({ name: "My Project" });
const all = await this.board.kanban.task.list({});
```

`this.<server>.<tool>.<noun>.<verb>(args)` compiles to
`tools/call("<tool>", { op: "<verb> <noun>", ...args })` — the SDK
looks up `_meta…[noun][verb].op` for the exact `op` string and folds
`args` in flat. The path sugar is optional: a plugin can pass `op`
itself with `this.board.kanban({ op: "add task", title: "…" })`. The
`op` form is the ground truth; the path is a convenience the SDK
derives from `_meta`.

The instance identifier (`id`, `task_id`, …) is just one of the flat
parameters — it has no special place in the path or on the wire. `op`
strings and id values freely contain dots, hyphens, and slashes; they
are string values, never JS path segments.

### Why operation tools

`swissarmyhammer-operations` already uses the one-tool-many-operations
shape across the workspace, so the plugin platform consumes it rather
than inventing a parallel one:

- One MCP tool covers a whole domain; the tool list stays small as
  operations grow.
- The `_meta` tree gives noun-grouped, verb-nested, parameter-typed
  metadata in one structure — the CLI generator
  (`<bin> <noun> <verb> --args`), the host's own callers, the plugin
  SDK path sugar, and codegen all read the same tree.
- `op` stays the single wire selector, so the call is a plain
  `tools/call` no matter how rich the discovery metadata gets.

The [Command service](./command-service.md) is a worked example.

### How the SDK reads `_meta`

The SDK caches each tool's `Tool` definition from `tools/list`. A tool
whose `_meta` has `io.swissarmyhammer/operations` is an operation tool;
without it, flat. The Proxy on `this.<server>...` resolves a call path
against that cached tree:

- **Flat tool:** `this.weather.current(args)` → path `[current]` →
  `tools/call("current", args)`.
- **Operation tool, path form:** `this.board.kanban.task.add(args)` →
  path `[kanban, task, add]` → the SDK reads
  `_meta…operations["task"]["add"].op` → `"add task"` →
  `tools/call("kanban", { op: "add task", ...args })`.
- **Operation tool, direct form:** `this.board.kanban({ op: "add task", ...})`
  → path `[kanban]`, `op` already in args → `tools/call("kanban", args)`.
- **Unknown noun/verb:** the `noun`/`verb` segment is not in the
  `_meta` tree → runtime error (`UnknownOperation`) listing the valid
  verbs for that noun straight from `_meta`.

```ts
// Generic dispatcher used for both shapes; capability awareness is
// layered on top via cached metadata, not baked into this primitive:
function makeDispatcher(transport: Transport, server: string, path: string[] = []) {
  const fn = (input?: unknown) =>
    transport.callPath(server, path, input ?? {});  // transport resolves shape from _meta
  return new Proxy(fn, {
    get(_, prop) {
      if (typeof prop !== "string") return undefined;
      if (prop === "then") return undefined;
      if (RESERVED.has(prop)) return reservedHandler(prop, server, path);
      return makeDispatcher(transport, server, [...path, prop]);
    },
  });
}

function makePluginThis(transport: Transport, base: PluginBase): Plugin {
  return new Proxy(base, {
    get(target, prop) {
      if (typeof prop !== "string") return Reflect.get(target, prop);
      if (prop in target) return Reflect.get(target, prop);    // base methods
      return makeDispatcher(transport, prop);                  // dynamic server
    },
  });
}
```

`transport.callPath(server, path, args)` consults the cached `Tool`
definition for `path[0]` (the tool):
- **flat tool** (no `io.swissarmyhammer/operations` in `_meta`): the
  path is `[tool]`; dispatch `tools/call(tool, args)`.
- **operation tool**, path `[tool, noun, verb]`: look up
  `_meta…operations[noun][verb].op` for the `op` string, dispatch
  `tools/call(tool, { op, ...args })`.
- **operation tool**, path `[tool]` with `op` already in `args`:
  dispatch `tools/call(tool, args)` unchanged — the direct form.

Either way the wire call is a plain `tools/call`; `op` is just an
argument. The platform never invents a noun/verb wire axis.

`RESERVED` covers SDK-handled names (`on`, `off`, `once`, `subscribe`,
`unsubscribe`) rather than forwarding them as tool, noun, or verb
segments.

### Parameters are always passed as `{}`

Both shapes take a single object argument. No positional args, no varargs,
no `Maybe<Args>`. This keeps wire encoding (JSON object) and TS
ergonomics consistent across the platform.

If a plugin accesses `this.nonexistent.foo()` and no `nonexistent` server
is registered, the call fails at dispatch time with `UnknownServer`. The
Proxy itself doesn't know what's registered; it asks the host on every
call.

## Service Registration

A plugin doesn't build MCP servers. It points the platform at servers
that already exist, by source. The platform connects via the appropriate
transport, calls `tools/list` on the server, caches the schema, and
routes calls. **Plugins never declare schemas in any form** — describing
a server's tools is the server's job, and asking the plugin to do it
again is duplicating work that's already done.

The signature: `register(name, source)`. The `name` is the registry key
the server will be reachable under (`this.<name>...`). The `source` is
one of three:

```ts
type ServerSource =
  | { url:  string; headers?: Record<string, string> }                     // HTTP
  | { cli:  string[]; env?: Record<string, string>; cwd?: string }         // stdio subprocess
  | { rust: string };                                                       // host-exposed Rust module, by id
```

There are exactly three places an MCP server can live:

| Source        | What it means                                                       | Transport          |
| ------------- | ------------------------------------------------------------------- | ------------------ |
| **url**       | HTTP-served MCP server somewhere on the network                     | HTTP               |
| **cli**       | A command to spawn as a subprocess; speaks MCP over its stdio       | stdio (subprocess) |
| **rust**      | Reference to a Rust module the host has exposed in its registry     | in-process (no IPC)|

For Rust modules to be available, the host build must compile them in
and call `host.exposeRustModule(id, server)` at startup. After that,
plugins (or the host itself) can register them under any name they
choose.

The host modules worth exposing are the existing in-process MCP tools —
`files`, `kanban`, `code_context`, `git`, `shell`, and the rest — which
all live in **`swissarmyhammer-tools`**. Wiring them up is therefore a
change to `swissarmyhammer-tools`: its MCP server bootstrap
(`crates/swissarmyhammer-tools/src/mcp/server.rs`) gains the
`exposeRustModule` calls that hand each tool to the plugin platform's
registry. `swissarmyhammer-tools` already owns the `McpTool` trait and
`ToolRegistry`; exposing those tools to `swissarmyhammer-plugin` is new
glue in that crate, not a new home for the tools.

### URL — external HTTP-served server

```ts
async load() {
  this.register("weather", {
    url: "https://weather.example.com/mcp",
    headers: { Authorization: `Bearer ${process.env.WEATHER_TOKEN}` },
  });
}
```

The platform performs the MCP initialize handshake over HTTP, calls
`tools/list`, caches what comes back, and routes future calls. The
plugin author never describes a single tool.

### CLI — subprocess server (stdio)

```ts
async load() {
  this.register("github", {
    cli: ["npx", "-y", "@modelcontextprotocol/server-github"],
    env: { GITHUB_TOKEN: process.env.GITHUB_TOKEN ?? "" },
  });
}
```

The platform spawns the subprocess, performs the MCP handshake over
stdio, queries `tools/list`, and routes future calls. Subprocess
lifecycle (restart on crash, kill on plugin unload) is managed by the
platform.

This is how plugin authors who want to *add new tools* contribute them:
write a standalone MCP server in any language, ship it as a CLI binary
(or as an npm package invoked via `npx`), reference it from your plugin.
The MCP server is a separate artifact with its own development, testing,
and publishing lifecycle. The plugin is the wiring.

### Rust — in-process Rust module

For Rust modules the host has exposed, plugins reference them by id:

```ts
async load() {
  // Host exposed `kanban_core` at startup; activate it under "kanban":
  this.register("kanban", { rust: "kanban_core" });
}
```

The host's startup code is what makes Rust modules available. For this
host that startup code is `swissarmyhammer-tools`' MCP server bootstrap
(`crates/swissarmyhammer-tools/src/mcp/server.rs`), extended to hand
its existing tools to the plugin platform:

```rust
// swissarmyhammer-tools MCP bootstrap — new wiring
host.expose_rust_module("files",        FilesTool::new());
host.expose_rust_module("kanban",       KanbanTool::new(state.clone()));
host.expose_rust_module("code_context", CodeContextTool::new(db.clone()));
// … one per tool already in swissarmyhammer-tools …
```

`expose_rust_module` registers the handler in a separate "available
modules" table; it doesn't put the server into the live registry. A
subsequent `register(name, { rust: id })` call (from the host itself or
from a plugin) activates it under the chosen name. The tools are not
moved or rewritten — they keep their `McpTool`/`ToolRegistry` home in
`swissarmyhammer-tools`; only the `expose_rust_module` glue is new.

This decouples *which Rust code the host has compiled in* from *which
servers are live and under what names*. The host can ship a library of
Rust modules; plugins (or the host's own config) choose which to expose,
under what names, and to whom. See *In-Process Host Servers (rmcp)*
below for how the Rust modules themselves are written.

This is the only source that bypasses serialization. URL and CLI sources
always pay one MCP-protocol round-trip per call.

### Schemas come from `initialize` + `tools/list`. Always.

For every source, the platform's flow is identical:

1. Establish connection (HTTP request, spawn subprocess, or attach to
   in-process handler).
2. Send MCP `initialize`. The server returns `ServerCapabilities`.
3. Send `tools/list`. The server returns each tool's `name`,
   `description`, and `inputSchema`.
4. Cache the tool list. Subscribe to `notifications/tools/list_changed`
   so the cache stays current.
5. Make the server available as `this.<name>` on plugin `this`.

Operation tools are identified per-tool, not per-server: a tool whose
`Tool._meta` carries the `io.swissarmyhammer/operations` key is an
operation tool, and the SDK offers `<tool>.<noun>.<verb>(args)` path
sugar over the noun → verb → parameters tree under that key. A tool
without it is flat — `<tool>(args)`. One server can expose a mix.
There is no server capability flag; the operation metadata travels in
each tool's own `_meta` (see *Service Consumption*). The plugin author
never declares anything; the shape comes from the cached `Tool`
definition.

### Name collisions

The server registry has a single global namespace. The first registration
of a name wins; subsequent attempts fail with `ServerNameTaken`. The
host reserves the names it registers at startup. Collisions surface at
runtime, when `register(name, source)` returns the error — the platform
has no install-time declaration to check against.

Servers do not have override-anything semantics. They're heavier units
than commands (multi-tool surfaces, possibly stateful, possibly remote);
silent shadowing would break consumers' expectations about what they're
calling. If a service wants override semantics, that's a design decision
for the service to make in its own tools (the [Command service](./command-service.md)
is an example), not a platform feature.

### Unregistration

`this.unregister(name)` removes one of the plugin's servers from the
registry mid-session. Consumers' in-flight calls into that server reject
with `ServerUnavailable`; subsequent calls fail the same way until the
server is re-registered. For CLI sources, the subprocess is killed.
For URL sources, the connection is closed.

Plugin unload auto-unregisters every server the plugin had registered,
so explicit `unregister` is only needed when a plugin wants to drop a
server without unloading entirely.

## In-Process Host Servers (rmcp)

Host Rust code is a first-class server provider too. The platform's
`ServerRegistry` accepts in-process servers built with **rmcp** (the
official Rust MCP SDK) directly — no subprocess, no stdio framing, no
JSON-RPC overhead. Same registry, same dispatcher, same `McpServer` trait
that plugin-provided servers implement.

The deliberate property: there is no second way to build server-side code
in this project. Host code uses rmcp idiomatically — `#[tool_router]`,
`#[tool]`, `#[tool_handler]`, schema-derived `Parameters<T>` — and the
exact same handler value goes into our registry. If you want to *also*
expose the same server over stdio or HTTP for external clients, that's
one extra line at startup; nothing about the in-process registration
changes.

### Writing the server (just rmcp)

```rust
use rmcp::{
    model::*, tool, tool_router, tool_handler, ServerHandler, ErrorData,
    handler::server::tool::{ToolRouter, Parameters},
};
use schemars::JsonSchema;

#[derive(Clone)]
pub struct KanbanServer {
    state: AppState,
    tool_router: ToolRouter<Self>,
}

#[derive(serde::Deserialize, JsonSchema)]
struct ArchiveStaleInput { board: String, older_than: String }

#[tool_router]
impl KanbanServer {
    pub fn new(state: AppState) -> Self {
        Self { state, tool_router: Self::tool_router() }
    }

    #[tool(description = "Archive cards on a board older than a given age")]
    async fn archive_stale(
        &self,
        Parameters(input): Parameters<ArchiveStaleInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let archived = self.state
            .archive_stale(&input.board, &input.older_than).await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![
            Content::text(serde_json::to_string(&archived).unwrap()),
        ]))
    }
}

#[tool_handler]
impl ServerHandler for KanbanServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
```

This is plain rmcp. It compiles and works as a stdio MCP server via
`KanbanServer::new(state).serve(stdio())` without involving the platform
at all. The platform just gives it another home.

### Registering it in-process

```rust
let kanban = Arc::new(KanbanServer::new(state.clone()));

registry.register(
    "kanban",
    Arc::new(InProcessServer::from_arc(kanban.clone())),
)?;
```

`InProcessServer<S>` is the adapter — a thin generic wrapper that
implements the platform's `McpServer` trait by calling `rmcp`'s handler
methods directly:

```rust
pub struct InProcessServer<S> {
    inner: Arc<S>,
}

impl<S: ServerHandler> InProcessServer<S> {
    pub fn new(inner: S) -> Self { Self { inner: Arc::new(inner) } }
    pub fn from_arc(inner: Arc<S>) -> Self { Self { inner } }
}

#[async_trait]
impl<S: ServerHandler + Send + Sync + 'static> McpServer for InProcessServer<S> {
    fn tools(&self) -> Vec<ToolMetadata> {
        // call inner.list_tools synchronously via a blocking adapter,
        // or cache once at construction; rmcp's list_tools is essentially
        // a router descriptor enumeration.
        self.inner.list_tools_sync()
    }

    async fn invoke(
        &self,
        caller: CallerId,
        tool: &str,
        input: Value,
    ) -> Result<Value> {
        // A call is just a tool name and an arguments map. For an
        // operation tool the `op` selector is already a key inside
        // `input` — the SDK put it there. The adapter does not parse
        // or special-case it; that is the tool handler's job.
        let request = CallToolRequestParam {
            name: tool.into(),
            arguments: input.as_object().cloned(),
        };
        let mut context = RequestContext::<RoleServer>::default();
        context.extensions.insert(caller);             // propagate CallerId
        let result = self.inner.call_tool(request, context).await?;
        Ok(result.into_value())                        // CallToolResult → Value
    }
}
```

No transport, no JSON-RPC. Calls to
`this.board.kanban.task.move({ id: "t_12", column: "doing" })` from a
plugin become `tools/call("kanban", { op: "move task", id: "t_12",
column: "doing" })` and land in the host's V8 dispatch path →
`Dispatcher::call` → `InProcessServer::invoke` →
`rmcp::ServerHandler::call_tool` → the tool's `op` match → the Rust
operation body. The whole thing is async function composition.

### Operation tools and `swissarmyhammer-operations`

An operation tool's metadata is **not hand-written and not invented by
the plugin platform** — it is generated from the operation definitions
by the `swissarmyhammer-operations` crate. Part of that generation
exists today; the `_meta` tree this design depends on does not, and is
new work in `swissarmyhammer-operations` and its macros, called out
explicitly below.

**What exists today.** An operation is a struct. The `#[operation]`
attribute (from `swissarmyhammer-operations`) carries the verb, noun,
and description; the struct's fields *are* the parameters, their doc
comments the parameter descriptions:

```rust
use swissarmyhammer_operations::{operation, Execute, ExecutionResult, async_trait, Value};
use serde::Deserialize;

/// Register a new command in the registry.
#[operation(verb = "register", noun = "command",
            description = "Register a new command in the registry")]
#[derive(Default, Deserialize)]
pub struct RegisterCommand {
    /// The command id (e.g. "myplugin.archive_stale").
    pub id: String,
    /// Human-readable name shown in the palette.
    pub name: String,
    /// Optional grouping category.
    pub category: Option<String>,
}

/// Run the command identified by `id`.
#[operation(verb = "execute", noun = "command",
            description = "Run a command in the given context")]
#[derive(Default, Deserialize)]
pub struct ExecuteCommand {
    /// The command id to run.
    pub id: String,
    /// Skip the availability re-check.
    pub force: Option<bool>,
}

// … one struct per operation: ListCommand, AvailableCommand,
//   UnregisterCommand, SchemaCommand …

#[async_trait]
impl Execute<CommandContext, CommandError> for ExecuteCommand {
    async fn execute(&self, ctx: &CommandContext) -> ExecutionResult<Value, CommandError> {
        // self.id and self.force are the already-parsed parameters
    }
}
```

The `#[operation]` macro generates the `Operation` trait impl —
`verb()`, `noun()`, `description()`, and `parameters()` (field metadata
derived from the struct fields and their doc comments). `op_string()`
is `format!("{verb} {noun}")`, e.g. `"execute command"`. A tool
collects its operations as `&[&dyn Operation]` and calls
`generate_mcp_schema` to build the `inputSchema` it advertises in
`tools/list` — the flat `op` enum plus the parameter union, kept free
of `oneOf`/`anyOf` for Claude API compatibility. Its `tools/call`
handler is an `op` match (read `arguments["op"]`, strip it, deserialize
the rest into the matching operation struct, run its `Execute` impl) —
exactly how `FilesTool::execute` works today.

**What this design adds.** Nothing above produces the noun → verb →
parameters `_meta` tree that *Service Consumption* depends on. Two
changes are required, both in `swissarmyhammer-operations`:

1. **A `_meta`-tree generator.** Alongside `generate_mcp_schema`,
   `swissarmyhammer-operations` gains a generator that builds the
   `io.swissarmyhammer/operations` value — `{ noun: { verb: { op,
   description, parameters } } }` — from the same `&[&dyn Operation]`
   slice. Every level is derived from data the `Operation` trait
   already exposes: `noun()`, `verb()`, `op_string()`, `description()`,
   and `parameters()`. Nothing new is asked of the operation author.

2. **Macro code that auto-attaches it.** An operation tool must not
   hand-assemble its `Tool._meta`. New macro support — the
   operation-tool analogue of `#[operation]` — declares a tool from
   its operation set and emits the `_meta` key onto the tool
   definition automatically, next to the generated `inputSchema`. The
   result: every operation tool the host (or a plugin) builds is
   self-describing by construction, and the `_meta` can never drift
   from the operation structs.

The wire call format does not change — `op` stays the single selector
argument and the `tools/call` handler stays an `op` match. Only the
discovery metadata gains the structured `_meta` tree. Because that
tree is noun-grouped, verb-nested, and parameter-typed, a generic MCP
client, the CLI generator, the plugin SDK, and codegen all read the
operations and their parameters straight off it — no discriminated
union to disassemble, no schema archaeology.

### Propagating `CallerId`

The platform threads caller identity (plugin id, external agent id, host-
internal) through the dispatcher. The adapter stuffs it into
`RequestContext::extensions` so rmcp handlers that need it can fetch:

```rust
#[tool(description = "Delete a board (requires admin)")]
async fn delete_board(
    &self,
    Parameters(input): Parameters<DeleteBoardInput>,
    ctx: RequestContext<RoleServer>,
) -> Result<CallToolResult, ErrorData> {
    let caller = ctx.extensions.get::<CallerId>().cloned().unwrap_or(CallerId::Unknown);
    if !self.state.is_admin(&caller).await {
        return Err(ErrorData::invalid_request("admin only", None));
    }
    // …
}
```

Handlers that don't care about caller identity ignore the context argument.

### Dual exposure: in-process + external

The same `Arc<KanbanServer>` can power both the in-process registration
and an external transport without any duplication:

```rust
let kanban = Arc::new(KanbanServer::new(state.clone()));

// In-process for plugins and host UI code:
registry.register("kanban", Arc::new(InProcessServer::from_arc(kanban.clone())))?;

// Also serve over stdio for external CLI tools (dev / debugging / Claude Desktop):
tokio::spawn({
    let kanban = kanban.clone();
    async move { kanban.serve(stdio()).await }
});
```

State is shared via `Arc`. Internal callers and external clients see the
same server, the same tools, the same data, with no synchronization layer
between them. This is the difference between a plugin platform that
treats MCP as one transport among many and a plugin platform that treats
MCP as *the* shape of services.

### Why this is the only Rust path

There is intentionally no second way to write server-side Rust code in
this project. Everything goes through rmcp:

- Tool schemas come from `schemars::JsonSchema` derivation — same as any
  rmcp server.
- Handler logic is `#[tool]` async functions — same as any rmcp server.
- Error types are `rmcp::ErrorData` — same as any rmcp server.
- Capability negotiation is `ServerInfo` — same as any rmcp server.

This means every server-side skill, example, code generator, and bit of
ecosystem tooling for rmcp works on this codebase unchanged. The platform
adds a registration adapter; it does not add a parallel server framework.

## Dispatch

All calls — host-to-host, plugin-to-host, plugin-to-plugin, agent-to-host —
go through one dispatcher:

```rust
pub struct Dispatcher {
    registry: Arc<ServerRegistry>,
}

impl Dispatcher {
    pub async fn call(
        &self,
        caller: CallerId,
        server: &str,
        tool: &str,    // the MCP tool name
        input: Value,  // the arguments map — `op` is just a key when present
    ) -> Result<Value> {
        let server = self.registry.get(server).ok_or(Error::UnknownServer)?;
        server.invoke(caller, tool, input).await
    }
}
```

The dispatcher routes by `(server, tool)` and forwards one arguments
map. It is a plain MCP `tools/call`: there is no verb or noun axis in
the dispatch signature. For an operation tool the `op` selector is
already a key inside `input` — the SDK's path sugar put it there
before the call reached the dispatcher (see *Service Consumption*).
The platform never reads `input`. The `McpServer` trait impl decides
how the call is fulfilled:

- **Host-implemented Rust servers (rmcp)** receive the call as an rmcp
  `CallToolRequestParam` — the tool name and the arguments map,
  unmodified. The tool's own handler reads `op` from the arguments.
  No serialization, no IPC.
- **CLI-sourced servers** receive a JSON-RPC `tools/call` over the
  subprocess's stdin — same tool name, same arguments map.
- **URL-sourced servers** receive a JSON-RPC `tools/call` over HTTP —
  same tool name, same arguments map.
- **External MCP transport** (agents calling into the host) uses the
  identical shape: one tool name, one arguments map.

There is no platform-level noun/verb concept — operation structure
lives entirely in the tool's `_meta` (for discovery) and the `op`
argument (for invocation). Every transport carries a plain
`tools/call`.

`CallerId` distinguishes host-internal, plugin-id, and external-agent
callers and is propagated to handlers so they can audit, log, or apply
their own service-specific behaviors. The platform does not gate calls
on it.

## Callbacks

Functions can't cross the host/plugin boundary directly. The SDK
handles this with a single primitive used wherever a plugin hands the
host a function (command `available`/`execute`, view `render`, event
handlers, elicitation responses):

1. When dispatching a call whose input contains function values, the SDK
   assigns each a callback id and stores `{id → fn}` locally.
2. Functions are replaced in the outgoing payload with
   `{ "$callback": "cb_a3f1" }`.
3. The host treats markers as opaque handles. When something needs to
   invoke the callback, it sends `notifications/callbacks/invoke { id, args }`.
4. The SDK receives the notification, looks up the stored function, runs
   it, and (if a return value is expected) sends
   `notifications/callbacks/result`.

One primitive covers event subscriptions, command handlers (`available` /
`execute`), view renderers, async streaming results, and elicitation.
The [Command service](./command-service.md) shows the pattern in use.

Note that **MCP tool calls themselves do not use this primitive.** Tool
calls cross to URL-sourced servers via HTTP and to CLI-sourced servers
via stdio; both transports speak full MCP JSON-RPC, not the callback
notification protocol. The callback primitive is purely for plugin →
host function references, not server → tool dispatch.

## Codegen

The SDK is generic at runtime; it doesn't know which servers exist or
which tools are operation-based. Types come from a `.d.ts` file that the
host **maintains automatically** as the server registry changes. There
is no separate CLI to invoke and no build step plugin authors need to
remember.

### Mechanism

The host owns a `TypesEmitter` that subscribes to registry events:

- A server registers → emitter queries it via `tools/list`, regenerates.
- A server unregisters → emitter regenerates without it.
- A server fires `notifications/tools/list_changed` → emitter regenerates
  for that server.
- A plugin loads or unloads → flush boundary (any pending regen runs once).

Regeneration is debounced ~100ms so that a plugin registering many tools
during `load()` produces a single file write at the end of the load. The
file is written atomically (write-then-rename) so language servers never
see a half-written declaration file.

The output path is configurable; defaults to `.swissarmyhammer/types/app.d.ts`
inside the active plugin development directory.

### What gets emitted

For each registered server, one nested namespace on the `App`
interface. The emitter walks each tool's `Tool` definition from
`tools/list`:

- **Flat tool** (no `io.swissarmyhammer/operations` in `_meta`): one
  method named for the tool, with the tool's `inputSchema` →
  TypeScript input type.
- **Operation tool** (`_meta` carries the operations tree): walk the
  noun → verb → parameters tree. Emit `tool.<noun>.<verb>(input)` for
  every leaf, where the `input` type is built from that verb's
  `parameters` map. The structure of the emitted types mirrors the
  `_meta` structure exactly — noun namespace, verb method, parameter
  object.

```ts
// .swissarmyhammer/types/app.d.ts — written by the host, not by you
interface App {
  // Flat tool — no operations _meta:
  weather: {
    current(input: { city: string }): Promise<{ tempC: number; conditions: string }>;
    forecast(input: { city: string; days: number }): Promise<Forecast>;
  };

  // Operation tool — emitted from _meta["io.swissarmyhammer/operations"]:
  //   server "board", tool "kanban", nouns "task" / "board".
  board: {
    kanban: {
      task: {
        add(input: { title: string; description?: string }): Promise<Task>;
        move(input: { id: string; column: string }): Promise<Task>;
        list(input: {}): Promise<Task[]>;
      };
      board: {
        init(input: { name: string }): Promise<Board>;
      };
    };
  };

  /* … one entry per registered server … */
}

declare global {
  interface Plugin {
    // The dispatching proxy on `this` is typed as App at the SDK side.
    readonly [K in keyof App]: App[K];
  }
}
```

The emitter is pure metadata → types — it copies the `_meta` tree's
shape into the namespace shape, with each verb's `parameters` map
becoming the input object type. No schema analysis, no inference. The
same `_meta` tree feeds the CLI generator (`<bin> <noun> <verb>
--args`) and the runtime proxy.

### How plugin authors consume it

Plugin projects' `tsconfig.json` includes the generated path:

```jsonc
{
  "compilerOptions": { /* … */ },
  "include": ["src/**/*", ".swissarmyhammer/types/**/*.d.ts"]
}
```

That's the only setup. With the host running in dev mode, the IDE's TS
language server watches the file, picks up changes within a tick of the
debounced write, and autocomplete updates live. Installing a new
plugin that provides a server, hot-reloading a plugin that adds a new
tool, or removing a plugin all reflect in the editor without any user
action.

### Production vs development

Generated types are a development convenience; they're irrelevant at
runtime since the host runs TS directly. In production:
- Plugin authors can ship the `.d.ts` alongside their plugin's source
  as part of the published artifact, useful for downstream plugin
  authors who depend on consuming this plugin's surface during their
  own development.
- The host doesn't write a types file in production unless a dev mode
  flag is set.

### Stale types are safe

Generated types are decoupled from runtime. Out-of-date types degrade
to runtime errors with clean messages (`UnknownServer`, `UnknownTool`,
`UnknownOperation`), never crashes. The worst case from a stale
`.d.ts` is missing autocomplete or a spurious red squiggle — never a
corrupted call.

## Plugin Identity

A plugin is a directory of TypeScript. There is no `plugin.json`, no
manifest, no separate descriptor file. Everything the host needs to load
a plugin comes from the directory itself and from properties on the
plugin's exported `Plugin` subclass:

| Field         | Where it lives                                          |
| ------------- | ------------------------------------------------------- |
| `id`          | The directory name on disk (e.g. `weather/` → `weather`) |
| Entry module  | `index.ts` or `index.js` at the top of the directory    |
| `name`        | `readonly name` property on the `Plugin` subclass       |
| `version`     | `readonly version` property on the `Plugin` subclass    |
| `description` | `readonly description` property on the `Plugin` subclass |

```ts
// plugins/weather/index.ts
import { Plugin } from "@swissarmyhammer/plugin";

export default class WeatherPlugin extends Plugin {
  readonly name = "Weather";
  readonly version = "1.0.0";
  readonly description = "Current conditions and short-range forecast";

  async load() {
    this.register("weather", { url: "https://weather.example.com/mcp" });
  }
}
```

The directory name is the plugin's identity across layers — project,
user, and builtin copies of the same id stack the same way every other
stacked resource does. `index.ts` and `index.js` are the only entry
filenames the host looks for; whichever exists wins, `index.ts` first.
Plugins import their own internal modules by relative path from there;
no `entry` field tells the host where to start because the convention
already does.

`name`, `version`, and `description` are descriptive metadata for the
host UI and logs. They are not validated by the platform and not part of
the wire protocol — a plugin with no overrides inherits the base class
defaults (`"unnamed plugin"`, `"0.0.0"`, `""`) and still loads.

### What is intentionally absent

The set of MCP servers a plugin will register is not declared anywhere
ahead of time:

- **No `provides` list.** The host learns what a plugin registers when
  it calls `register(name, source)`. Collisions surface there as
  `ServerNameTaken`; there is no install-time check.
- **No upgrade-time re-approval prompt.** A plugin that expands its set
  of registrations on hot reload does so silently — the host has no
  prior declaration to diff against.
- **No declared set of *consumed* servers.** A plugin can call any
  registered server at runtime. A call fails when the named server
  isn't registered (`UnknownServer`) or the tool isn't on that server
  (`UnknownTool`); the SDK additionally raises `UnknownOperation` when
  a `noun.verb` path is not in the tool's operations `_meta`.

The platform queries each registered server via `tools/list` after
registration to discover its tools — that is the only place tool
metadata lives.

## Plugin Discovery

Plugins are stored on disk and discovered through `swissarmyhammer-directory` —
the same crate that finds skills, prompts, modes, and agents. Plugin
authors and users do not learn a new discovery model for plugins; the
stacking rules are the rules they already know from every other
user-editable resource in the project.

### Layout

A plugin is a directory containing `index.ts` (or `index.js`) and any
local modules the plugin imports. The directory name is the plugin's
id. Plugin directories live under the `plugins/` subdirectory of
whichever layer they ship in:

| Layer       | Path                                                 | Source              |
| ----------- | ---------------------------------------------------- | ------------------- |
| **builtin** | compiled into the host binary via `include_dir!`     | host build          |
| **user**    | `$XDG_CONFIG_HOME/kanban/plugins/<plugin-id>/`       | user-installed      |
| **project** | `<board_dir>/.kanban/plugins/<plugin-id>/`           | repo-checked-in     |

The directory namespace is the **embedder's**, not a fixed `sah` one. The
kanban app resolves its layers through `swissarmyhammer_directory::KanbanConfig`
(`XDG_NAME = "kanban"`, `DIR_NAME = ".kanban"`), so the user layer is
`~/.config/kanban/plugins/` and the project layer is the `.kanban/plugins/`
directory of the board's own folder. A different embedder (a future TUI or
headless host) supplies its own `DirectoryConfig` and so its own namespace —
the platform hardcodes none.

The host loads each layer with a `VirtualFileSystem` scoped to the `plugins/`
subdirectory. The directory *name* on disk is authoritative for identity
across layers — a `weather/` directory in the project layer shadows a
`weather/` directory in the user layer regardless of what their `Plugin`
subclasses set for `name`.

### Project layer is per board window

The project layer is **not global** — its root is the board the window is
showing, so each kanban board window discovers project plugins from *its own*
`<board_dir>/.kanban/plugins/`. Two windows open on two different boards see
two different project-layer plugin sets, each stacked over the shared user and
builtin layers. The builtin and user layers are process-wide; only the project
layer (and the registrations it produces) is scoped to the board window.

The kanban app is multi-window in a single process: `window.new` builds a new
`WebviewWindow` in-process, and a window can open, close, or switch the board
it shows at runtime. Today there is one process-wide `PluginHost` on
`AppState`, with one global server registry and one global command registry —
which cannot give each board window its own project-layer plugin set.

**Chosen model: one `PluginHost` per board window.** Each open board window
owns its own host — its own V8 isolate pool, its own `ServerRegistry`, its own
command registry and `CommandService` — rooted at the shared builtin and user
layers plus *that window's board* as the project layer
(`<board_dir>/.kanban/plugins/`). Full registry isolation falls out for free:
- a project plugin's commands and servers surface only in their own window,
  because each window's palette/menus read that window's host;
- two boards each shipping a `weather` plugin (or each overriding `task.move`)
  never collide, because they are different registries entirely;
- closing or switching a board tears down that window's host — isolates,
  registrations, watcher — without touching any other window.

The builtin bundles are extracted to a shared on-disk cache once at startup;
each per-window host discovers from that cache and the shared user `plugins/`
dir, so the *source* is shared even though each host loads its own isolates.
The cost is N× the V8 per-isolate floor for N open windows — accepted in
exchange for the simplest isolation model. `AppState` holds a map of
window → host instead of a single host; command dispatch resolves the host of
the calling `WebviewWindow`. Each per-window host still wires its command
backends to its board (the existing `tokio::task_local!` substrate seam is set
around that host's dispatch), so the data path is unchanged; only the host and
its registries become per-window. The hot-reload watcher runs per host: the
shared user `plugins/` dir plus that window's board `.kanban/plugins/`.

### Precedence

A plugin id stacks the same way every other resource stacks: project
shadows user shadows builtin. If `weather` exists in both user and
project, the project copy is the active one. Removing the project copy
causes the user copy to re-emerge, and so on down to builtin. This
means a project can override a user-installed plugin without
uninstalling it — exactly the pattern skills and prompts already use.

### File-change events

The host needs to react when a plugin's files change:

- A plugin directory appears in any layer → load it (or, if a
  higher-precedence copy already exists, refresh the override stack
  without changing the active layer).
- A plugin file is edited → reload that plugin (host tears down the
  isolate, walks the ledger, reloads; see *Hot Reload*).
- A plugin directory is removed → unload from its layer; if a lower
  layer still has it, reload from there.

**This requires a watcher that `swissarmyhammer-directory` does not
have today.** Discovery and stacking are point-in-time today;
`code-context` has the only filesystem watcher in the workspace, built
on `async-watcher`, and it is bespoke to its own use case.

The right answer is to add a generic stack-aware watcher to
`swissarmyhammer-directory`. Plugins, skills, prompts, modes, and
agents all stack through the same machinery, all are user-editable,
all benefit from hot reload, and duplicating the watcher in five
crates is wrong. Proposed API:

```rust
// swissarmyhammer-directory
pub enum LayerChange {
    Added    { layer: FileSource, path: PathBuf },
    Modified { layer: FileSource, path: PathBuf },
    Removed  { layer: FileSource, path: PathBuf },
}

pub struct StackedEvent {
    pub subdirectory: String,   // e.g. "plugins"
    pub name:         String,   // e.g. "weather"  (top-level entry in the subdirectory)
    pub change:       LayerChange,
}

pub struct Watcher<C: DirectoryConfig> { /* … */ }

impl<C: DirectoryConfig> Watcher<C> {
    pub fn watch(subdirectory: &str) -> Result<(Self, mpsc::Receiver<StackedEvent>)>;
}
```

The watcher fans out across every layer that exists for the config
(builtin is read-only, so not watched at runtime — its content is
fixed at build time). Events are *stack-aware*: the consumer learns
which layer changed and what name is affected, not which raw file path
underneath. Debouncing happens inside the watcher (reusing the
`async-watcher` pipeline `code-context` already uses), so a save that
touches several source files inside `plugins/weather/` produces one
event per affected name, not one per file.

The plugin host subscribes at startup and translates `StackedEvent` to
load/reload/unload decisions based on which layer is currently active
for each plugin id:

- `Added { layer }` → if `layer` becomes the highest-precedence layer
  for this id, load it.
- `Modified { layer }` → if `layer` is the active layer for this id,
  reload it.
- `Removed { layer }` → if `layer` was the active layer, fall back to
  the next layer (which may be nothing — unload).

The watcher does not interpret the contents of a plugin directory; it
only reports per-name layer events. The plugin host re-reads the
bundle's `index.ts`/`index.js` and any imported modules after each event.

### Relationship to the code-context watcher

`code-context` watches the user's source tree, not a stacked config
directory, so its scope is genuinely different and it stays where it
is. The underlying `async-watcher` plumbing (debounce, cancellation,
teardown) should become a shared internal helper in
`swissarmyhammer-directory` so the two watchers share that machinery
rather than maintaining parallel copies.

## Runtime: deno_core

All plugins run in **deno_core** (V8 + the deno_core embedding layer)
inside the host's main Rust process. The same runtime is used across all
host kinds (Tauri kanban app, TUI, headless). Plugin source, dispatch
path, and debugger story are identical.

deno_core is the workspace's **single JavaScript engine**. The
`swissarmyhammer-js` crate — today built on `rquickjs` (QuickJS-NG) and
used only for entity field-validation functions — is converted to
deno_core as part of this work, so field validation and plugins share
one engine, one async model, and one debugger story. See *Why
deno_core, not rquickjs* below.

**Per-plugin isolates.** Each plugin gets its own V8 isolate for fault
isolation and clean teardown.

**Costs:** ~15–25 MB binary growth from V8 + ~5–10 MB from the TS
toolchain; slower link step; per-isolate memory floor of a few MB.

**Benefits:** real Chrome DevTools via V8 Inspector (`--inspect[=PORT]`,
attach with `chrome://inspect`); identical semantics across hosts;
first-class async, ES2024+, web streams.

### TypeScript is built in

Plugins are written in TypeScript and ship as `.ts`. The host transpiles
to JS at module-load time — no build step required of plugin authors.
The host loads `index.ts` (or `index.js`) at the top of the plugin
directory and follows its imports from there:

```text
plugins/weather/
├── index.ts          # default-exports a class extending Plugin
└── lib/forecast.ts   # imported from index.ts by relative path
```

The transpiler is **`deno_ast`** (which wraps `swc_core`), the same
toolchain Deno itself uses. It produces JS + source maps; the source
maps are registered with the V8 Inspector so Chrome DevTools shows
original TS line numbers in stack traces and breakpoints. Hot reload
re-transpiles on each load.

This is a hard requirement, not an optional convenience. The codegen
story (`.d.ts` files written into the plugin's project) assumes plugin
authors write TS; making them set up a `tsc` or `esbuild` step just to
run that TS would defeat the point. The host carries the toolchain.

Type-checking is *not* run by the host. The transpiler does syntactic
TS-to-JS only — type errors aren't surfaced at load time. Plugin authors
get type-checking through their editor (the `.d.ts` file is for that
purpose) and through their own CI if they want it; the runtime treats TS
as JS-with-extra-syntax.

### Module loading

deno_core's `ModuleLoader` is wired up so multi-file plugins work
without bundling:

- **Relative imports** (`./util`, `../shared/foo`) — resolved against
  the plugin's bundle directory; each loaded module is transpiled the
  same way. Imports outside the plugin's directory are rejected.
- **Bare imports** (`lodash`, `zod`) — not resolved by the host. If a
  plugin uses npm packages, the author bundles them in (esbuild,
  rollup, `bun build`, whatever — that's their tool choice, not ours)
  and ships the bundled file as `entry`. The host is not an npm client.
- **`@swissarmyhammer/*` imports** — resolved to host-provided
  built-ins: the plugin SDK (`@swissarmyhammer/plugin`), generated app
  types (`@swissarmyhammer/app` → `.swissarmyhammer/types/app.d.ts`),
  etc. These are virtual modules served from memory.

### Why deno_core, not rquickjs

The workspace already embeds a JavaScript engine: `swissarmyhammer-js`
wraps `rquickjs` (QuickJS-NG) and runs the `validate` function bodies
on entity fields. The plugin platform could be built on rquickjs too —
QuickJS-NG supports ES classes, `Proxy`, ES modules, and async, so the
`Plugin` class model and the SDK Proxy would all work — and that would
avoid pulling V8 into the binary. We choose deno_core anyway, and
convert `swissarmyhammer-js` to match:

- **Plugin debugging.** deno_core exposes the V8 Inspector — real
  Chrome DevTools with breakpoints, source maps, and stepping
  (`--inspect`). QuickJS has no comparable inspector; plugin authors
  would be left with `console.log` and stack traces. For a surface
  authored by third parties, this is decisive.
- **One engine, not two.** Settling on deno_core means converting
  `swissarmyhammer-js`; the alternative — plugins on deno_core,
  validation on rquickjs — links both V8 and QuickJS and maintains two
  runtime models, two async stories, two debuggers. The conversion is
  cheap: `swissarmyhammer-js` has a single consumer
  (`swissarmyhammer-fields`), its public API (`JsState`, `set`/`get`)
  is preserved, and its dedicated-worker-thread + channel structure
  carries straight over to `deno_core::JsRuntime`.
- **TypeScript and tooling.** The `deno_core` + `deno_ast` pairing
  gives transpilation, source maps, and the module loader as one
  coherent toolchain (see *TypeScript is built in*). On rquickjs the
  host would assemble that itself.

The cost is real and accepted: ~15–25 MB of binary from V8, a slower
link, and a multi-MB per-isolate memory floor — paid even by hosts
that only run field validation and never load a plugin. QuickJS would
have been lighter. We trade that footprint for first-class plugin
debugging and a single engine across the workspace.

### Why not Bun

Bun bakes TS in and would save us the deno_ast piece, but as of writing
there's no shippable Rust embedding crate (Bun's own embed-in-native
issue is still open), Bun is mid-port from Zig to Rust and not stable,
and its runtime philosophy is "ship everything" (Node compat, npm,
bundler, sqlite, $) which is the opposite of what we want in a sandboxed
plugin host. The deno_core + deno_ast pairing gives us V8 (better
DevTools and broader tooling familiarity), per-isolate sandboxing as a
first-class concept, and a stable Rust embedding API today. Revisit if
Bun ships a real Rust embedding crate with first-class isolate support;
not a v1 concern.

## Hot Reload

Hot reload is always available; plugins don't opt in. The reload
*trigger* is the filesystem watcher described in [Plugin Discovery](#plugin-discovery):
the host subscribes to stack-aware events on the `plugins/` subdirectory
and decides per-event whether to load, reload, or unload. This section
covers what happens once that decision is made.

**Host owns the bookkeeping.** Every MCP call originating from a plugin
carries that plugin's id through the dispatcher. Calls that create
long-lived state — `this.register` (a server), `register` tools that
particular servers expose (commands, views, etc., via callbacks) — are
recorded in a per-plugin ledger. On unload, the host walks the ledger
and disposes every entry without the plugin's cooperation.

This means:
- `this.track(disposable)` is a convenience for mid-session cleanup, not
  a requirement for lifecycle cleanup.
- `unload()` is optional; it exists only for side effects outside the
  plugin runtime (signaling remote services, releasing external handles).
- Sloppy plugins still clean up correctly.

**Reload mechanics.** Host tears down the plugin's V8 isolate; walks the
ledger; disposes registrations; creates a fresh isolate; loads new source;
calls `load()`. Total latency on the order of tens of ms.

**Edge cases:**
1. **In-flight operations terminate abruptly.** Isolate is killed; calls
   reject with `PluginReloaded`.
2. **Registration set may change silently.** Nothing declares ahead of
   time which servers a plugin registers, so a v2 that registers more
   (or different) servers than v1 just does so when its `load()` runs.
   Conflicts surface as `ServerNameTaken` from the new registrations.
3. **Failed v2 load leaves the plugin unloaded.** No fallback to v1; v1
   is already torn down by the time v2 is attempted. Manual retry.
4. **Crashed plugins do not auto-restart.** The platform records
   [`ReloadStatus::Crashed { error }`](crate::ReloadStatus::Crashed) and
   exposes it through `PluginHost::reload_status(plugin_id)`. "No
   auto-restart" is structural: the watcher only fires on file changes,
   and a crash is not a file change. Host applications (settings UI, a
   TUI badge, a notification system) consume the status and surface it
   to the user, who then triggers a manual reload by touching the bundle
   on disk or calling `PluginHost::load` directly.
5. **Plugin state in class fields is lost on reload.** Intended.

**Dev-mode niceties:** the `swissarmyhammer-directory` watcher triggers
auto-reload on every save; reload commands exposed; V8 Inspector
connection survives reloads.

## Testing

Every advertised capability of the plugin platform needs at least one
integration test that exercises the full pipeline — real V8 isolate,
real TS transpile, real on-disk plugin bundle, real dispatcher, real
registered server. Fixture-only tests that mock the dispatcher or
hand-construct the registry prove math, not features; the same
principle that governs the rest of this workspace
(see `swissarmyhammer-tools/tests/integration/semantic_search_e2e.rs`
as the reference pattern). A mocked plugin host that "passes" tells
you nothing about whether a real plugin will load.

### Isolation model

Every test gets its own world. Nothing shared, nothing global,
parallel-safe by construction.

| Resource              | Per-test isolation                                      |
| --------------------- | ------------------------------------------------------- |
| Plugin source on disk | `tempfile::TempDir` as a project-layer plugins root     |
| Working directory     | The test's `TempDir` — plugins inherit it via the host  |
| V8 isolates           | Fresh `PluginHost` per test — no `static` host          |
| Server registry       | Fresh `ServerRegistry` per test — no shared singletons  |
| Filesystem state      | All paths the plugin touches live under the temp dir    |
| Watcher               | Watcher scoped to the temp `plugins/` dir; dropped with the test |

A test that owns its temp dir already has a perfectly good isolated
cwd — the plugin running inside the host inherits it through
`Deno.cwd()` and can write relative paths into the same temp dir
without any platform-side cwd plumbing. Tests that need `HOME`/`XDG_*`
isolation (for example, to exercise the user-layer of plugin discovery)
can compose `swissarmyhammer-common::IsolatedTestEnvironment` on top
per the workspace's CWD-isolation rule, but the reference dispatch
tests don't need it.

The plugin host should expose a `PluginHost::for_tests()` constructor
that takes explicit roots (project plugins dir, optional user/builtin
roots) rather than reading global config.

### Reference integration test: the `files` MCP server

The `files` MCP server (`swissarmyhammer-tools/src/mcp/tools/files`)
is the right target for a first integration test because it is real
(in-process rmcp), it has observable state (the filesystem), and
verification reduces to "did the files land on disk where we expect."

In prose:

1. The test creates a `TempDir` and runs from it (the host inherits
   the cwd). It builds a `PluginHost::for_tests` pointing at
   `<tempdir>/plugins/` as the project plugin root.
2. The host wraps the real `FilesTool` in an `InProcessServer` and
   registers it under a known name. No mocks.
3. The test writes a small probe plugin to `<tempdir>/plugins/probe/`
   — a real `index.ts` whose default export extends `Plugin`. The
   plugin's `load()` does three things, using only the registered files
   server: write a probe file in cwd, read it back, then write the
   readback content into a second probe file. (Reporting through a
   second written file means the test never needs a special host-side
   reporter hook — observation is the filesystem, the same as for any
   other test.)
4. The test triggers discovery. The host transpiles the plugin,
   creates a fresh isolate, and runs `load()`.
5. The test asserts both probe files exist in the temp dir with their
   expected contents. The first file proves the `op` dispatch reached
   the real `files` tool handler. The second file proves the return
   value crossed back through the dispatcher into the isolate. If any
   stage of the pipeline is broken, at least one assertion fails.

This test exercises the whole pipeline — bundle discovery, TS transpile,
isolate creation, server lookup, operation-tool `op` dispatch,
return-value marshalling — using only platform primitives. No `cwd`
field on the Plugin base class, no test-only reporter hook, no fakes.

### What each kind of test must exercise

One reference integration test per capability the platform advertises.
Every test follows the same shape: real isolate, real registered
server, observe an effect that only happens if the platform works.

| Capability                       | What the test proves                                       |
| -------------------------------- | ---------------------------------------------------------- |
| **In-process server dispatch**   | A plugin call into the real `files` operation tool writes a real file (the case above) |
| **CLI subprocess server**        | Plugin registers `{ cli: [...] }`; host spawns it; calls go through stdio and return |
| **URL server**                   | Plugin registers `{ url: ... }`; host calls it; mock HTTP endpoint records the request shape |
| **Operation `_meta` round-trip** | An operation tool's `io.swissarmyhammer/operations` `_meta` tree is read by the SDK; `this.<server>.<tool>.<noun>.<verb>({...})` reaches the tool as `tools/call("<tool>", { op: "<verb> <noun>", ... })` |
| **Callback round-trip**          | Plugin passes a function; host invokes it; return value flows back to where it's awaited |
| **Plugin discovery & layering**  | Same plugin id in user + project layers: project wins; remove project, user re-emerges |
| **Hot reload**                   | Write source, observe behavior; rewrite source, watcher fires, observe new behavior in same `PluginHost` |
| **Unload disposal**              | Unload plugin; its registered server fails with `ServerUnavailable`; its callbacks no longer fire |
| **Override stack** (Command svc) | Two plugins register the same command id; second wins; second unloads; first re-emerges |
| **Failed load**                  | Plugin throws in `load()`; host surfaces the error; no zombie isolate, no half-registered servers |

### What not to do

- **Don't write fixture-only tests** that hand-build a `ServerRegistry`
  and call `Dispatcher::call` directly. They skip the V8 boundary —
  the part most likely to break — and pass even when the real loader
  is broken. This is the same anti-pattern the workspace's
  fixture-only-anti-pattern rule already captures for code-context:
  raw-state-insert tests prove math, not features.
- **Don't mock registered servers** when a real one is available.
  In-process rmcp servers are cheap; use them. Reserve mocks for
  things that genuinely can't run in-process (external HTTP services
  with no test double).
- **Don't share state between tests.** No `static` `PluginHost`, no
  reused temp dirs, no leaking plugins from one test into the next.
  Hot reload tests in particular need a fresh host because reload
  behavior depends on the previous load's ledger.
- **Don't assert on intermediate state** (the registry has entry X,
  the ledger has entry Y). Assert on observable effects (file
  written, callback fired, plugin's reported value matches).

### Test layout in the workspace

Plugin platform integration tests live in
`swissarmyhammer-plugin/tests/integration/`, with one file per
capability. The `files` reference test is `files_dispatch_e2e.rs`.
The naming convention matches the existing `*_e2e.rs` pattern already
used by the code-context and skill suites.

## Implementation Notes

### Server registry

```rust
pub struct ServerRegistry {
    servers: HashMap<ServerName, Arc<dyn McpServer>>,
}

impl ServerRegistry {
    pub fn register(&mut self, name: ServerName, server: Arc<dyn McpServer>) -> Result<()> {
        match self.servers.entry(name) {
            Entry::Vacant(e) => { e.insert(server); Ok(()) }
            Entry::Occupied(e) => Err(Error::ServerNameTaken(e.key().clone())),
        }
    }
    pub fn unregister(&mut self, name: &str) -> Option<Arc<dyn McpServer>> {
        self.servers.remove(name)
    }
}
```

The host's startup configuration registers whatever servers the host
build includes. Plugins register additional servers by source (URL or
CLI). The platform owns a transport per source kind; the registry holds
a uniform `Arc<dyn McpServer>` regardless of where the server actually
runs.

### `McpServer` trait

```rust
#[async_trait]
pub trait McpServer: Send + Sync {
    fn tools(&self) -> Vec<ToolMetadata>;
    async fn invoke(
        &self,
        caller: CallerId,
        tool: &str,    // the MCP tool name
        input: Value,  // the arguments map, passed through untouched
    ) -> Result<Value>;
}
```

`invoke` is a plain `tools/call`: a tool name and an arguments map. The
platform does not read `input`; for an operation tool the `op` selector
is one of its keys, placed there by the SDK, and parsed only by the
tool's own handler.

`ToolMetadata` is the tool's `Tool` definition from `tools/list` —
`name`, `description`, `inputSchema`, and `_meta`. An operation tool
carries the noun → verb → parameters tree under
`_meta["io.swissarmyhammer/operations"]`; the codegen, CLI generator,
and SDK path sugar all read it from there.

Three production implementations, one per source kind:

- **`InProcessServer<S: rmcp::ServerHandler>`** — wraps an rmcp handler.
  `invoke` forwards the tool name and arguments map to `S::call_tool`
  directly. No serialization, no IPC. This is the path for host Rust
  code (see *In-Process Host Servers*).
- **`CliServer`** — wraps a spawned subprocess. `invoke` sends a JSON-RPC
  `tools/call` over the subprocess's stdin and awaits the response on
  stdout. The platform manages the subprocess lifecycle (spawn on
  register, kill on unregister, restart on crash if configured).
- **`UrlServer`** — wraps an HTTP MCP transport. `invoke` sends a
  JSON-RPC `tools/call` to the configured URL and awaits the response.
  Authentication headers from the registration are reused on every
  call.

Each backend implements `tools()` by caching the server's `tools/list`
response at connection time, refreshed on
`notifications/tools/list_changed`.

The dispatcher routes to any backend uniformly. Adding a fourth — a
WebSocket MCP transport, an in-process Python interpreter, whatever — is
a fourth `McpServer` impl; nothing else in the platform changes.

### Per-plugin ledger

```rust
pub struct PluginLedger {
    by_plugin: HashMap<PluginId, Vec<RegistrationHandle>>,
}

pub enum RegistrationHandle {
    Server(ServerName),
    Callback(CallbackId),
    // Service-specific handles (commands, views, etc.) are owned by their
    // services, but the ledger holds opaque dispose-fns for them so the
    // platform doesn't need to know the handle types.
    Opaque(Box<dyn FnOnce() + Send>),
}
```

Every long-lived registration appends to the calling plugin's vec. On
unload, the vec is drained in reverse and each handle is disposed. Services
that maintain their own registries (Commands, etc.) hand the platform an
`Opaque` dispose-fn at registration time; the service's internal cleanup
runs when the platform calls the fn.
