---
assignees:
- wballard
position_column: done
position_ordinal: fffffffffffffffffffffffc80
title: 'Validator MCP server: always start in-process with read-only files + code_context tools'
---
**ACP-only constraint:** Tools reach the validator agent through ACP — i.e. via `McpServerConfig` passed at agent construction, which the agent backend (claude-code, llama-agent) consumes when implementing the `Agent` trait. `avp-common` itself doesn't call llama-agent or claude-code directly, and that property must hold for this fix too.

**The path:**

1. Parent claude (or any host) starts sah as an MCP server with HTTP transport. sah binds a port and exports `SAH_HTTP_PORT` (or `SWISSARMYHAMMER_HTTP_PORT`).
2. Claude's hooks fire avp as a subprocess. avp inherits the env, including `SAH_HTTP_PORT`.
3. `avp-common/src/context.rs::resolve_validator_mcp_config` reads the port and returns `Some(McpServerConfig { url: "http://localhost:{port}/mcp/validator" })` plus `tools_override = Some("")`.
4. The validator agent (claude-code or llama-agent) is constructed with that MCP config.
5. For llama-agent, the chat template reads tools via the MCP client and renders them into the system message — handled in llama-agent (see `01KQ35KFJXJ70GNB4ZPRJD6R43`).

The current `(None, None)` branch (env var unset) leaves llama-agent with no tools at all. Claude-as-validator falls back to its own built-in Read/Grep so it doesn't notice; qwen has no such fallback.

## What to change

**If `SAH_HTTP_PORT` is set:** keep the existing path. No change.

**If `SAH_HTTP_PORT` is unset:** start an in-process sah MCP server. **Do not shell out a subprocess.** The required entry point already exists in this workspace:

```rust
// In swissarmyhammer-tools/src/mcp/unified_server.rs
pub async fn start_mcp_server_with_options(
    mode: McpServerMode,                     // McpServerMode::Http { port: None } → bind 127.0.0.1:0
    library: Option<PromptLibrary>,          // None — defaults are fine
    model_override: Option<String>,          // None
    working_dir: Option<std::path::PathBuf>, // Some(project_root)
    agent_mode: bool,                        // see below
) -> Result<McpServerHandle>;
```

`McpServerHandle` exposes `.url() -> &str` (e.g. `http://127.0.0.1:54321`) and `.shutdown(&mut self) -> Result<()>` for graceful teardown. That's the entire integration surface — `await` to start, hand the URL into `McpServerConfig`, hold the handle on `AvpContext`.

**`agent_mode` flag matters:**
- For llama-agent-as-validator: `agent_mode: true` so the in-process sah registers the agent tools (Read/Glob/Grep/code_context). qwen needs these.
- For claude-code-as-validator: `agent_mode: false` — claude already has its own Read/Glob/Grep, registering ours would just duplicate or confuse. Claude only needs sah's domain tools (kanban, etc.), which `agent_mode: false` provides.

Determine `agent_mode` from the resolved model config (`ClaudeCode` → false, `LlamaAgent` → true). The selection already happens elsewhere in `AvpContext`; wire it through.

**Loud log on the fallback:**

```rust
tracing::warn!(
    url = %handle.url(),
    "SAH_HTTP_PORT not set; started in-process sah MCP server for validator tools — for production, run sah with HTTP transport in your parent shell so this fallback isn't needed"
);
```

Use `warn!` so it's visible at default log levels and shows up in `.avp/log` without flag wrangling.

## Lifecycle

- Hold the `McpServerHandle` on `AvpContext` (one per process). Drop the context → drop the handle → the handle's `Drop` (via the `shutdown_tx: oneshot::Sender<()>`) signals the spawned server task to exit. Verify the shutdown path actually works under `cargo test`; if `Drop` isn't enough, add an explicit `shutdown().await` in `AvpContext`'s teardown.
- The in-process server is local-only because `start_http_server` already binds `127.0.0.1` (verify in `unified_server.rs`).
- Across multiple hook invocations within one claude session, avp gets re-spawned each time, so a new in-process server starts each time too. That's acceptable; in-process startup is fast (no subprocess fork, no model load — just an axum listener and the tool registry that's already in memory). If hook latency is bad later, a long-running coordinator can be added — not now.

## Acceptance

- With qwen3.6 as the validator agent and `SAH_HTTP_PORT` unset:
  - A `WARN` log line in `.avp/log` clearly stating the in-process sah fallback fired and the URL it bound.
  - The validator agent's `McpServerConfig.url` matches the in-process handle's URL.
  - Combined with `01KQ35KFJXJ70GNB4ZPRJD6R43` (llama-agent Qwen3 strategy), at least one rule actually invokes a tool against the target file and the rule's verdict references actual file content.
  - No `Validator returned unparseable response, passing with warning` lines (assuming `01KQ35V5GTDS4ED3VWG8SAH4DQ` has also landed).
- With `SAH_HTTP_PORT` set (parent sah present), behavior is unchanged — no in-process server starts, no warn log.
- claude-code-as-validator continues to work in both cases. With `agent_mode: false` selected for claude, no tool collision.
- The in-process server is shut down cleanly when `AvpContext` is dropped — verify with a unit test that constructs an `AvpContext` (with `SAH_HTTP_PORT` unset), reads the URL, drops the context, then asserts the port is no longer listening.

## Investigation (do first, write findings into the task)

Before implementing, drop a `tracing::info!` in `resolve_validator_mcp_config` logging the `SAH_HTTP_PORT` value (or its absence), then re-run a Stop-hook test in the user's normal claude-with-sah setup. Determines whether the in-process fallback is the only path qwen ever uses or just a safety net for unconfigured users. Document the answer.

## Pairs with

- `01KQ35KFJXJ70GNB4ZPRJD6R43` (llama-agent Qwen3 strategy). Tools first puts qwen on Hermes rails so it emits `<tool_call>...</tool_call>`; that strategy renders the schemas in and parses the calls out.
- `01KQ34TAVZNR7FBYKNFSH0F19T` (per-rule fresh session). Independent — order doesn't matter.

## Sources

- `swissarmyhammer-tools/src/mcp/unified_server.rs` — `start_mcp_server_with_options`, `McpServerHandle`, `resolve_port`, `start_http_server`
- [Qwen Function Calling docs — chat template tool injection](https://qwen.readthedocs.io/en/latest/framework/function_call.html) #avp

## Review Findings (2026-04-27 07:47)

Scope: `avp-common/Cargo.toml`, `avp-common/src/context.rs` (the SAH_HTTP_PORT fallback / in-process MCP server work for this task; other branch changes ignored).

Verified:
- `cargo check -p avp-common` clean
- `cargo clippy -p avp-common --tests` clean
- All 546 `avp-common` lib tests pass, including the 3 new tests for this task
- Drop-based shutdown is genuinely correct: `McpServerHandle` holds `Option<oneshot::Sender<()>>` and the server task awaits `shutdown_rx.await.ok()` inside `with_graceful_shutdown` (`swissarmyhammer-tools/src/mcp/unified_server.rs:741-746`). Dropping the sender resolves the receiver with `Canceled`, `.ok()` discards it, and the listener releases. The lifecycle test polls and confirms.
- `/mcp/validator` route exists on the in-process router (`unified_server.rs:551`) — manual URL construction from `handle.port()` is appropriate and avoids string-rewriting `handle.url()`.
- Acceptance criteria met: WARN log on fallback, URL matches handle, env-set path unchanged, ClaudeCode → `agent_mode: false`, drop-based shutdown verified by test, investigation `info!` log present.

### Nits
- [x] `avp-common/src/context.rs` — `agent_mode_for_validator` has no test asserting the `LlamaAgent → true` branch. Only the `ClaudeCode → false` branch is exercised by `test_agent_mode_for_validator_defaults_to_false_for_claude`. Building a `ModelConfig` whose `executor_type()` returns `LlamaAgent` requires some test scaffolding, but a one-liner assertion would close the gap.

  **Resolved (2026-04-27):** Added `test_agent_mode_for_validator_is_true_for_llama_agent` in `avp-common/src/context.rs`. Test constructs a context via `AvpContext::init()`, swaps in `ModelConfig::llama_agent(LlamaAgentConfig::for_testing())` (direct private field mutation, permitted within the test submodule), and asserts `agent_mode_for_validator()` returns `true`. Both branches now covered. `cargo test -p avp-common --lib` passes 547 tests; `cargo clippy -p avp-common --tests` clean.