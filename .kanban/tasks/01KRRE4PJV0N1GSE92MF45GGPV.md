---
assignees:
- claude-code
depends_on:
- 01KRRE3C4RDD999B43BRWJMA3J
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffff280
project: plugin-arch
title: 'plugin: McpServer trait, ToolMetadata, and ServerRegistry'
---
## What
Implement the registry core of `swissarmyhammer-plugin`: the `McpServer` trait every transport implements, the `ToolMetadata` type, and the `ServerRegistry`.

In `crates/swissarmyhammer-plugin/src/server.rs`:
- `#[async_trait] pub trait McpServer: Send + Sync` with `fn tools(&self) -> Vec<ToolMetadata>` and `async fn invoke(&self, caller: CallerId, tool: &str, input: Value) -> Result<Value>`. `invoke` is a plain `tools/call` — the platform never reads `input`; `op` (when present) is just a key the tool's own handler parses.
- `pub struct ToolMetadata` — the tool's `Tool` definition from `tools/list`: `name`, `description`, `inputSchema`, and `_meta` (so operation tools carry the `io.swissarmyhammer/operations` tree). Model it on rmcp's `Tool` shape so `InProcessServer` can produce it cheaply.
- A placeholder `CallerId` enum (`HostInternal`, `Plugin(PluginId)`, `External(String)`, `Unknown`) — the dispatcher task refines its use, but the trait signature needs it now.

In `crates/swissarmyhammer-plugin/src/registry.rs`:
- `pub struct ServerRegistry { servers: HashMap<ServerName, Arc<dyn McpServer>> }` with `register(name, Arc<dyn McpServer>) -> Result<()>` (vacant → insert, occupied → `Err(ServerNameTaken)`), `unregister(&str) -> Option<Arc<dyn McpServer>>`, and `get(&str) -> Option<Arc<dyn McpServer>>`. Single global namespace; first registration wins.

## Acceptance Criteria
- [x] `McpServer` trait, `ToolMetadata`, `CallerId`, `ServerRegistry` exist and are exported.
- [x] `ServerRegistry::register` returns `Err(ServerNameTaken)` on a duplicate name and `Ok` on a fresh name.
- [x] `unregister` removes and returns the server; `get` returns `None` after unregister.

## Tests
- [x] Unit tests with a trivial in-test `McpServer` impl (a fake that returns a fixed `tools()` and echoes `invoke`): register two distinct names succeeds; registering a taken name errors with `ServerNameTaken`; unregister-then-get yields `None`.
- [x] Run: `cargo test -p swissarmyhammer-plugin` — all green.

## Workflow
- Use `/tdd` — registry behavior tests first, then implement.

## Depends on
swissarmyhammer-plugin crate scaffold.