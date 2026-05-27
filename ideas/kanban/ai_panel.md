# AI Panel

## The Core Idea

The Kanban app grows a right-docked **AI panel** on the main layer. The user
picks a model, types a prompt, and converses with an agent that can *see and
modify the board they are looking at*. The conversation UI is built from
[AI Elements](https://elements.ai-sdk.dev/docs); the agent runs over **Agent
Client Protocol (ACP)** — Claude Code when `claude` is on `PATH`, and our own
local models otherwise. The agent reaches the board through the **kanban MCP
tool**, which the app exposes on a random loopback port and hands to the agent
over ACP at session start.

Two hard rules, set by this design and non-negotiable:

- **The Kanban app is a proper ACP `Client`.** The Tauri backend implements the
  `agent_client_protocol::Client` *role* — `Client::builder()` with real
  registered handlers for every client-side method. Not a hand-rolled
  approximation, not a side channel.
- **All agent communication is ACP, and only ACP.** The one and only channel
  between the app and the agent is the ACP connection. No broadcast side
  channel, no bespoke IPC, no reaching into agent internals.

Three things have to come together:

1. **Tools** — the app hosts an in-process HTTP MCP server *per open board*,
   each on its own random `127.0.0.1` port, exposing the `kanban` tool bound to
   that board.
2. **Agent** — the Tauri backend, acting as the ACP `Client`, constructs the
   agent (`swissarmyhammer-agent::create_agent`), runs the ACP session, and
   hands the agent the kanban MCP URL via the ACP `NewSessionRequest.mcp_servers`
   field. Every byte to and from the agent crosses on the ACP wire.
3. **UI** — the panel is a React surface built from AI Elements components,
   driven by the AI SDK `useChat` hook through a custom transport that bridges
   the webview to Tauri commands and events. The chat is **stateless**: a fresh
   ACP session each time, no persisted transcripts.

Everything that already exists is reused. Nothing about model orchestration
lives in the webview — the webview only renders.

---

## Background — What Already Exists

**ACP agent infrastructure** (`crates/`):

- `agent-client-protocol` 0.11 + `agent-client-protocol-extras` — the protocol
  crates, with `TracingAgent` middleware and conformance helpers.
- `claude-agent` — wraps the `claude` CLI as an ACP agent. Spawns
  `Command::new("claude")` with `CLAUDE_ACP=1` (`claude_process.rs`). Already
  depends on `swissarmyhammer-kanban`.
- `llama-agent` — local llama.cpp inference exposed over ACP.
- `swissarmyhammer-agent` — the **unified entry point**. `create_agent(&ModelConfig,
  Option<McpServerConfig>) -> AcpAgentHandle` dispatches on
  `ModelExecutorType::{ClaudeCode, LlamaAgent, …}`. `McpServerConfig::from_port(port)`
  builds `http://localhost:{port}/mcp`. `AcpAgentHandle` carries a
  `DynConnectTo<Client>` agent component plus a
  `broadcast::Receiver<SessionNotification>`.
- `swissarmyhammer-config::model` — `ModelConfig`, `ModelExecutorType`,
  `ModelExecutorConfig`.

**Kanban MCP** (`apps/kanban-cli/src/commands/serve.rs`):

- `KanbanMcpServer` — an `rmcp::ServerHandler` exposing one tool, `kanban`,
  that dispatches to `swissarmyhammer_kanban::dispatch::execute_operation`. It
  currently serves **stdio only** and builds a fresh `KanbanContext` rooted at
  `<cwd>/.kanban` per call.

**HTTP MCP transport** — `rmcp::transport::streamable_http_server::StreamableHttpService`
with `LocalSessionManager` is the established in-repo pattern for serving MCP
over HTTP. `agent-client-protocol-extras/src/test_mcp_server.rs` shows the full
shape: bind `TcpListener` to `127.0.0.1:0`, wrap the handler in a
`StreamableHttpService`, mount it on an `axum` router at `/mcp`, `axum::serve`.

**The Kanban app** (`apps/kanban-app`):

- A Tauri 2 app. The Rust backend owns the kanban engine *in process* —
  `AppState.boards: RwLock<HashMap<PathBuf, Arc<BoardHandle>>>`, one
  `BoardHandle` per open board, each holding an `Arc<KanbanContext>`, an
  `Arc<EntityCache>`, and a bridge task that forwards entity-cache events to
  the webview as Tauri events.
- The webview (`apps/kanban-app/ui`, React 19 + Tailwind 4 + shadcn/Radix +
  CodeMirror 6) talks to the backend through `invoke_handler` commands and
  receives entity changes through emitted Tauri events.
- The layout is NAV │ VIEW AREA │ (inspectors) │ bottom bar. The window-root
  spatial-nav layer is named `"window"` in `App.tsx` — this is the **main
  layer** the AI panel docks onto.
- App architecture (commands as composable scopes, perspectives, grid) is in
  [`app-architecture.md`](app-architecture.md).

---

## Architecture Overview

```
┌──────────────────────── Kanban.app (one window) ─────────────────────────┐
│                                                                          │
│  Webview (React)                          Tauri backend (Rust)           │
│  ┌────────────────────────────┐           ┌───────────────────────────┐  │
│  │ NAV │ VIEW AREA │ AI PANEL  │           │ AppState                  │  │
│  │                 │  ┌──────┐ │  invoke   │  boards: {path→BoardHandle│  │
│  │                 │  │ AI   │ │ ───────►  │   (each owns its kanban   │  │
│  │                 │  │ Elem.│ │  events   │    HTTP MCP server)       │  │
│  │                 │  └──────┘ │ ◄───────  │  ai_sessions: {window→…}  │  │
│  └────────────────────────────┘           └─────────────┬─────────────┘  │
│   (the webview is NOT an ACP participant — it only renders)              │
│                                       ┌──────────────────┴────────────┐   │
│                                       │ ACP CLIENT  (per window)       │   │
│                                       │  agent_client_protocol::Client │   │
│                                       │  .builder() … .connect_with()  │   │
│                                       └──────────────────┬────────────┘   │
│                                                          │ ACP (only)     │
│                                          ┌───────────────┴────────────┐   │
│                                          │ claude-agent  │ llama-agent │   │
│                                          │  (spawns      │  (in-proc   │   │
│                                          │   `claude`)   │   inference)│   │
│                                          └───────────────┬────────────┘   │
│                                                          │ MCP / HTTP     │
│                          (URL delivered over ACP: NewSessionRequest)       │
│                                       ┌──────────────────┴────────────┐   │
│                                       │ KanbanHttpServer — one per     │   │
│                                       │ open board, each on its own    │   │
│                                       │ random 127.0.0.1 port, /mcp    │   │
│                                       │   factory move-captures THAT   │   │
│                                       │   board's live KanbanContext   │   │
│                                       └────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────────────┘
```

The decisive choice: the HTTP MCP server dispatches against the **same
`Arc<KanbanContext>` the UI uses**, not a fresh one. When the agent calls
`add task`, the write lands in the live `EntityCache`, the existing bridge task
fires, and the board re-renders — the user watches the agent edit the board in
real time, with no extra plumbing.

---

## Phase 1 — Kanban Tools over HTTP

### Why a new server

ACP hands an agent its MCP servers as **HTTP** endpoints — `NewSessionRequest.mcp_servers`
carries URLs. The existing `KanbanMcpServer` serves **stdio** and re-roots a
fresh `KanbanContext` per call, so it cannot be used. The app needs an HTTP MCP
server that dispatches every call against the *live* board.

### One server per board — and why not one server with board-id paths

A single server keyed by a URL path segment (`/boards/{id}/mcp`) **does not
work**, and the `rmcp` source proves it:

- `StreamableHttpService::new`'s service factory is
  `Fn() -> Result<S, io::Error>` — it receives **no request context** at all,
  and in stateful mode is called once per MCP *session* (at `initialize`), not
  per request (`rmcp` `transport/streamable_http_server/tower.rs`). It cannot
  pick a board.
- `axum`'s `nest_service` **strips the matched path prefix** before the inner
  service sees the request, and a captured `:board_id` param lands in a
  *private* axum extension that only the `Path` extractor — inside an axum
  handler, which `StreamableHttpService` is not — can read. The id is
  unreachable.

The design that *does* work: **one HTTP MCP server per open board**, each on
its own random port, with the board bound into the factory by `move`. A new
module, `apps/kanban-app/src/ai/kanban_http.rs`; one server per `BoardHandle`:

```rust
let ctx = Arc::clone(&board.ctx);                       // live KanbanContext
let listener = TcpListener::bind("127.0.0.1:0").await?; // random, loopback
let port = listener.local_addr()?.port();
let svc = StreamableHttpService::new(
    move || Ok(KanbanMcpHandler::new(Arc::clone(&ctx))), // board bound HERE
    LocalSessionManager::default().into(),
    StreamableHttpServerConfig::default(),               // loopback allow-list
);
let app = axum::Router::new().nest_service("/mcp", svc); // static prefix
tokio::spawn(async move { axum::serve(listener, app).await });
```

- **One server per `BoardHandle`.** Started in `BoardHandle::open`, stopped on
  board close (drop the serve task; cancel via the config's
  `cancellation_token`). URL: `http://127.0.0.1:<port>/mcp`.
- **`KanbanMcpHandler`** is an `rmcp::ServerHandler` holding *that board's*
  live `Arc<KanbanContext>`. `call_tool` runs
  `swissarmyhammer_kanban::dispatch::execute_operation(&ctx, op)` — what
  `kanban-cli`'s `serve` does, minus the fresh-context step. Tool name,
  description, schema, and the `KanbanError` → `McpError` classifier are lifted
  from `serve.rs`; extract the shared pieces (`build_list_tools_result`,
  `classify_kanban_error`) into `swissarmyhammer-kanban` so the two servers
  cannot drift.
- **No board id in the URL.** The board *is* the port. Each window's panel is
  handed only its board's URL — it cannot name, enumerate, or reach another.

(A single server with a `?board=<id>` *query string* would also work — the
query survives `nest_service`, and `rmcp` injects `http::request::Parts` into
every message for the handler to read. It is the documented fallback if N
listeners ever becomes a concern; per-board servers are simpler and need it
not.)

### What Phase 1 delivers

```
- apps/kanban-app/src/ai/kanban_http.rs — per-board HTTP MCP server + handler
- One axum + StreamableHttpService per BoardHandle on a random loopback port
- Service factory move-captures the board's live KanbanContext
- Dispatch against that live context (mutations flow to the UI)
- Shared kanban-tool schema/classifier extracted into swissarmyhammer-kanban
- Started in BoardHandle::open, stopped on board close
- Tauri command: get_kanban_mcp_url(board_path) -> String
```

### Open dependency

`kanban-app/Cargo.toml` gains `rmcp`, `axum`, and `tokio` HTTP features. These
are already in the workspace lock — no new external surface.

---

## Phase 2 — The ACP Client

### The backend *is* an ACP `Client` — stated precisely

```
webview  ──Tauri commands/events──►  Rust backend  ──ACP (only)──►  agent
(renders)                            (ACP Client)                   (ACP Agent)
```

- The **webview is not an ACP participant.** It renders and speaks Tauri
  commands/events to the backend. Those are app-internal plumbing — not ACP,
  and they never pretend to be.
- The **Rust backend is the ACP `Client`.** It implements the
  `agent_client_protocol::Client` role via `Client::builder()`, registering a
  real handler for *every* client-side method. It owns the
  `ConnectionTo<Agent>` for issuing requests and the registered handlers for
  the callbacks the agent makes back. Nothing else talks to the agent.
- The **agent is the ACP `Agent`** — `claude-agent` or `llama-agent`, produced
  as a `DynConnectTo<Client>` component by `swissarmyhammer_agent::create_agent`.

`create_agent` is used **only** to construct that component (it picks `claude`
vs `llama` and wraps `TracingAgent`). We do **not** call
`swissarmyhammer_agent::execute_prompt` — that one-shot convenience reads the
`broadcast::Receiver<SessionNotification>` *side channel*, which violates
"strictly ACP." We take `AcpAgentHandle::agent` and drive it through our own
`Client` — exactly the path `swissarmyhammer-agent` documents for "applications
that need session persistence and history."

### Building the Client

```rust
use agent_client_protocol::{Agent, Client, ClientCapabilities, ConnectionTo};

// 1. Construct the agent component. mcp_config = None — see "MCP over ACP".
let handle = swissarmyhammer_agent::create_agent(&model_config, None).await?;

// 2. Build the Client: a handler for every callback the agent can make.
let connection: ConnectionTo<Agent> = Client::builder()
    .on_receive_notification(/* session/update → Tauri events        */)
    .on_request_permission(/*  session/request_permission → panel    */)
    .on_read_text_file(/*      fs/read_text_file                     */)
    .on_write_text_file(/*     fs/write_text_file                    */)
    .on_create_terminal(/*     terminal/* …                         */)
    // … every remaining Client-role method …
    .connect_with(handle.agent /* the DynConnectTo<Client> */, transport)
    .await?;
```

The `Client` *role* surface is the contract: every client-side method is
genuinely handled — none stubbed with `unimplemented!`, none bypassed. (Exact
builder method names track `agent-client-protocol` 0.11.)

### Client capabilities — advertised honestly

A proper Client sends truthful `ClientCapabilities` in `initialize`. For v1 the
kanban agent works *through the kanban MCP tool*, not raw files or shells:

| Capability          | v1 value | Handler behavior                              |
|---------------------|----------|-----------------------------------------------|
| `fs` (read/write)   | `false`  | Implemented; returns ACP "capability not supported". |
| `terminal`          | `false`  | Implemented; returns ACP "capability not supported". |
| streaming / updates | `true`   | `session/update` notifications consumed.      |

Advertising `false` does **not** mean omitting the handler — every `Client`
method is implemented; the handler honestly refuses. (Raising `fs` to a
board-scoped real implementation is a later phase.)

### MCP servers travel over ACP

The kanban tool is given to the agent the spec-defined way: in the ACP
`NewSessionRequest.mcp_servers` field. The backend resolves the window's board
→ its `KanbanHttpServer` URL (`http://127.0.0.1:<port>/mcp`) and passes it in
the `new_session` request. The URL crosses on the ACP wire — not a side
channel — which is why `create_agent` is called with `mcp_config: None`. (If a
backend honored only creation-time MCP config, that is a backend bug to fix,
not a license to bypass ACP.)

### Per-window session — stateless

One ACP session per window; **each chat is stateless**. There is no transcript
persistence: a window reload, a "New conversation", or a model switch all start
a fresh ACP `new_session`. The conversation lives only in webview state for the
lifetime of the current session.

```rust
struct AiSession {
    window_label: String,
    board_path: PathBuf,
    model_id: String,
    connection: ConnectionTo<Agent>,   // the Client's handle to the agent
    acp_session_id: SessionId,
    client_task: JoinHandle<()>,       // drives the Client connection
    cancel: CancellationToken,
}

// AppState gains:
ai_sessions: RwLock<HashMap<String /*window_label*/, AiSession>>,
```

### Session lifecycle — all ACP

```
1. Resolve the window's board → its KanbanHttpServer URL.
2. Load the ModelConfig for the selected model (Phase 3).
3. create_agent(&model_config, None) → AcpAgentHandle (agent component only).
4. Client::builder() … .connect_with(handle.agent) → ConnectionTo<Agent>.
5. ACP: initialize   — send honest ClientCapabilities
        new_session  — mcp_servers = [ the board's kanban HTTP MCP server ]
        set_session_mode("code")
6. ACP: prompt — content arrives via session/update notifications, handled by
        the Client and forwarded to the webview as Tauri events. The
        PromptResponse carries only the StopReason.
```

### Notification handling

`session/update` notifications arrive through the Client's registered
notification handler — the ACP JSON-RPC channel, never a broadcast. Each
`SessionUpdate` variant is translated and emitted to the webview:

| ACP `SessionUpdate`           | Panel rendering (AI Elements)              |
|-------------------------------|--------------------------------------------|
| `AgentMessageChunk`           | `Response` — streamed assistant text       |
| `AgentThoughtChunk`           | `Reasoning` — collapsible thinking         |
| `ToolCall` / `ToolCallUpdate` | `Tool` — kanban call, args, status, result |
| `AgentPlan`                   | `Task` / `ChainOfThought` — the agent's plan |
| `AvailableCommandsChanged`    | slash-command suggestions in `PromptInput` |

### Permission requests

`session/request_permission` is a first-class **ACP Client request** — the
agent calls it, the backend's registered handler receives it, forwards
`ai://permission/{window}` to the panel, awaits the user's choice, and returns
the ACP `RequestPermissionResponse`. The reply *is the return value* of the ACP
handler — not a separate message. A permission-policy setting
(`always-ask` / `auto-approve-reads` / `auto-approve-all`) lets the handler
answer common `kanban` read calls without a prompt.

### Cancellation & teardown

- `ai_cancel_prompt` → ACP `cancel` notification on the connection +
  `CancellationToken`.
- Window/board close → drop `AiSession`, close the ACP connection (dropping
  `ConnectionTo<Agent>` terminates the agent subprocess), stop `client_task`.

### What Phase 2 delivers

```
- apps/kanban-app/src/ai/client.rs — the ACP Client (Client::builder + handlers)
- Every client-role method implemented; honest ClientCapabilities
- apps/kanban-app/src/ai/session.rs — AiSession, stateless per-window lifecycle
- MCP server delivered via ACP NewSessionRequest.mcp_servers
- session/update → Tauri events (via the Client notification handler)
- session/request_permission handled as an ACP request; reply is the return value
- Tauri commands: ai_start_session, ai_send_prompt, ai_cancel_prompt,
  ai_respond_permission, ai_close_session
- Tauri events: ai://chunk/*, ai://permission/*, ai://status/*
```

---

## Phase 3 — Model Selection & `claude` Detection

### Detecting Claude Code

Claude Code is available *iff* the `claude` executable resolves on `PATH`. The
backend probes once at startup (and on demand when the selector opens):

```
fn detect_claude() -> Option<ClaudeInfo>
  - resolve `claude` on PATH (which-style lookup; honor CLAUDE_CLI override)
  - optionally `claude --version` for a display string
  - cache the result; re-probe when the user reopens the selector
```

When present, the selector shows a **Claude Code** entry backed by a
`ModelConfig` with `ModelExecutorType::ClaudeCode`. When absent, the entry is
shown disabled with a "claude not found on PATH" hint rather than hidden — so
the feature is discoverable.

### Local models

Local llama models come from `swissarmyhammer-config` (`ModelConfig` /
`ModelExecutorType::LlamaAgent`). The selector enumerates configured models;
each is its own entry.

### The selector

A compact dropdown in the panel header (AI Elements ships a model-picker
pattern; otherwise a shadcn `Select`). Entries:

```
● Claude Code            (claude 1.x)         ← if `claude` on PATH
○ <local model name>     (llama-agent)        ← per configured model
  claude not found ⓘ                          ← disabled, if absent
```

Selecting a different model **ends the current ACP session and starts a new
one** — the MCP URL is unchanged, only the agent backend differs. The choice
is remembered per board in `UIState` so each board reopens with its last model.

### What Phase 3 delivers

```
- apps/kanban-app/src/ai/models.rs — detect_claude(), model enumeration
- Tauri command: ai_list_models() -> [{ id, label, kind, available, hint }]
- Model choice persisted per board in UIState
- Selector switches the agent backend, reusing the same MCP URL
```

### Build-weight note

`claude-agent` is light — it shells out to `claude`. `llama-agent` bundles
inference and is heavy to compile and ship. Gate local-model support behind a
Cargo feature (`ai-local-models`, off by default for the standard
`Kanban.app` bundle) so Claude Code works without paying the llama.cpp cost.
Treat local models as the last phase.

---

## Phase 4 — The AI Panel UI

### AI Elements

[AI Elements](https://elements.ai-sdk.dev/docs) is a shadcn-style component
library — components are *copied into* the project (the repo already uses
shadcn: `components.json`, Radix, Tailwind 4). Install with the AI Elements
CLI; components land under `apps/kanban-app/ui/src/components/ai-elements/`.
Components used:

```
Conversation   scroll container + autoscroll
Message        user / assistant message rows
Response       streamed markdown assistant text
Reasoning      collapsible thinking blocks (AgentThoughtChunk)
Tool           tool-call card: name, args, status, result (kanban calls)
Task           the agent's plan (AgentPlan)
PromptInput    composer — textarea, model selector slot, submit/stop
Loader         streaming indicator
Actions        copy / retry on a message
```

### Transport — `useChat` over Tauri

AI Elements pairs with the AI SDK `useChat` hook, which accepts a custom
`transport` implementing the AI SDK's `ChatTransport`. We implement
**`TauriChatTransport`** — the webview↔backend bridge. To be unambiguous: this
is the *AI SDK* transport interface, **not** an ACP transport. ACP is strictly
backend↔agent (Phase 2); the webview never speaks ACP.

```
// AI SDK ChatTransport — bridges the webview to the Tauri backend. Not ACP.
class TauriChatTransport implements ChatTransport<UIMessage> {
  sendMessages({ messages, abortSignal }) {
    // invoke("ai_send_prompt", { windowLabel, text })
    // return a ReadableStream<UIMessageChunk> fed by Tauri events:
    //   listen("ai://chunk/{windowLabel}")   → text / reasoning / tool parts
    //   listen("ai://status/{windowLabel}")  → finish / error
    // abortSignal → invoke("ai_cancel_prompt", …)
  }
  reconnectToStream() { /* not supported — sessions are stateless */ }
}
```

The backend's notification handler (Phase 2) already shapes events as
`UIMessageChunk` parts, so the transport is a thin event-to-stream adapter.
This keeps `useChat` — and every AI Elements component — idiomatic, with no
mock HTTP server. Because the chat is stateless, there is nothing to
reconnect to and no transcript to rehydrate.

### Layout & placement

The panel docks on the **right** of the main layer, a sibling of the view
area, *inside* the window layer and *outside* the inspector stack:

```
┌──────┬───────────────────────────────┬──────────────────┐
│ NAV  │  VIEW AREA                    │  AI PANEL         │
│      │  (grid / board)               │  ┌─────────────┐  │
│      │                               │  │ model ▾   ⤬ │  │
│      │                               │  ├─────────────┤  │
│      │                               │  │ Conversation│  │
│      │                               │  │  Message    │  │
│      │                               │  │  Tool …     │  │
│      │                               │  ├─────────────┤  │
│      │                               │  │ PromptInput │  │
│      │  ┌─────────────────────────┐  │  └─────────────┘  │
│      │  │ -- NORMAL --   Tasks    │  │                   │
└──────┴───┴─────────────────────────┴─┴───────────────────┘
```

- **Collapsible.** A toggle command (`ai.toggle`) and keybinding show/hide the
  panel. Width is user-draggable, persisted per board in `UIState`.
- The quick-capture window never shows the panel.
- New container `AiPanelContainer` slots into `App.tsx`'s hierarchy as a
  sibling of `ViewsContainer`, inside `WindowContainer`.

### What Phase 4 delivers

```
- AI Elements components vendored under ui/src/components/ai-elements/
- TauriChatTransport (AI SDK ChatTransport; webview↔backend bridge, not ACP)
- AiPanel component (Conversation, Message, Response, Reasoning, Tool, Task)
- AiPanelContainer wired into App.tsx, right-docked, collapsible, resizable
- Panel + model + width state persisted per board in UIState (NOT the transcript)
```

---

## Phase 5 — Commands & Spatial Navigation

The panel is a first-class citizen of the command and focus systems
([`app-architecture.md`](app-architecture.md)).

### Command scope

The panel registers a command scope at the window layer:

```yaml
- id: ai.toggle      name: Toggle AI panel     keys: { vim: ":ai",    cua: Mod+J }
- id: ai.focus       name: Focus AI panel      keys: { vim: "Mod+I" }
- id: ai.newChat     name: New conversation    keys: { vim: ":ai new" }
- id: ai.model       name: Change model        pattern: ":ai model <name>"
- id: ai.cancel      name: Stop generation     keys: { vim: Escape (while streaming) }
```

### Spatial navigation

The panel is its own spatial-nav **layer/zone**, a child of the window root.
Per the path-based-moniker rule, its zone moniker must be a proper path
*through* the window layer — not a flat leaf — so navigation between the view
area and the panel does not register as a cross-layer jump. The composer
(`PromptInput`) is a CodeMirror 6 single-line/multi-line instance, consistent
with every other text input in the app, so the user's keymap (vim/emacs/CUA)
works inside the prompt.

### What Phase 5 delivers

```
- AI panel command scope (ai.toggle, ai.focus, ai.newChat, ai.model, ai.cancel)
- Panel spatial-nav layer/zone, path-correct moniker under the window layer
- CM6 composer with the app keymap
- Bottom bar shows AI status (idle / streaming / error)
```

---

## Phase 6 — Local Models

Wire `ModelExecutorType::LlamaAgent` end to end behind the `ai-local-models`
Cargo feature: enumerate configured llama models, create the agent, run
sessions. Same MCP URL, same panel, same transport — only the backend differs.
Last because of the build/bundle cost called out in Phase 3.

---

## Wire Protocol

These are the app-internal **webview↔backend** channels — *not* ACP. ACP is
strictly backend↔agent (Phase 2); the webview never speaks it.

**Tauri commands** (webview → backend):

```
get_kanban_mcp_url(board_path)                   -> string
ai_list_models()                                 -> Model[]
ai_start_session(window_label, model_id)         -> { sessionId }
ai_send_prompt(window_label, text)               -> ()    // stream via events
ai_cancel_prompt(window_label)                   -> ()
ai_respond_permission(window_label, request_id, decision) -> ()
ai_close_session(window_label)                   -> ()
```

**Tauri events** (backend → webview), per `window_label`:

```
ai://chunk/{label}      UIMessageChunk part (text | reasoning | tool | data)
ai://permission/{label} { requestId, toolName, args }
ai://status/{label}     { state: idle|streaming|error, stopReason?, error? }
```

---

## Security

- Each board's HTTP MCP server binds **loopback only** (`127.0.0.1:0`). rmcp's
  `StreamableHttpServerConfig` defaults `allowed_hosts` to
  `["localhost", "127.0.0.1", "::1"]` and rejects other `Host` headers — a
  built-in DNS-rebinding guard. Keep that default.
- The agent receives only its board's URL — a port. It cannot name, enumerate,
  or reach another open board's server. The port *is* the capability.
- *Hardening:* mint a per-session bearer token, require it in `KanbanMcpHandler`,
  and deliver it on the URL carried by ACP `NewSessionRequest.mcp_servers`.
  Adopt once it is confirmed the ACP MCP-server entry can carry HTTP headers.
- The `kanban` tool can mutate the board. Tool calls flow through the ACP
  `session/request_permission` request to the panel's permission UI; the
  default policy is `always-ask`.
- All agent traffic is ACP — no side channel widens the surface.
- `claude` is spawned with the same argument/env shape `claude-agent` already
  uses (`CLAUDE_ACP=1`); no new process-spawning surface.

---

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| ACP role | Backend implements the ACP `Client` role (`Client::builder()`) | Proper Client, every callback handled; the only channel to the agent. |
| Agent communication | Strictly ACP — no broadcast side channel, no `execute_prompt` | "All agent traffic is ACP" is a hard rule; `execute_prompt` uses a side channel. |
| Tool transport | New in-process HTTP MCP server | `swissarmyhammer-agent` only points agents at HTTP URLs; stdio `KanbanMcpServer` can't be reused. |
| MCP dispatch target | The **live** `BoardHandle` | Agent edits flow through the entity cache → bridge → UI updates in real time. |
| Board scoping | One MCP server per open board, own random port | `StreamableHttpService`'s factory gets no request context and `nest_service` strips path params; per-board lets the factory `move`-capture the board. |
| MCP delivery | `NewSessionRequest.mcp_servers`, over ACP | Strictly ACP — the URL crosses on the wire; `create_agent` gets `mcp_config: None`. |
| Port | Random `127.0.0.1:0`, one per board | Loopback-only; the port *is* the board's identity. |
| Agent entry point | `swissarmyhammer_agent::create_agent` (component only) | One call dispatches Claude Code vs llama, wraps `TracingAgent`; we own the Client. |
| Session scope | One ACP session per window; **stateless** | Each window shows one board; no transcript persisted — fresh `new_session` each chat. |
| Content source | `session/update` notifications, not `PromptResponse` | ACP delivers content via notifications; the response is only a `StopReason`. |
| UI library | AI Elements (shadcn-style copy-in) | The app already uses shadcn/Radix/Tailwind; components are owned, not a dependency. |
| Webview ↔ backend | `useChat` + `TauriChatTransport` | AI SDK transport, *not* ACP; webview is not an ACP participant. |
| Claude detection | `claude` resolvable on `PATH` | Matches how `claude-agent` spawns it; absent → disabled selector entry. |
| Local models | Cargo feature `ai-local-models`, last phase | `llama-agent` bundles llama.cpp — heavy; Claude Code must work without it. |
| Placement | Right dock, window layer, sibling of view area | The "main layer" the user named; outside the inspector stack. |
| Composer | CodeMirror 6 | One keymap contract with every other text input in the app. |

---

## Open Questions

1. **Bearer token.** Per-board loopback ports are the baseline. Confirm whether
   the ACP `NewSessionRequest.mcp_servers` entry can carry HTTP headers; if so,
   add a per-session token (see Security). If not, loopback + per-board port
   stands for v1.
2. **System prompt.** Does the agent get a board-specific system prompt (board
   name, columns, conventions) at `new_session`? Likely yes — decide the
   content.
3. **Slash commands.** ACP `AvailableCommandsChanged` can surface agent
   commands in `PromptInput`. In scope for v1 or deferred?
4. **`fs` capability.** v1 advertises `fs: false`. Some flows (the agent
   reading a referenced file) may want a board-scoped real implementation —
   decide when to raise it.
5. **`mcp_servers` honored at `new_session`.** Confirm `claude-agent` and
   `llama-agent` read `NewSessionRequest.mcp_servers` rather than only
   creation-time MCP config (`swissarmyhammer-agent::create_claude_agent`
   currently bakes MCP config in at creation for its one-shot path). If a
   backend ignores the ACP field, that gap must be closed *in the agent crate*
   — the app must not work around it with a side channel.

*Resolved:* **conversation persistence** — none; the chat is stateless, a
fresh ACP session each time (per the design above). **One server vs. per-board**
— per-board (Phase 1); the single-server + `?board=` query-string variant is
the fallback if N listeners ever becomes a concern.

---

## Implementation Todo (Dependency Order)

**Phase 1 — Kanban tools over HTTP**
- [ ] Extract the kanban-tool schema + `classify_kanban_error` from
      `kanban-cli/src/commands/serve.rs` into `swissarmyhammer-kanban` (shared).
- [ ] `apps/kanban-app/src/ai/kanban_http.rs` — `KanbanHttpServer` +
      `KanbanMcpHandler`; `axum` + `StreamableHttpService`, bind `127.0.0.1:0`.
- [ ] One server per `BoardHandle`; factory `move`-captures the board's live
      `KanbanContext`. Start in `BoardHandle::open`, stop on board close.
- [ ] `get_kanban_mcp_url` Tauri command + tests (call `kanban` over HTTP,
      assert the mutation reaches the live board).

**Phase 2 — The ACP Client**
- [ ] Add `swissarmyhammer-agent` (+ `agent-client-protocol`) to
      `kanban-app/Cargo.toml`.
- [ ] `apps/kanban-app/src/ai/client.rs` — implement the ACP `Client` role via
      `Client::builder()`; a handler for every client-side method; honest
      `ClientCapabilities` (`fs`/`terminal` = false but implemented).
- [ ] `apps/kanban-app/src/ai/session.rs` — `AiSession`; create via
      `create_agent(.., None)` then `connect_with`; stateless lifecycle.
- [ ] Deliver the kanban MCP URL in `NewSessionRequest.mcp_servers`.
- [ ] `session/update` handler → `UIMessageChunk`-shaped Tauri events.
- [ ] `session/request_permission` handler → panel prompt; reply is the
      handler's return value.
- [ ] `ai_start_session` / `ai_send_prompt` / `ai_cancel_prompt` /
      `ai_close_session` commands; teardown on window/board close.

**Phase 3 — Model selection**
- [ ] `apps/kanban-app/src/ai/models.rs` — `detect_claude()`, model enumeration.
- [ ] `ai_list_models` command.
- [ ] Persist model choice per board in `UIState`.

**Phase 4 — Panel UI**
- [ ] Vendor AI Elements components under `ui/src/components/ai-elements/`.
- [ ] `TauriChatTransport implements ChatTransport` (AI SDK transport, not ACP).
- [ ] `AiPanel` (Conversation/Message/Response/Reasoning/Tool/Task) + model
      selector + permission prompt.
- [ ] `AiPanelContainer` into `App.tsx`; right dock, collapsible, resizable;
      persist panel + width state.

**Phase 5 — Commands & spatial nav**
- [ ] AI panel command scope (`ai.toggle`, `ai.focus`, `ai.newChat`,
      `ai.model`, `ai.cancel`) + keybindings.
- [ ] Panel spatial-nav layer/zone with a path-correct moniker.
- [ ] CM6 composer; bottom-bar AI status.

**Phase 6 — Local models**
- [ ] `ai-local-models` Cargo feature; wire `LlamaAgent` end to end.

---

## References

- [App Architecture](app-architecture.md) — commands, scopes, layout, layers.
- [ACP Integration Plan](../acp.md) — ACP background and the llama-agent plan.
- [AI Elements](https://elements.ai-sdk.dev/docs) — the component library.
- `agent-client-protocol` 0.11 — the ACP `Client` and `Agent` roles.
- `crates/swissarmyhammer-agent/src/lib.rs` — `create_agent`, `AcpAgentHandle`;
  documents the `Client::builder().connect_with(handle.agent, …)` path and
  warns that `execute_prompt` uses the broadcast side channel.
- `rmcp` `transport/streamable_http_server/tower.rs` — proof the service
  factory takes no request context and `nest_service` strips the path prefix.
- `crates/agent-client-protocol-extras/src/test_mcp_server.rs` — the
  `StreamableHttpService` + `axum` HTTP-MCP pattern.
- `apps/kanban-cli/src/commands/serve.rs` — `KanbanMcpServer`, the kanban tool.
