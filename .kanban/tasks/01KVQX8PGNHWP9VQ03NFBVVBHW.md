---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvqxefqg26534h18zvb55z7y
  text: /finish picked up this task (single-task mode). Driving implement → test → review → commit loop. No prior comments. Starting with /implement.
  timestamp: 2026-06-22T14:58:41.520786+00:00
- actor: claude-code
  id: 01kvqy9sk00caybxefq49jms9a
  text: |-
    Implemented via /tdd. Production change (2 lines) in apps/swissarmyhammer-cli/src/mcp_integration.rs: added register_review_tools to the swissarmyhammer_tools use-import block and called register_review_tools(&mut tool_registry) in create_tool_registry() after register_web_tools. Tests added to apps/swissarmyhammer-cli/tests/integration/cli_mcp_integration.rs: review_exposed_as_sah_tool_category and review_list_validators_executes_via_cli.

    RED→GREEN verified with forced rebuilds (nextest reuses stale binaries unless the source is touched — watch for this):
    - Reverting the registration line: Test 1 FAILS ("review" absent from the 7 categories), Test 2 PASSES.
    - Restored: both PASS (2 passed, 171 skipped).

    Acceptance criteria all met: get_cli_categories() contains "review" + get_tools_for_category("review") non-empty; ./target/release/sah tool --help now lists all 8 categories including review; execute_tool("review",{op:"list validators"}) returns non-error; existing 7 categories unchanged (no regression). cargo fmt clean, cargo clippy -p swissarmyhammer-cli -- -D warnings clean.

    really-done adversarial double-check returned REVISE (advisory) with 2 findings, both addressed/justified:
    - Finding 1 (Test 2 path-independent of the fix): CONFIRMED empirically — Test 2 routes through the backing McpServer (register_all_tools, server.rs:944), not create_tool_registry(), so it passes with or without the fix. Proceeding as-is: Test 2 is the card's explicitly-prescribed test for acceptance criterion 3, and the card's own Scope note documents that execution flows through the backing server. Test 1 is the true RED→GREEN guard and is proven. Acceptable per the card's specification.
    - Finding 2 (RED evidence): addressed — captured the revert→RED→restore→GREEN cycle above.

    Moving to review.
  timestamp: 2026-06-22T15:13:36.352640+00:00
- actor: claude-code
  id: 01kvqydw9xyh8zap84bvkpr2gh
  text: 'Implement landed in `review`. Tests verified green: `cargo nextest run -p swissarmyhammer-cli` → 842 passed, 0 skipped; clippy `-D warnings` clean; `cargo fmt --check` clean. Files: mcp_integration.rs (2-line registration) + cli_mcp_integration.rs (2 new tests). Proceeding to /review.'
  timestamp: 2026-06-22T15:15:50.205136+00:00
- actor: claude-code
  id: 01kvqypmgznj8a386an8cbc4qt
  text: 'Review verdict: findings (0 blockers, 3 warnings, 1 nit) — all on PRE-EXISTING code in mcp_integration.rs (CliToolContext Debug, constructor error type, get_tool_registry_arc naming, create_arguments signature), none implicating the 2-line registration fix or the new tests. Dispatching /implement iteration 2 to action/justify the findings: apply trivial safe in-file fixes; justify (don''t refactor) the get_tool_registry_arc rename since it has cross-file blast radius (main.rs, validate.rs, cli_executor.rs) and is out of this task''s scope.'
  timestamp: 2026-06-22T15:20:37.151472+00:00
- actor: claude-code
  id: 01kvqz5fc1vy4ffftahgvqm3qg
  text: |-
    Iteration 2: worked the 4 Review Findings (2026-06-22 10:16), all on PRE-EXISTING code in apps/swissarmyhammer-cli/src/mcp_integration.rs (not the 2-line registration). Followed /tdd.

    FIXED in-file (2):
    - Finding 1 (CliToolContext lacks Debug): added a manual `impl std::fmt::Debug for CliToolContext`. #[derive(Debug)] is impossible — ToolRegistry (Box<dyn McpTool>) and McpServer don't implement Debug, and touching those foreign types is out of this task's one-file scope. Manual impl uses opaque placeholders ("<ToolRegistry>", "<McpServer>") and forwards mcp_server_handle (McpServerHandle already has a Debug impl at unified_server.rs:335). Guarded by new test test_cli_tool_context_implements_debug — RED verified (pre-fix: E0277 "doesn't implement std::fmt::Debug"), GREEN after.
    - Finding 4 nit (create_arguments took Vec): widened signature to `pub fn create_arguments<'a>(&self, args: impl IntoIterator<Item = (&'a str, Value)>)`. Backward-compatible — every existing vec![...]/vec![] caller across main.rs/validate.rs/cli_executor.rs/tests still compiles (Vec is IntoIterator). Guarded by new test test_create_arguments_accepts_array — RED verified (pre-fix: E0308 expected Vec, found [_;2] on an array literal), GREEN after.

    JUSTIFIED out-of-scope, NOT changed (2):
    - Finding 2 (new returns Box<dyn Error>): pre-existing public API on new/new_isolated/new_with_config/initialize_mcp_server; switching to anyhow::Result<T> ripples to all callers' error handling. Cross-file refactor, out of scope. Logged as a follow-up card item in the description.
    - Finding 3 (get_tool_registry_arc get_ prefix): rename has 13 call sites across main.rs (8), validate.rs, cli_executor.rs, tests. Mechanical cross-file sweep, out of scope. Logged as a follow-up card item.

    All 4 finding checkboxes flipped to [x] with justification notes in the description.

    really-done gate GREEN: cargo fmt --check (exit 0); cargo clippy -p swissarmyhammer-cli --all-targets -- -D warnings (exit 0); cargo nextest run -p swissarmyhammer-cli (844 passed, 0 skipped). Adversarial double-check returned PASS; its one cosmetic note (Debug doc comment lumping "task handles" with both foreign types) was addressed by tightening the comment. Moving back to review.
  timestamp: 2026-06-22T15:28:43.393495+00:00
- actor: claude-code
  id: 01kvqzhgrvnck7fwgn1mwh9snq
  text: 'DONE. Loop complete: implement → tests (844 passed) → review #1 (4 findings on pre-existing code) → implement #2 (2 fixed: manual Debug impl + create_arguments IntoIterator; 2 justified out-of-scope: error-type + getter-rename, captured as follow-up) → review #2 (clean) → done. Local rollback-point commit 4795366d5 (NOT pushed). No persistent findings hit the guardrail.'
  timestamp: 2026-06-22T15:35:18.043002+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffd180
project: local-review
title: Expose `review` as a `sah tool` — register it in the CLI's create_tool_registry
---
## What

The `sah tool` command tree exposes only 7 of the registered tool categories. `review` is **absent** — confirmed by running `./target/release/sah tool --help`, which lists only: `code_context`, `files`, `git`, `kanban`, `question`, `shell`, `web`.

**Root cause:** the CLI builds its `sah tool` subcommand tree from the registry returned by `CliToolContext::create_tool_registry()` in `apps/swissarmyhammer-cli/src/mcp_integration.rs` (the function body is at lines 130–140). That function has drifted from the canonical `register_all_tools()` in `crates/swissarmyhammer-tools/src/mcp/server.rs` (lines 929–946) — its own doc comment (`mcp_integration.rs:129`) says it "should mirror" `register_all_tools`. It calls `register_code_context_tools`, `register_file_tools`, `register_git_tools`, `register_kanban_tools`, `register_questions_tools`, `register_shell_tools`, `register_web_tools` — but **not** `register_review_tools`. (`register_all_tools` does call it at `server.rs:944`.)

**Fix** — in `apps/swissarmyhammer-cli/src/mcp_integration.rs`:
1. Add `register_review_tools` to the `use swissarmyhammer_tools::{ ... }` import block (currently lines 16–19). It is re-exported at the crate root — see `crates/swissarmyhammer-tools/src/lib.rs:77`.
2. Inside `create_tool_registry()` (after the existing `register_web_tools(&mut tool_registry);` call, ~line 138), add `register_review_tools(&mut tool_registry);`.

The `review` tool already implements `cli_category()` → `Some("review")` and `operations()` (`crates/swissarmyhammer-tools/src/mcp/tools/review/mod.rs:437,441`), so once registered it appears in the tree automatically — no further wiring needed for discovery.

**Scope note (intentional boundary):** This exposes the `review` category and makes the loader-read ops (`list validators`, `get validator`, `check validators`) fully functional — they need no agent factory (`review/mod.rs:485–505`). Tool execution flows through `CliToolContext::execute_tool` → `resolve_server()` → the backing `McpServer`, which already registers `review` via `register_all_tools` (`server.rs:944`), so the loader ops run immediately once the CLI tree exposes the category. The three pipeline ops (`review working` / `review file` / `review sha`) will appear in the tree but still return the actionable error `the review ops need a live agent; this tool was built without an agent factory` (`review/mod.rs:305–311`) until the review agent factory is wired into the CLI's backing server (as `apps/swissarmyhammer-cli/src/commands/serve/mod.rs::wire_review_factories` does for `sah serve`). That wiring is a separate concern — see Follow-up.

## Acceptance Criteria
- [x] The registry from `CliToolContext::create_tool_registry()` reports `"review"` in `get_cli_categories()`, and `get_tools_for_category("review")` is non-empty.
- [x] `sah tool --help` lists `review` among the commands (alongside the existing 7).
- [x] `CliToolContext::execute_tool("review", { "op": "list validators" })` returns a non-error `CallToolResult` (validator summaries), not an unknown-tool or parse error.
- [x] The existing 7 categories (`code_context`, `files`, `git`, `kanban`, `question`, `shell`, `web`) remain present — no regression.

## Tests
- [x] Add `review_exposed_as_sah_tool_category` to `apps/swissarmyhammer-cli/tests/integration/cli_mcp_integration.rs`: build a context via `CliToolContext::new_isolated(&temp_path)`, take `context.get_tool_registry_arc().read().await`, and assert `get_cli_categories()` contains `"review".to_string()` and `get_tools_for_category("review")` is non-empty. (Mirrors the category assertions in `tests/integration/mcp_tools_registration.rs:116–139`.) This is RED before the fix (review absent) and GREEN after.
- [x] Add `review_list_validators_executes_via_cli` to the same file: `let args = context.create_arguments(vec![("op", json!("list validators"))]); let result = context.execute_tool("review", args).await;` then assert `result.is_ok()` and the returned `CallToolResult.is_error` is not `Some(true)`. (Mirrors the `files` / `read file` execution test at `cli_mcp_integration.rs:86–91`.)
- [x] Run: `cargo test -p swissarmyhammer-cli --test cli_tests review_exposed_as_sah_tool_category review_list_validators_executes_via_cli` → both pass. (The integration modules are aggregated under the `cli_tests` binary via `tests/cli_tests.rs` → `mod integration`.)

## Workflow
- Use `/tdd` — write the two failing integration tests first and confirm RED (`review` not in categories / unknown tool), then add the two-line registration to make them pass.

## Follow-up (suggest `/plan`, out of scope here)
- `create_tool_registry` also omits `register_ralph_tools`, `register_diagnostics_tools`, `register_agent_tools`, and `register_skill_tools` — the same drift, which breaks `sah tool ralph` (used by the `finish` / `test-loop` skills), `sah tool diagnostics`, etc. Worth a guard test that asserts `create_tool_registry()`'s category set matches `register_all_tools()`.
- Wire the review agent factory into the CLI tool context's backing server so `sah tool review review working|file|sah` actually execute (mirror `commands/serve/mod.rs::wire_review_factories` + `review_agent_factory`). #bug #cli
- Error-type / naming sweep of `mcp_integration.rs` (deferred from the review findings below): switch `new`/`new_isolated`/`new_with_config`/`initialize_mcp_server` from `Box<dyn std::error::Error>` to `anyhow::Result<T>` with `.context(...)`, and rename `get_tool_registry_arc()` → `tool_registry_arc()` across all ~13 call sites (`main.rs`, `validate.rs`, `cli_executor.rs`, tests). Both are cross-file refactors out of scope for the registration fix.

## Review Findings (2026-06-22 10:16)

### Warnings
- [x] `apps/swissarmyhammer-cli/src/mcp_integration.rs:23` — Public struct `CliToolContext` does not implement `Debug`. FIXED in-file: added a manual `impl std::fmt::Debug for CliToolContext`. A `#[derive(Debug)]` is impossible — its field types `ToolRegistry` (holds `Box<dyn McpTool>`) and `McpServer` (holds task handles/trait objects) do not implement `Debug`, and per the task's "no bonus refactoring / stay in one file" scope I did not touch those foreign types. The manual impl renders those two fields as opaque placeholders (`"<ToolRegistry>"`, `"<McpServer>"`) and forwards `mcp_server_handle` (which already has a manual `Debug` impl in `unified_server.rs:335`). Guarded by `test_cli_tool_context_implements_debug` (RED→GREEN verified: pre-fix `E0277 doesn't implement std::fmt::Debug`).
- [x] `apps/swissarmyhammer-cli/src/mcp_integration.rs:33` — `new()` returns `Result<Self, Box<dyn std::error::Error>>`. JUSTIFIED out-of-scope (do NOT change): this is pre-existing public API on `CliToolContext::new` / `new_isolated` / `new_with_config` / `initialize_mcp_server`. Switching to `anyhow::Result<T>` changes the public error type and ripples to every caller's `?`/error handling across `main.rs`, `validate.rs`, `cli_executor.rs`, and tests — explicitly outside this task's stated scope ("no bonus refactoring", one-file change). Not the registration fix under review. Captured as a follow-up card item above.
- [x] `apps/swissarmyhammer-cli/src/mcp_integration.rs:153` — `get_tool_registry_arc()` getter naming (`get_` prefix). JUSTIFIED out-of-scope (do NOT rename): renaming to `tool_registry_arc()` has cross-file blast radius — 13 call sites across `main.rs` (8), `validate.rs`, `cli_executor.rs`, and the in-file + integration tests. That is a mechanical rename sweep beyond this task's "no bonus refactoring", one-file scope, and unrelated to the `review` registration fix under review. Captured as a follow-up card item above.

### Nits
- [x] `apps/swissarmyhammer-cli/src/mcp_integration.rs:149` — `create_arguments()` took `Vec<(&str, Value)>`. FIXED in-file (trivial, breaks no callers): changed signature to `pub fn create_arguments<'a>(&self, args: impl IntoIterator<Item = (&'a str, Value)>) -> Map<String, Value>`. Every existing `vec![...]` caller still compiles (a `Vec` is `IntoIterator`); confirmed across `main.rs`/`validate.rs`/`cli_executor.rs`/all tests. Guarded by `test_create_arguments_accepts_array` (RED→GREEN verified: pre-fix `E0308 expected Vec, found [_; 2]` when passing an array literal).