---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffff480
project: remove-prompts
title: Remove MCP prompt protocol surface (list_prompts/get_prompt) from the server
---
## What
Remove the rmcp MCP-protocol prompt endpoints that expose sah "prompts" to MCP clients. Keep the internal `PromptLibrary` (skills/agents render through it) — this task removes only the protocol-facing prompt capability, not the rendering engine.

Files to edit in `crates/swissarmyhammer-tools/src/mcp/server.rs`:
- Remove the rmcp trait impls `async fn list_prompts(...) -> ListPromptsResult` and `async fn get_prompt(...) -> GetPromptResult`. (The "paginated handler ~line 1977" was `list_tools`'s own `PaginatedRequestParams` arg, not a prompt handler — left intact.)
- Remove `caps.prompts = Some(PromptsCapability { ... })` so the server no longer advertises the prompts capability.
- Remove the public helper methods `McpServer::list_prompts` and `McpServer::get_prompt`. Verified via callgraph: their only callers were in-crate tests + rmcp client-peer forwarding in the proxy/llama crates (out of scope).
- KEPT `load_all_prompts`-driven `initialize`/`reload_prompts_internal`: confirmed `load_all_prompts` loads `_partials` into the library, which skill/agent rendering needs.
- Removed the now-unused private helpers `validate_prompt_access`, `render_prompt_with_args`, and the server-local `json_map_to_string_map` (only fed the removed get_prompt path; a public `json_map_to_string_map` remains in utils.rs).
- Removed the now-unused `is_prompt_visible` import (left `is_prompt_visible` itself in the common crate for the coordinating cleanup task).

## Acceptance Criteria
- [x] The MCP server no longer advertises `prompts` capability in its `initialize` response.
- [x] No `list_prompts` / `get_prompt` rmcp handlers remain in `server.rs`.
- [x] Skill and agent tools still render correctly (partials still load).
- [x] `cargo build -p swissarmyhammer-tools` succeeds.

## Tests
- [x] Added MCP test asserting `initialize` capabilities do NOT include `prompts` (`test_create_server_capabilities`, `test_mcp_server_does_not_advertise_prompts_capability`, `test_mcp_server_advertises_tools_but_not_prompts_capability`).
- [x] Kept skill-render integration tests green (`mcp::tools::skill` — 21 pass, incl. delegate-partial rendering); added `test_builtin_partials_load_into_library_for_rendering` to prove partial loading survived.
- [x] `cargo test -p swissarmyhammer-tools mcp::` is green (1038 passed, 0 failed). Modified integration tests pass: `rmcp_integration`, `rmcp_stdio_working`, CLI `mcp_integration`.

## Workflow
- Used `/tdd` — wrote the "no prompts capability" assertion first (watched it fail), ran callgraph/grep checks before each deletion.

## Review Findings (2026-06-07 09:49)

_Scope note: the warning below targets `ServerHandler::call_tool`, which is NOT in this task's diff (the prompt-protocol removal did not touch `call_tool`). It is a pre-existing-code observation, not a defect introduced by this change. All four acceptance criteria re-verified clean: build exit 0, capability-absence tests pass, partial-loading test passes, `mcp::` test suite green._

### Warnings
- [ ] `crates/swissarmyhammer-tools/src/mcp/server.rs:1764` — `ServerHandler::call_tool` runs ~90 lines of actual code (1764–1926, ~160 physical lines), far over the ~50-line limit. It interleaves four distinct concerns — argument-preview logging, tool lookup, progress-token/context preparation, handler dispatch, and response-preview logging plus the four-phase timing — in one body, which is hard to read and impossible to unit-test in isolation. Extract the instrumentation into helpers that return the values the body needs, e.g. `log_call_args(&tool_name, request.arguments.as_ref())`, `resolve_progress_token(&context, &request)`, and `log_call_result(&tool_name, &result, timings)`. That leaves `call_tool` as lookup → context-prep → `tool.execute()` → return, comfortably under 50 lines, with the logging/timing logic independently testable.

## Review Findings (2026-06-08 06:40)

_Scope note: this review ran `review working` over the full uncommitted working tree, which spans the broader `remove-prompts` project, not solely this task's `server.rs` scope. The findings below in `cli_integration.rs`, `context.rs`, and the `rmcp_*` integration tests touch CLI-command-removal work that belongs to sibling tasks — they are NOT defects introduced by this task's prompt-protocol-handler removal in `server.rs`. The only in-scope item is the `tests.rs` nit (duplicated capability-absence test added by this task). The pre-existing `call_tool` length finding from 2026-06-07 is out of scope per the task's own scope note and remains so._

### Blockers
- [x] `apps/swissarmyhammer-cli/tests/integration/cli_integration.rs:54` — RESOLVED: replaced the always-true `result.exit_code >= 0` with a concrete `assert_eq!(exit_code, 0, ...)` per concurrent `validate` run, and wrapped the test body in `IsolatedTestEnvironment::new()` so the assertion holds in a clean environment (matching the sibling `test_verbose_flag`/`test_quiet_flag` pattern). Test passes.
- [x] `crates/swissarmyhammer-tools/tests/integration/rmcp_stdio_working.rs:18` — RESOLVED by sibling task 01KTC7GT2DQ84KH443BRC75SHJ via shared `test_utils` helper: this file now calls `start_test_server_and_client()` (extracted into `crates/swissarmyhammer-tools/src/mcp/test_utils.rs`), removing the duplicated isolated-temp-dir server/client bootstrap. Verified in place; test passes.
- [x] `crates/swissarmyhammer-tools/tests/rmcp_integration.rs:20` — RESOLVED by sibling task 01KTC7GT2DQ84KH443BRC75SHJ via shared `test_utils` helper: this file likewise calls `start_test_server_and_client()`, so the two rmcp smoke tests share one bootstrap and differ only in their own `files` op/pattern and `shell` assertion. Verified in place; test passes.

### Warnings
- [x] `apps/swissarmyhammer-cli/src/context.rs:206` — RESOLVED (deleted): grepped the whole workspace for `render_prompt` — the only references to `CliContext::render_prompt` were its own definition and the `test_cli_context_render_prompt_nonexistent` test (the other matches are unrelated: `mirdan/src/search.rs`, `avp-common/.../runner.rs`, `swissarmyhammer-prompts` lib, and `cli/src/test.rs`'s separate `render_prompt_with_env`). No production caller exists. Deleted `render_prompt` (and its `#[allow(dead_code)]` + comment-promise) and the `test_cli_context_render_prompt_nonexistent` test; git history preserves them. Downstream rendering tasks operate on the rendering library crate, not `CliContext`.
- [x] `apps/swissarmyhammer-cli/src/context.rs:341` — RESOLVED: changed `test_mode: self.quiet.unwrap_or_default()` to `test_mode: self.test_mode.unwrap_or_default()` so the field reads from its own builder setter rather than mirroring `quiet`.

### Nits
- [x] `apps/swissarmyhammer-cli/src/context.rs:229` — RESOLVED: changed `display<T>(&self, items: Vec<T>)` (and the private `display_as_table`/`display_as_json`/`display_as_yaml` helpers) to take `items: &[T]`, dropping the redundant `&` in the serde calls. Updated the two production callers in `commands/serve/mod.rs` (`&basic_status`, `&verbose_status`) and the four `context.rs` test callers. Callers can now pass slices without allocating.
- [x] `crates/swissarmyhammer-tools/src/mcp/tests.rs:80` — RESOLVED: deleted the redundant `test_mcp_server_does_not_advertise_prompts_capability`; `test_mcp_server_advertises_tools_but_not_prompts_capability` already asserts the same prompts-absence plus the tools capability.

## Review Findings (2026-06-08 10:00)

_Scope note: this `review working` swept the full uncommitted tree (the broader `remove-prompts` project). Triage against THIS task's scope (prompt-protocol surface removal in `server.rs` + its own capability/library tests): the `serve/mod.rs`, `context.rs`, and `cli.rs` findings belong to the sibling CLI-removal task 01KTC7GT2DQ84KH443BRC75SHJ (done) — not introduced here. The `server.rs:1764` `call_tool` warning is the same pre-existing out-of-scope finding from 2026-06-07 and remains out of scope (the prompt-protocol removal did not touch `call_tool`). The one genuinely in-scope item is the `server.rs:2493` nit on this task's newly-added `test_initialize_loads_prompts_into_library`._

### Warnings
- [ ] `apps/swissarmyhammer-cli/src/commands/serve/mod.rs:422` — The host literal "127.0.0.1" is hardcoded as the fallback in the changed display_verbose_server_status, but the same literal is repeated across this file (lines 93, 97, 99, 115, 422). A scattered default address gets missed when it changes — e.g. if the bind host ever moves, the health-URL fallback here silently drifts out of sync. Introduce a `const DEFAULT_HOST: &str = "127.0.0.1";` (module level) and reference it at every site, including the `.unwrap_or(DEFAULT_HOST)` fallback here, so the default address changes in one place.
- [ ] `apps/swissarmyhammer-cli/src/context.rs:14` — `map_error` flattens the error chain — it formats the underlying error into a `String` (`format!("{}: {}", context, e)`) and wraps it in `SwissArmyHammerError::Other { message }`. The resulting error has no `source()`, so every display/serialization failure routed through it (`display_as_json`, `display_as_yaml`, `display_as_table`, `get_prompt_library`) loses the original error as a matchable/inspectable source. The error-handling rule explicitly requires `Error::source()` chains to exist for wrapped errors. Either return `anyhow::Result` with `.context("...")` (CLI = application code), preserving the chain; or wrap in a typed variant that carries the source, e.g. `SwissArmyHammerError::Serialization { context, #[source] source }`, so `source()` walks to the original `serde`/IO error.
- [ ] `crates/swissarmyhammer-tools/src/mcp/server.rs:1764` — `call_tool` is a ~160-line handler that interleaves the real request flow (registry lookup, progress-token plumbing, dispatch, execute) with three independent conditional-logging blocks, each gated on its own `tracing::enabled!` check. A reader tracing the actual control flow must repeatedly step over logging instrumentation, and the args-preview block nests three control levels deep (`if let Some(args)` → `else if INFO` → `if total_bytes > preview.len()`). The branch density makes the function hard to reason about and obscures the four phases the comments describe. Extract the two logging concerns into helpers — e.g. `log_tool_call_args(&tool_name, request.arguments.as_ref())` and `log_tool_call_result(&tool_name, &result, &timings)` — so `call_tool` reads as the four labeled phases (parse → dispatch → handler → response) without the inline tracing-enablement nesting.

### Nits
- [ ] `apps/swissarmyhammer-cli/src/cli.rs:15` — Public enum `OutputFormat` has no doc comment, unlike its sibling public enums in the same file which are all documented. The inconsistency leaves a public CLI type undocumented. Add a `///` doc comment describing that this is the global output format selector for CLI commands (table/json/yaml).
- [ ] `apps/swissarmyhammer-cli/src/cli.rs:126` — Public enum `Commands` (the top-level subcommand dispatch type) has no type-level doc comment. While each variant is documented, the type itself lacks a summary. Add a `///` doc comment on the enum, e.g. describing it as the set of top-level `sah` subcommands.
- [x] `crates/swissarmyhammer-tools/src/mcp/server.rs:2493` — RESOLVED: rewrote `test_initialize_loads_prompts_into_library` to assert the library is non-empty immediately after `initialize()`, before any reload call — removing the `reload_prompts()` invocation that duplicated the reload tests. The test now exercises only the initialize() load path it claims to test. Was a near-duplicate of the existing reload test: it creates a server, calls reload_prompts(), and asserts the library is non-empty — the same mechanism the reload tests already exercise. Its stated subject (initialize() populating the library) is proven only indirectly via reload, so the unique coverage it adds over the reload tests is thin. Assert on what initialize() uniquely does (e.g. that the library is non-empty immediately after initialize(), before any reload) instead of re-running reload_prompts(); that removes the overlap with the reload tests and makes the test actually about initialize().