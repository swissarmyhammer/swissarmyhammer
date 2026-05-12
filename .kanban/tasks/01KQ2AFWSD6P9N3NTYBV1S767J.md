---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffff680
title: Upgrade rmcp 1.2 → 1.5 (latest)
---
## What

Upgrade the `rmcp` workspace dependency from `1.2` (resolves to 1.3.0) to `1.5` (latest released). Per upstream release notes for 1.3, 1.4, and 1.5, there are **no breaking API changes** — only additive features and bug fixes — so this is a version-bump-and-verify task.

**Files to modify:**

- `Cargo.toml` (workspace root, single change)
  ```toml
  rmcp = { version = "1.5", features = [
      "server",
      "client",
      "macros",
      "transport-io",
      "transport-streamable-http-server",
      "transport-streamable-http-client-reqwest",
      "transport-child-process",
      "auth",
      "elicitation",
      "schemars",
  ] }
  ```
- `Cargo.toml` (workspace root) — bump `rust-version` from `"1.91.1"` to `"1.92"` (rmcp 1.4 raised its MSRV to 1.92; current toolchain 1.95 is fine).
- `Cargo.lock` — regenerated automatically by `cargo update -p rmcp`.

**Consumers (verify each compiles cleanly):**
- `agent-client-protocol-extras`, `claude-agent`, `code-context-cli`, `kanban-cli`, `llama-agent`, `model-context-protocol-extras`, `shelltool-cli`, `swissarmyhammer-cli`, `swissarmyhammer-mcp-proxy`, `swissarmyhammer-tools`, `swissarmyhammer`

**API surface in use (all stable, confirmed unchanged in 1.5):**
- `rmcp::model::*` (CallToolResult, RawContent, Content, ProgressNotificationParam, etc.)
- `rmcp::{ServerHandler, ErrorData, RoleServer, RoleClient, ServiceExt, Peer}`
- `rmcp::serve_server`, `rmcp::service::{RequestContext, NotificationContext, serve_client, ServiceError}`
- `rmcp::transport::{io::stdio, StreamableHttpClientTransport, StreamableHttpServerConfig, StreamableHttpService}`
- `rmcp::transport::streamable_http_server::session::local::LocalSessionManager`

No custom `Transport` impls and no internal API reach-arounds — confirmed by codebase survey.

**Steps:**
1. Edit workspace `Cargo.toml`: change `rmcp = { version = "1.2", ... }` to `version = "1.5"`.
2. Bump `rust-version` to `"1.92"` if needed for clean resolution.
3. Run `cargo update -p rmcp` to refresh `Cargo.lock`.
4. Run `cargo build --workspace --all-targets` — must succeed with zero warnings.
5. Run `cargo clippy --workspace --all-targets -- -D warnings` — must succeed.
6. Run `cargo nextest run --workspace` — full test suite must pass.
7. Verify no new deprecation warnings against rmcp APIs.

**Out of scope (defer to follow-up if desired):**
- Adopting new 1.4/1.5 features (e.g., `local` feature for non-Send tools, auto-`get_info` from macros, transparent session re-init, 2025-11-25 protocol version support). This task is mechanical upgrade only.

## Implementation Note

rmcp 1.5 changed the default behavior of the `#[tool_handler]` and `#[prompt_handler]` macros: they now default to calling `Self::tool_router()` (static) rather than reading `self.tool_router` (field). In `llama-agent/src/echo.rs`, `EchoService` stores router fields populated by the constructor, so the new default would have rendered them dead code (one warning, blocking the zero-warnings acceptance criterion). Fixed by pinning the handlers to the field form — the rmcp 1.5 idiom for this constructor pattern:

```rust
#[tool_handler(router = self.tool_router)]
#[prompt_handler(router = self.prompt_router)]
impl ServerHandler for EchoService { ... }
```

This is a one-line, two-attribute change; no other consumer code needed adjustment.

## Acceptance Criteria

- [x] `cargo tree -p rmcp` reports `rmcp v1.5.x` (no other rmcp versions in the lockfile).
- [x] `cargo build --workspace --all-targets` succeeds with zero warnings.
- [x] `cargo clippy --workspace --all-targets -- -D warnings` succeeds.
- [x] `cargo nextest run --workspace` is fully green (no new failures, no skipped tests beyond pre-existing).
- [x] No new `deprecated` warnings against any `rmcp::*` symbol.
- [x] `Cargo.lock` regenerated and committed.

## Tests

- [x] Existing test suite is the regression net — every consumer crate's tests must still pass.
  - Run: `cargo nextest run --workspace`
  - Result: 13106 tests run, 13106 passed, 0 failed, 5 skipped (pre-existing).
- [x] MCP integration tests in `swissarmyhammer-tools` exercise `ServerHandler`, `serve_server`, and stdio transport — these prove the server path still works.
  - Covered by full workspace nextest run.
- [x] MCP proxy/client tests in `swissarmyhammer-mcp-proxy` and `model-context-protocol-extras` exercise `serve_client`, `StreamableHttpClientTransport`, and `ServiceExt` — these prove the client path still works.
  - Covered by full workspace nextest run.
- [x] If any test fails after the bump, file a follow-up task with the failure and its rmcp-related root cause; do **not** silence/skip tests. (No failures.)

## Workflow

- This is a dependency bump, not new behavior — TDD does not apply. Existing tests are the regression check.
- If `cargo build` surfaces unexpected compile errors, that contradicts the no-breaking-change premise; investigate before patching call sites.