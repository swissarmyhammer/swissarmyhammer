---
assignees:
- wballard
position_column: done
position_ordinal: ffffffffffffffffffffffff8380
title: 'Tool name mismatch: rule prompts say `read_file`, MCP serves `files` (with op argument)'
---
**Blocker for end-to-end qwen-as-validator.** Even after the always-on validator MCP server (`01KQ35MHFJQPMEKQ08PZKBKFY0`) lands, the rule prompts will advertise tool names that the MCP registry doesn't actually serve.

## The mismatch

`FilesTool::name()` (`swissarmyhammer-tools/src/mcp/tools/files/mod.rs:99-101`) returns the literal string `"files"`. It's a single tool that takes an `op` argument: `{"op": "read file"}`, `{"op": "glob files"}`, `{"op": "grep files"}`. The MCP `tools/list` endpoint returns one tool named `files`.

The integration test `llama-agent/tests/integration/tool_call_round_trip.rs` and the rule prompts in `builtin/validators/**/*.md` both reference a tool named `read_file`:

- The round-trip test asserts `tool_calls[0].name == "read_file"` after a Qwen3-0.6B run.
- Rule prompts say: *"You may have access to **files** (read file, glob, grep — read-only)..."* — easily read by a Hermes-trained model as: tool name is `read_file` (or `read file`).

Models trained on Hermes-style tool schemas (Qwen3 family) call tools by name. They will emit:

```
<tool_call>{"name": "read_file", "arguments": {"path": "..."}}</tool_call>
```

The validator MCP server has no tool by that name. Tool-not-found error. Validator stalls or hallucinates a recovery, doesn't actually read the file.

## Two ways to fix — pick one

### Option A: Split `FilesTool` into per-operation tools (recommended)

Replace the single `FilesTool` in the validator registry with three separate tool implementations:
- `ReadFileTool` (name `read_file`, schema: `{path: string}`)
- `GlobFilesTool` (name `glob_files`, schema: `{pattern: string, path: string}`)
- `GrepFilesTool` (name `grep_files`, schema: `{pattern: string, path: string, regex: bool}`)

Each is a thin wrapper around the existing `read::execute_read`, `glob::execute_glob`, `grep::execute_grep` handlers in `tools/files/`. The `FilesTool::read_only()` constructor stays (for backward compat with any existing callers), but the **validator registry** uses the three split tools instead.

Why this is the better answer: the names match what models naturally emit. No prompt-engineering required. The `op`-dispatched form is a CLI convenience, not the right shape for an MCP tool surface.

The non-validator `FilesTool` (the unified one with `op` argument) keeps its current shape — only the validator path uses the split form.

### Option B: Rewrite rule prompts to be explicit about the unified `files` tool

Replace the boilerplate "Available Tools" section in every `builtin/validators/**/*.md` with:

```markdown
## Available Tools

You have access to a `files` tool with read-only operations. To read a file, call:

```json
{"name": "files", "arguments": {"op": "read file", "path": "/absolute/path"}}
```

To search file content, use `{"op": "grep files", ...}`. To find files by pattern, use `{"op": "glob files", ...}`.
```

Trains the model away from the natural `read_file` shape and toward the `files`-with-op shape. Less elegant but no Rust changes.

**Recommendation: Option A.** It aligns with how models are trained, removes a cognitive tax on the prompt, and matches the round-trip test's existing assumption (which is *also* the right assumption — `tool_calls[0].name == "read_file"`).

## What to change for Option A

1. New file `swissarmyhammer-tools/src/mcp/tools/files/read_file.rs` (or extend `read.rs`) with a `ReadFileTool` struct that implements `McpTool` with `name() = "read_file"`, schema for `{path: string}`, `is_validator_tool() = true`.
2. Same pattern for `GlobFilesTool` (`glob_files`) and `GrepFilesTool` (`grep_files`).
3. New `register_validator_file_tools(registry)` (or fold into `build_validator_tool_registry` in the tools task) that registers the three split tools.
4. The validator-only registration helper (introduced by `01KQ35MHFJQPMEKQ08PZKBKFY0`) calls `register_validator_file_tools` instead of `registry.register(FilesTool::read_only())`.
5. Update `01KQ7G1R9KRQ8RDBKYVSNEN9V4`'s expected tool-list allowlist from `{"files"}` to `{"read_file", "glob_files", "grep_files"}`.

## Tests

- Unit tests for each new tool (`ReadFileTool::execute` round-trip, etc.). Mostly mirror existing tests in `tools/files/{read,glob,grep}.rs`.
- The existing `tool_call_round_trip.rs` integration test (which already asserts `name == "read_file"`) should now pass against the *real* validator MCP server, not just an in-process injected `ToolDefinition`. Update the test to drive the agent through the MCP connection rather than direct injection — see `01KQ7GA8KZRTH3D7PYQTM7HJ9F` (the MCP-fetch verification task) for the deeper change.
- Rule-prompt wording sweep — `01KQ7GA8KZ...` (the rule-prompt task) updates the rule prompts to reference the split tool names accurately.

## Acceptance

- `tools/list` against the validator MCP server returns `{"read_file", "glob_files", "grep_files", <code_context tools...>}` — no `files` (unified) entry.
- Round-trip test in `llama-agent/tests/integration/tool_call_round_trip.rs` passes against the real MCP server (not just an injected `ToolDefinition`).
- A Stop-hook qwen run shows qwen calling `read_file` (the split tool name), the call succeeds, the result flows back, and the rule's verdict references the file content.

## Pairs with

- `01KQ35MHFJQPMEKQ08PZKBKFY0` — the always-on validator MCP server. The split tools live inside its registry.
- `01KQ7G1R9KRQ8RDBKYVSNEN9V4` — the verification task's allowlist needs updating.
- The MCP-fetch verification card (filed alongside this) — without that, splitting tools doesn't help if llama-agent never fetches them. #avp

## Review Findings (2026-04-27 14:58)

### Warnings
- [x] `swissarmyhammer-tools/src/mcp/tools/files/grep_files.rs:71-74` — `context_lines` is advertised in the wrapper's JSON schema, but the underlying `GrepRequest` in `swissarmyhammer-tools/src/mcp/tools/files/grep/mod.rs:152-153` carries `#[allow(dead_code)]` and `execute_grep` never honors it. The split tool inherits and gives more visibility to a misleading documented capability — a Hermes-trained validator model that emits `{"context_lines": 3}` will silently get zero context lines, with no error to recover from. Either drop `context_lines` from the wrapper schema (clean fix at this layer) or wire it through the underlying handler. Same applies, less prominently, to the unified `files` tool's schema — but the split tool's schema is the new validator-facing surface and worth fixing here.
  - **Fix:** Removed `context_lines` from the `grep_files` wrapper schema and updated the description to drop the "context lines" mention. Added a `// NOTE` in the schema explaining when the field should be re-introduced (after `execute_grep` actually honors it). Added a regression test `test_schema_does_not_have_context_lines`. The unified `files` tool's schema is auto-generated from `GREP_FILES_PARAMS`; that surface is unchanged here per the reviewer's "less prominently … worth fixing here" guidance — fixing the underlying handler or scrubbing the unified schema can be a separate task if needed.

### Nits
- [x] `swissarmyhammer-tools/src/mcp/tools/files/read_file.rs:61` — Description says "Returns text content..." but `read::execute_read` automatically base64-encodes binary files (per the docstring on the legacy `read::ReadFileTool` at line 111 of `read/mod.rs`: "**Binary Support**: Automatic base64 encoding for binary files"). A validator model that needs to inspect a binary file will read this description and assume it can't. Suggest: "Read file contents from the local filesystem. Returns text for text files; binary files are returned as base64. Supports optional line-based offset and limit for partial reads."
  - **Fix:** Updated the description to the suggested wording so a validator model knows binary content is returned as base64.
- [x] `swissarmyhammer-tools/src/mcp/tools/files/read/mod.rs:124` — Legacy unused `ReadFileTool` struct remains, shadowed by the new `read_file::ReadFileTool` re-export at `mod.rs:25`. The legacy struct never implemented `McpTool`, was only a hand-rolled stub used to anchor a doc comment, and now has no callers (`grep -r read::ReadFileTool` finds only one rustdoc reference at `read/mod.rs:33`). Two `ReadFileTool` types in the same crate is a navigation hazard. Suggest deleting `pub struct ReadFileTool` (lines 123-131) and the dangling rustdoc reference at line 33; keep `pub struct ReadFile` (the `Operation` impl) and `pub async fn execute_read` since the new wrapper depends on them.
  - **Fix:** Deleted the legacy `pub struct ReadFileTool` (and its `impl new()`) from `read/mod.rs`. Moved the documentation block onto `execute_read` (the actual handler) and updated the module-level rustdoc to reference `execute_read` and the new `read_file::ReadFileTool` rather than the deleted stub. `pub struct ReadFile` (the `Operation` impl) and `pub async fn execute_read` are preserved.
- [x] `swissarmyhammer-tools/src/mcp/tools/files/mod.rs:261-263` and `mod.rs:286-290` — `register_file_tools` is `async`, `register_validator_file_tools` is `pub fn` (sync). Neither does any async work — both are simple `registry.register(...)` calls. The async on `register_file_tools` is misleading and the asymmetry forces callers to remember which is which. Suggest making `register_file_tools` synchronous to match. (Or, if there's a deeper async-context reason, document it.) This is a follow-up cleanup; the split tools work correctly as-is.
  - **Fix:** Made `register_file_tools` synchronous in both `tools/files/mod.rs` and the wrapper in `tool_registry.rs`. Updated all `.await` callers (`mcp/server.rs`, `mcp/tool_config.rs`, `health_registry.rs`, `mcp/tools/files/mod.rs` test, `tool_registry.rs::create_fully_registered_tool_registry`, `swissarmyhammer-cli/src/mcp_integration.rs`, `swissarmyhammer-cli/tests/integration/mcp_tools_registration.rs`, `tests/integration/file_size_limits.rs`, `tests/integration/file_tools_integrations.rs`) and the rustdoc examples in `lib.rs` and `mcp/mod.rs`. Added a doc comment explaining the sync convention. The unit test for `register_file_tools` is now `#[test]` instead of `#[tokio::test]`.
- [x] `swissarmyhammer-tools/src/mcp/tools/files/{read_file,glob_files,grep_files}.rs` — The three wrappers duplicate ~30 lines of identical boilerplate each (struct, `new()`, `Default`, `cli_category() = None`, `hidden_from_cli() = true`, `is_validator_tool() = true`, `ValidatorTool` marker, `Initializable`, `Doctorable`). A small declarative macro `validator_file_tool!(name, description, schema, dispatch)` would collapse three 100-line files to one definition site each. Not required — matches the prevailing per-tool-file convention in the codebase — but worth considering if a fourth split tool is ever added.
  - **Decision:** Reviewed and intentionally deferred. The reviewer explicitly notes "Not required — matches the prevailing per-tool-file convention in the codebase". The prevailing pattern in this crate is a separate file per MCP tool (see `tools/git/`, `tools/kanban/`, `tools/shell/` siblings). Introducing a one-off macro for only three call sites would deviate from that convention and add a navigation hazard (a hidden definition site behind a macro). If/when a fourth split validator tool is added, this trade-off can be revisited as a focused refactor.
