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
   dispatcher know nothing about specific server, tool, or verb names.
   Two dispatch shapes are supported, distinguished by whether the server
   declared an `experimental.operations` capability at initialize time
   (per [MCP `ServerCapabilities`](https://modelcontextprotocol.io/specification/2025-11-25/schema#servercapabilities)):
   - **Flat server:** `this.foo.bar(args)` → `call(server: "foo", tool: "bar", args)`.
   - **Operation server:** `this.foo.<noun>.<verb>(args)` → tool name is the
     noun class, verb identifies the operation, args carry every parameter
     including any instance id. The platform folds `verb` into the args
     map at the MCP wire boundary, since MCP itself has no verb concept.
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

  // Dynamic dispatch for any registered server. Shape depends on whether
  // the server declared the operations capability at initialize time:
  //   Flat server:       this.foo.bar(input)
  //   Operation server:  this.foo.<noun>.<verb>(input)   // input includes any id
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
come in two shapes — flat and operation-based — and the SDK supports both
through the same generic Proxy. Which shape applies to a given tool is
determined by the tool's schema, never by anything baked into the SDK.

### Flat tools

One MCP tool per operation. TS surface is `this.<server>.<tool>(args)`.
Args is always a single object.

```ts
// Server "weather" exposes flat tools "current" and "forecast":
const now    = await this.weather.current({ city: "Austin" });
const next3  = await this.weather.forecast({ city: "Austin", days: 3 });
```

Dispatch: `call(server: "weather", tool: "current", args: { city: "Austin" })`.
Wire (MCP): `tools/call("weather", "current", { city: "Austin" })`.

### Operation-based tools

One MCP tool per noun class; verbs are part of the call. The TS surface
is `this.<server>.<noun>.<verb>(args)` where `args` is a single object
carrying every parameter the verb takes — including any instance
identifier:

```ts
// Server "entities" exposes an operation-based tool "cards":

// Verbs that operate on the collection take no id:
const card  = await this.entities.cards.create({ data: { title: "..." } });
const all   = await this.entities.cards.list({});

// Verbs that operate on a specific instance take id as a normal arg:
const got   = await this.entities.cards.read({ id: "card_123" });
await         this.entities.cards.update({ id: "card_123", data: { title: "new" } });
await         this.entities.cards.delete({ id: "card_123" });

// Instance ids are just string values; characters like dots, hyphens,
// and slashes need no special treatment because the id never appears in
// the JS path:
await         this.entities.cards.update({ id: "card.with.dots", data: {...} });
```

Dispatch:
`call(server: "entities", tool: "cards", verb: "update", args: { id: "card_123", data: {...} })`.

Wire (MCP): `tools/call("cards", { verb: "update", id: "card_123", data: {...} })`.
The verb rides in the arguments map, since MCP's wire format has no
verb concept. The SDK rolls/unrolls it at the boundary. Every other
parameter — including any instance id — is just a normal property in
the arguments map.

Why operation-based: schema sharing (all verbs on `cards` share the
Card shape), discoverability (operations grouped by noun), and
structured metadata for clean codegen and CLI generation — the
`(verb, noun, args)` triple maps directly to `<bin> <noun> <verb> --args`
on the command line and to `this.server.<noun>.<verb>(args)` in
TypeScript with no presentation-layer rewriting. The
[Command service](./command-service.md) is a worked example built on
top of the platform.

### How the SDK knows which shape to use

The platform reads the server's `ServerCapabilities` from the MCP
`initialize` handshake. A server declares operation-mode by including
the experimental capability:

```json
{
  "capabilities": {
    "experimental": {
      "io.swissarmyhammer/operations": { "version": "1" }
    }
  }
}
```

Per the [MCP 2025-11-25 schema](https://modelcontextprotocol.io/specification/2025-11-25/schema#servercapabilities),
`ServerCapabilities.experimental` is an open `{ [key: string]: object }`
map for non-standard capabilities. This is the standard hook for
declaring server-side extensions.

For operation servers, each operation tool reports its **structured verb
metadata** in the tool's `_meta` field on the `tools/list` response:

```jsonc
{
  "tools": [
    {
      "name": "command",
      "description": "Command registry operations",
      "inputSchema": { "type": "object" },          // generic; the verbs are the real schemas
      "_meta": {
        "io.swissarmyhammer/operations": {
          "verbs": {
            "register":   { "input": { /* JSON schema */ }, "output": { /* … */ }, "description": "..." },
            "list":       { "input": { /* … */ },           "output": { /* … */ }, "description": "..." },
            "execute":    { "input": { /* … */ },           "output": { /* … */ }, "description": "..." },
            "available":  { "input": { /* … */ },           "output": { /* … */ }, "description": "..." },
            "unregister": { "input": { /* … */ },           "output": { /* … */ }, "description": "..." },
            "schema":     { "input": { /* … */ },           "output": { /* … */ }, "description": "..." }
          }
        }
      }
    }
  ]
}
```

The metadata is explicit and structured:
- The **tool name** is the noun class (`command`, `cards`, `task`).
- `verbs` — the verb set this tool supports.
- Each verb has `input` and `output` JSON schemas derived from the Rust
  method signature, plus a `description` from the doc comment.

Verbs that operate on a specific instance carry the instance identifier
as a normal required property in their `input` schema (commonly `id`).
There is no separate scope axis, no path-vs-args distinction at the
metadata level — every parameter is just a property.

The metadata is a faithful reflection of the `(verb, noun, args)` model
the operation infrastructure already uses (see
`swissarmyhammer-operations/src/schema.rs` and the CLI generator in
`kanban-cli/src/cli_gen.rs`). The same triple drives the SDK proxy,
the wire format, the codegen, and the CLI surface.

The SDK Proxy on `this.<server>...` uses this cached metadata directly:

- **Operations not declared:** flat dispatch. `path[0]` is the tool
  name. `this.weather.current(...)` → tool=`current`, args=`{...}`.
- **Operations declared:** path is exactly two segments —
  `<noun>.<verb>`. `this.entities.cards.update(...)` →
  tool=`cards`, verb=`update`, args carry every parameter including
  any instance id.
- **Verb not in the tool's metadata:** runtime error
  (`UnknownVerb`), with the valid verb set pulled from `_meta` so the
  message can list alternatives.
- **More than two segments under an operation server:** runtime
  error (`UnexpectedPath`). The metadata defines no path sugar.

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

`transport.callPath(server, path, args)` consults the cached metadata
for the path's tool (if any):
- flat tool (no operation metadata): `path[0]` is the tool name,
  dispatch `(server, tool, args)`.
- operation tool: `path[0]` is the noun-class tool name, `path[1]` is
  the verb (validated against the cached verb list); dispatch
  `(server, tool, verb, args)`. Any instance id is already a property
  in `args`.

`RESERVED` covers SDK-handled verbs (`on`, `off`, `once`, `subscribe`,
`unsubscribe`, `schema`) rather than forwarding as tool or verb names.

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

The host's startup code is what makes Rust modules available:

```rust
// host startup
host.expose_rust_module("kanban_core", KanbanServer::new(state.clone()));
host.expose_rust_module("entities",    EntitiesServer::new(state.clone()));
// …
```

`expose_rust_module` registers the rmcp handler in a separate "available
modules" table; it doesn't put the server into the live registry. A
subsequent `register(name, { rust: id })` call (from the host itself or
from a plugin) activates it under the chosen name.

This decouples *which Rust code the host has compiled in* from *which
servers are live and under what names*. The host can ship a library of
Rust modules; plugins (or the host's own config) choose which to expose,
under what names, and to whom. See *In-Process Host Servers (rmcp)*
below for how the Rust modules themselves are written.

This is the only source that bypasses serialization. URL and CLI sources
always pay one MCP-protocol round-trip per call.

### Schemas and capabilities come from `initialize` + `tools/list`. Always.

For every source, the platform's flow is identical:

1. Establish connection (HTTP request, spawn subprocess, or attach to
   in-process handler).
2. Send MCP `initialize`. The server returns `ServerCapabilities` in
   its response, including any `experimental` extensions.
3. Send `tools/list`. The server returns its tool metadata with full
   input/output schemas.
4. Cache both the capability set and the tool list. Subscribe to
   `notifications/tools/list_changed` so the tool-list cache stays
   current. Capabilities are fixed for the lifetime of the connection.
5. Make the server available as `this.<name>` on plugin `this`.

Operation-based servers are identified by the
`experimental.operations` capability returned at step 2. Per the
[MCP 2025-11-25 schema](https://modelcontextprotocol.io/specification/2025-11-25/schema#servercapabilities),
`ServerCapabilities.experimental` is an open `{ [key: string]: object }`
map for non-standard capabilities — this is the standard hook for an
extension like operation-mode dispatch.

A server that declares operations gets `<noun>.<verb>(args)` ergonomics
in the SDK Proxy and the emitted types — the tool name is the noun
class. A server without the capability gets `<tool>(args)` (flat). The
plugin author never declares anything; the right shape comes from the
cached capability set.

### Name collisions

The server registry has a single global namespace. The first registration
of a name wins; subsequent attempts fail with `ServerNameTaken`. The
host reserves the names it registers at startup. Plugins declare what
they intend to register in the manifest's `provides`, so collisions are
typically caught at install time.

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
        verb: Option<&str>,
        mut input: Value,
    ) -> Result<Value> {
        // Operation servers: fold `verb` into the args map so the rmcp
        // handler sees it as a normal input field. Every other parameter
        // — including any instance id — is already a property of `input`.
        // The wire shape is the same for in-process, stdio, and HTTP.
        if let (Some(v), Some(obj)) = (verb, input.as_object_mut()) {
            obj.insert("verb".into(), Value::String(v.into()));
        }
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

No transport, no JSON-RPC. Calls to `this.kanban.cards.update({ id: "card-123", ... })`
from a plugin land in the host's V8 dispatch path → `Dispatcher::call` →
`InProcessServer::invoke` → `rmcp::ServerHandler::call_tool` → the Rust
function body. The whole thing is async function composition.

### Declaring operation-mode

Operation servers don't write a manual dispatcher that switches on
`(verb, noun)`. The `#[operation_server]` and `#[verb]` attribute macros
generate the wire dispatch, the experimental capability declaration, the
tool's `_meta` metadata, and the JSON schemas — all from the method
signatures.

The example below sketches the registry server for the
[Command service](./command-service.md) (specified in detail in that
file) to illustrate the macro mechanics:

```rust
use swissarmyhammer_rmcp::{operation_server, verb};
use schemars::JsonSchema;

#[derive(Clone)]
pub struct CommandsServer {
    state: Arc<CommandRegistry>,
}

#[derive(Deserialize, JsonSchema)] pub struct RegisterArgs    { /* full command definition */ }
#[derive(Serialize,   JsonSchema)] pub struct RegisterOutput  { /* … */ }
#[derive(Deserialize, JsonSchema)] pub struct ListArgs        { /* optional filters */ }
#[derive(Deserialize, JsonSchema)] pub struct ExecuteArgs     { id: String, ctx: Context, force: Option<bool> }
#[derive(Serialize,   JsonSchema)] pub struct ExecuteOutput   { /* … */ }
#[derive(Deserialize, JsonSchema)] pub struct AvailableArgs   { id: String, ctx: Context }
#[derive(Serialize,   JsonSchema)] pub struct AvailableOutput { /* … */ }
#[derive(Deserialize, JsonSchema)] pub struct UnregisterArgs  { id: String }
#[derive(Deserialize, JsonSchema)] pub struct SchemaArgs      { id: String }

#[operation_server(noun = "command")]
impl CommandsServer {
    pub fn new(state: Arc<CommandRegistry>) -> Self { Self { state } }

    /// Register a new command in the registry.
    #[verb]
    async fn register(&self, args: RegisterArgs) -> Result<RegisterOutput, ErrorData> {
        self.state.register(args).await
    }

    /// List all active commands.
    #[verb]
    async fn list(&self, args: ListArgs) -> Result<Vec<CommandSummary>, ErrorData> {
        self.state.list(args).await
    }

    /// Execute the command identified by `id` in the given context.
    #[verb]
    async fn execute(&self, args: ExecuteArgs) -> Result<ExecuteOutput, ErrorData> {
        self.state.execute(&args.id, args.ctx, args.force).await
    }

    /// Check whether the command can run in the given context.
    #[verb]
    async fn available(&self, args: AvailableArgs) -> Result<AvailableOutput, ErrorData> {
        self.state.available(&args.id, args.ctx).await
    }

    /// Remove a command from the registry.
    #[verb]
    async fn unregister(&self, args: UnregisterArgs) -> Result<(), ErrorData> {
        self.state.unregister(&args.id).await
    }

    /// Return the input schema for a command.
    #[verb]
    async fn schema(&self, args: SchemaArgs) -> Result<Value, ErrorData> {
        self.state.schema(&args.id).await
    }
}
```

Every verb takes one `args` struct; if the verb operates on a specific
instance, the struct includes an `id: String` field like any other
parameter. There is no separate id-in-path channel and no `collection`
vs `on_noun` distinction — the args struct is the entire input.

What the macros emit:

1. **`rmcp::ServerHandler` impl** with `get_info()` declaring
   `experimental.operations` and `tools/list` returning one Tool whose
   name is the noun class (`command`) with structured `_meta`.
2. **The Tool's `_meta["io.swissarmyhammer/operations"]`** populated
   from the attributes:
   ```jsonc
   {
     "verbs": {
       "register":   { "input": <RegisterArgs schema>,   "output": <RegisterOutput schema>,   "description": "Register a new command in the registry." },
       "list":       { "input": <ListArgs schema>,       "output": <[CommandSummary] schema>, "description": "List all active commands." },
       "execute":    { "input": <ExecuteArgs schema>,    "output": <ExecuteOutput schema>,    "description": "Execute the command identified by `id` in the given context." },
       "available":  { "input": <AvailableArgs schema>,  "output": <AvailableOutput schema>,  "description": "Check whether the command can run in the given context." },
       "unregister": { "input": <UnregisterArgs schema>, "output": {},                        "description": "Remove a command from the registry." },
       "schema":     { "input": <SchemaArgs schema>,     "output": <Value schema>,            "description": "Return the input schema for a command." }
     }
   }
   ```
   Doc comments become the verb descriptions; the args struct's schema
   becomes the per-verb `input` (with `id` showing up as a required
   property when the verb operates on a specific instance); return
   types become output schemas via `schemars::JsonSchema`.
3. **A `call_tool` dispatcher** that:
   - reads `verb` from the wire args,
   - validates it against the metadata,
   - deserializes the remainder of `arguments` into the matching
     verb's args struct,
   - calls the matching method and serializes the result.

This is the same attribute-driven metadata approach already used for
the CLI surface in the workspace — derive structured information once
from the source of truth (the method signatures + attributes), use it
everywhere it's needed.

### Why structured metadata, not discriminated unions

An earlier draft of this design had operation tools declare one giant
JSON schema discriminated union on `verb`, with codegen splitting the
union to recover per-verb shapes. That's strictly worse:

- The server's source of truth becomes a union type spanning multiple
  verbs, instead of cleanly separated method signatures.
- Codegen has to perform schema archaeology to recover what the server
  already knows.
- Generic MCP clients see a confusing union and can't tell which
  parameters belong to which verb from the schema alone.
- Adding a verb requires editing the union; with metadata, you just
  add a new `#[verb]` method.

Structured per-verb metadata in `_meta` is the right shape. Each verb is
its own typed entity from end to end: Rust signature → JSON schema →
TypeScript method type. No reconstruction.

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
        tool: &str,           // for operation servers, this is the noun class
        verb: Option<&str>,   // None for flat tools, Some for operation servers
        input: Value,         // all parameters including any instance id
    ) -> Result<Value> {
        let server = self.registry.get(server).ok_or(Error::UnknownServer)?;
        server.invoke(caller, tool, verb, input).await
    }
}
```

The dispatcher routes by `(server, tool)`. For operation servers (those
that declared the `experimental.operations` capability at initialize),
`tool` is the noun-class name and `verb` identifies the operation.
Every other parameter — including any instance id — rides in `input`.
The `McpServer` trait impl decides how the call is fulfilled:

- **Host-implemented Rust servers (rmcp)** receive `verb` folded into
  the rmcp `CallToolRequestParam.arguments` map under the `verb` key,
  since MCP's wire format has no verb concept. The rmcp handler reads
  it from its typed input and dispatches internally. No serialization,
  no IPC.
- **CLI-sourced servers** receive the call as a JSON-RPC `tools/call`
  over stdin to the subprocess, with `verb` folded into the arguments
  map the same way.
- **URL-sourced servers** receive the call as a JSON-RPC `tools/call`
  over HTTP, with `verb` folded into the arguments map the same way.
- **External MCP transport** (agents calling into the host) sees the
  same wire shape going the other direction: one tool name (the noun
  class), one arguments map with `verb` as a regular field alongside
  every other parameter.

`verb` is a first-class dispatch axis in our Rust types and our SDK
ergonomics; it's a JSON field on the wire. Both sides agree on
operation semantics through the experimental capability declaration —
they know to look for the `verb` field when the capability is present.

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

For each registered server, one nested namespace on the `App` interface.
The emitter inspects the cached `ServerCapabilities` to choose shape:

- **Flat server** (no `experimental.operations` capability): for each
  tool in `tools/list`, one method with that tool's `inputSchema` →
  TypeScript input type.
- **Operation server** (capability declared): for each tool, read its
  `_meta["io.swissarmyhammer/operations"].verbs` map. Each verb entry
  has `input`, `output`, `description` — emit one method per verb
  under the tool namespace, using `input`/`output` directly as the TS
  types. Tool name = noun class; method name = verb; the `input` type
  carries every parameter the verb takes (including any `id`).

```ts
// .swissarmyhammer/types/app.d.ts — written by the host, not by you
interface App {
  // Flat server (no experimental.operations capability):
  weather: {
    current(input: { city: string }): Promise<{ tempC: number; conditions: string }>;
    forecast(input: { city: string; days: number }): Promise<Forecast>;
  };

  // Operation server — tool name is the noun class; every verb is a
  // method on it. Verbs that operate on a specific instance take `id`
  // as a normal property of their input type.
  commands: {
    command: {
      register(input: RegisterArgs): Promise<RegisterOutput>;
      list(input: ListArgs): Promise<CommandSummary[]>;
      execute(input: ExecuteArgs): Promise<ExecuteOutput>;     // ExecuteArgs.id: string
      available(input: AvailableArgs): Promise<AvailableOutput>; // AvailableArgs.id: string
      unregister(input: UnregisterArgs): Promise<void>;        // UnregisterArgs.id: string
      schema(input: SchemaArgs): Promise<unknown>;             // SchemaArgs.id: string
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

Every method takes one args object and returns a promise of the verb's
output type. The emitter is pure metadata → types — no schema
analysis, no inference, no special handling for instance ids. The same
metadata feeds the CLI generator (`<bin> <noun> <verb> --args`) and
the runtime proxy.

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
`UnknownVerb`), never crashes. The worst case from a stale `.d.ts` is
missing autocomplete or a spurious red squiggle — never a corrupted
call.

## Manifest

```jsonc
{
  "id": "weather-plugin",
  "name": "Weather",
  "version": "1.0.0",
  "entry": "src/plugin.ts",

  // Server names this plugin registers. The server itself declares its
  // tools via the MCP SDK; we don't repeat them here.
  "provides": ["weather"]
}
```

`provides` is the set of server names this plugin will register at
load time. The host validates at install time that names don't collide
with reserved host servers, and at runtime that `this.register(server, source)`
doesn't try to register a name not listed in `provides`. The platform
queries each provided server via `tools/list` after registration to
discover its tools — that's the only place tool metadata lives.

The plugin's set of *consumed* servers is not declared in the manifest.
A plugin can call any registered server at runtime. The dispatcher only
fails calls when the named server isn't registered (`UnknownServer`),
the tool isn't on that server (`UnknownTool`), or the verb isn't in
that tool's `_meta` (`UnknownVerb`).

`provides` expansions trigger a re-approval prompt on upgrade so users
can see what new servers a plugin will add to the registry.

## Plugin Discovery

Plugins are stored on disk and discovered through `swissarmyhammer-directory` —
the same crate that finds skills, prompts, modes, and agents. Plugin
authors and users do not learn a new discovery model for plugins; the
stacking rules are the rules they already know from every other
user-editable resource in the project.

### Layout

A plugin is a directory containing the manifest (`plugin.json`), the
entry TypeScript file (named in the manifest's `entry`), and any local
modules the plugin imports. Plugin directories live under the
`plugins/` subdirectory of whichever layer they ship in:

| Layer       | Path                                                 | Source              |
| ----------- | ---------------------------------------------------- | ------------------- |
| **builtin** | compiled into the host binary via `include_dir!`     | host build          |
| **user**    | `$XDG_CONFIG_HOME/sah/plugins/<plugin-id>/`          | user-installed      |
| **project** | `<git_root>/.sah/plugins/<plugin-id>/`               | repo-checked-in     |

The host loads them with `ManagedDirectory<SwissarmyhammerConfig>` and a
`VirtualFileSystem` scoped to the `plugins/` subdirectory. The directory
*name* on disk does not need to match the plugin id; the manifest's
`id` field is authoritative for identity across layers.

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
touches the manifest and several source files inside `plugins/weather/`
produces one event per affected name, not one per file.

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
manifest and source after each event.

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
Manifest's `entry` points directly at a `.ts` file:

```jsonc
{
  "name": "Weather",
  "entry": "src/plugin.ts",
  "provides": ["weather"]
}
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
2. **`provides` expansions require re-approval.** Hot reload pauses at
   load if v2's manifest expands the set of servers the plugin registers;
   user decides.
3. **Failed v2 load leaves the plugin unloaded.** No fallback to v1; v1
   is already torn down by the time v2 is attempted. Manual retry.
4. **Crashed plugins do not auto-restart.** Surfaced via notification and
   settings UI badge; user-initiated reload.
5. **Plugin state in class fields is lost on reload.** Intended.

**Dev-mode niceties:** the `swissarmyhammer-directory` watcher triggers
auto-reload on every save; reload commands exposed; V8 Inspector
connection survives reloads.

## Testing

Every advertised capability of the plugin platform needs at least one
integration test that exercises the full pipeline — real V8 isolate,
real TS transpile, real manifest load, real dispatcher, real
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
   — a real `plugin.json` and a real entry `.ts` file. The plugin's
   `load()` does three things, using only the registered files
   server: write a probe file in cwd, read it back, then write the
   readback content into a second probe file. (Reporting through a
   second written file means the test never needs a special host-side
   reporter hook — observation is the filesystem, the same as for any
   other test.)
4. The test triggers discovery. The host transpiles the plugin,
   creates a fresh isolate, and runs `load()`.
5. The test asserts both probe files exist in the temp dir with their
   expected contents. The first file proves dispatch reached the real
   rmcp handler. The second file proves the return value crossed back
   through the dispatcher into the isolate. If any stage of the
   pipeline is broken, at least one assertion fails.

This test exercises the whole pipeline — manifest load, TS transpile,
isolate creation, server lookup, operation-mode dispatch, return-value
marshalling — using only platform primitives. No `cwd` field on the
Plugin base class, no test-only reporter hook, no fakes.

### What each kind of test must exercise

One reference integration test per capability the platform advertises.
Every test follows the same shape: real isolate, real registered
server, observe an effect that only happens if the platform works.

| Capability                       | What the test proves                                       |
| -------------------------------- | ---------------------------------------------------------- |
| **In-process server dispatch**   | `this.files.write({...})` from a plugin writes a real file (the case above) |
| **CLI subprocess server**        | Plugin registers `{ cli: [...] }`; host spawns it; calls go through stdio and return |
| **URL server**                   | Plugin registers `{ url: ... }`; host calls it; mock HTTP endpoint records the request shape |
| **Operation-mode metadata**      | Operation server's `_meta.verbs` round-trips: plugin calls `this.<server>.<noun>.<verb>({...})`, server receives the right `(tool, verb, args)` |
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
`swissarmyhammer-plugins/tests/integration/` (or wherever the plugin
host crate lands), with one file per capability. The `files`
reference test is `files_dispatch_e2e.rs`. The naming convention
matches the existing `*_e2e.rs` pattern already used by the code-context
and skill suites.

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
    fn capabilities(&self) -> &ServerCapabilities;     // cached from initialize
    async fn invoke(
        &self,
        caller: CallerId,
        tool: &str,           // for operation servers, the noun class
        verb: Option<&str>,   // None for flat tools, Some for operation servers
        input: Value,         // all parameters including any instance id
    ) -> Result<Value>;
}
```

`capabilities()` exposes the cached `ServerCapabilities` from the
`initialize` handshake. The dispatcher checks
`capabilities.experimental` for `io.swissarmyhammer/operations` to know
whether to pass `verb` or leave it `None`. The codegen reads the same
flag when emitting types.

`ToolMetadata` includes the per-tool `_meta` (with the verb table for
operation tools) returned by the server's `tools/list`, from which the
codegen and CLI generator both derive per-verb input shapes.

Three production implementations, one per source kind:

- **`InProcessServer<S: rmcp::ServerHandler>`** — wraps an rmcp handler.
  `invoke` folds `verb` (when present) into the `arguments` map under
  that key and calls `S::call_tool` directly. No serialization, no
  IPC. This is the path for host Rust code (see *In-Process Host
  Servers*).
- **`CliServer`** — wraps a spawned subprocess. `invoke` sends a JSON-RPC
  `tools/call` over the subprocess's stdin (with `verb` folded into
  arguments) and awaits the response on stdout. The platform manages
  the subprocess lifecycle (spawn on register, kill on unregister,
  restart on crash if configured).
- **`UrlServer`** — wraps an HTTP MCP transport. `invoke` sends a
  JSON-RPC `tools/call` to the configured URL (with `verb` folded into
  arguments) and awaits the response. Authentication headers from the
  registration are reused on every call.

Each backend implements `tools()` by caching the response of the
server's `tools/list` at connection time, refreshed on
`notifications/tools/list_changed`. Each implements `capabilities()` by
caching the `ServerCapabilities` returned from `initialize`, fixed for
the lifetime of the connection.

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
