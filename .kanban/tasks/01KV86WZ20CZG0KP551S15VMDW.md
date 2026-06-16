---
assignees:
- claude-code
position_column: todo
position_ordinal: a880
project: diagnostics
title: 'Inline-on-edit: mutated_paths + shared diagnostics fold-in step'
---
## What
When a write op mutates a file, its own result carries diagnostics — no hook, no config, model-facing in every host (a tool's own return value always reaches the model; confirmed: llama-agent appends `ToolResult.result` JSON straight into the conversation at `agent.rs:1142/1158`). Today there is NO `mutated_paths` and NO diagnostics fold-in.

**Dispatch chokepoint (corrected location).** The single place all MCP tool calls funnel through is `crates/swissarmyhammer-tools/src/mcp/server.rs` — NOT `unified_server.rs` (which has no dispatch). Two paths call `tool.execute(...)`:
- `ServerHandler::call_tool` at ~`server.rs:1888` (the live MCP path) — it already has a post-call result-handling block (~1888-1929: `is_error`/`format_call_result_text`/`tool_call complete`). The fold-in extends THIS existing block.
- `McpServer::execute_tool` at ~`server.rs:1025` (a second entry path) — must share the same fold-in step (route both through one helper, don't duplicate).

**The core mechanism is a typed `mutated_paths` channel (not an afterthought).** `McpTool::execute` returns `CallToolResult` = `Vec<Content>` (text/JSON blocks) — the chokepoint canNOT see a typed `mutated_paths`, and `EditResult` (`files/edit/mod.rs:61`) is serialized into content while `write` returns a bare `usize`. So this card's central work is introducing a typed side-channel: every mutator declares `mutated_paths: Vec<PathBuf>` in a typed envelope the chokepoint reads WITHOUT string-parsing content. Decide where the field lives (e.g. an extended result/envelope type or a `ToolContext` out-param the tool fills), how `execute` populates it, and how the chokepoint reads it.

- Add `mutated_paths` to the typed channel; have `edit`/`write` populate it (extensible to any future mutator).
- In the shared fold-in helper called by both `server.rs` paths: for diagnosable paths, call `swissarmyhammer_diagnostics::diagnose()` and fold the `DiagnosticsReport` into the op result JSON.
- **Gating** (design "when appropriate"): diagnosable language only via the shared `is_diagnosable(path)` helper (see crate task 9b7tq7x) — `.md`/`.txt` attaches nothing; settle generously, `pending` on timeout; severity/scope = edited file always + capped broken one-hop dependents inline.
- Keep output sharp (guardrail k=1): compute the full blast radius, return only what broke.

## Depends on
- "diagnose(paths) core API with capped broken-dependents" (9fq036d)
- "Create swissarmyhammer-diagnostics crate..." (9b7tq7x) — for the `is_diagnosable(path)` helper
- "diagnostics MCP tool..." (na6cvh0) — shares the diagnose plumbing / config wiring

## Acceptance Criteria
- [ ] A typed `mutated_paths` channel exists; `edit` and `write` populate it; the chokepoint reads it without parsing `CallToolResult` content.
- [ ] A single shared fold-in helper runs from BOTH `server.rs:call_tool` (~1888 post-call block) and `server.rs:execute_tool` (~1025); no per-op duplication.
- [ ] A `.rs` edit returns inline diagnostics in the op result; a `.md`/`.txt` edit attaches nothing (via `is_diagnosable`).
- [ ] Non-quiescent analysis yields `pending` rather than blocking indefinitely.
- [ ] Edited file plus only broken capped dependents appear; no project-wide dump.

## Tests
- [ ] `cargo test -p swissarmyhammer-tools`: edit op on a fixture `.rs` with an injected error (mock diagnose) returns a result whose JSON contains the diagnostics; edit on `.md` returns none; timeout path yields `pending`. Assert BOTH `call_tool` and `execute_tool` paths fold in via the same helper, and that a stub non-file mutator reporting `mutated_paths` also gets diagnostics.
- [ ] Integration (gated on rust-analyzer): real `files edit` introducing a type error returns the error inline in the tool result.

## Workflow
- Use `/tdd` — write the "edit .rs result contains diagnostics / edit .md does not" test first, then build the typed channel + the shared `server.rs` fold-in helper. #diagnostics