---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff8480
project: local-review
title: 'Stop truncating MCP tool-call logs: emit full args + full result, delete the truncation utilities'
---
## What
The MCP server's per-tool-call logging in `crates/swissarmyhammer-tools/src/mcp/server.rs` TRUNCATES log messages at INFO level — the `tool_call args` line caps args at `MAX_ARGS_BYTES_INFO` (512) with a `...[+N more bytes]` suffix, and the `tool_call complete` line caps the result preview at `MAX_PREVIEW_BYTES_INFO` (256) via `truncate_utf8_for_log`. The full payload is only emitted at TRACE. **The user has repeatedly and explicitly forbidden truncating log messages** — a truncated log is useless. Remove the truncation entirely; emit full payloads at INFO; and delete the truncation utilities so it cannot creep back.

This is NOT part of the review feature — it's the generic MCP tool-call logger (fires for every tool: kanban, git, etc.). It was introduced by commit `795debfe5`.

## Changes
In `crates/swissarmyhammer-tools/src/mcp/server.rs`:
- **`tool_call args` (the pre-call args log):** log the FULL serialized args at INFO — `serde_json::to_string(args)` — with NO byte cap and NO `...[+N more bytes]` suffix. Keep it gated behind `tracing::enabled!(Level::INFO)` so info-disabled runs don't allocate. The separate TRACE-only `args_full` branch is now redundant (INFO already logs full) — collapse to a single full-payload INFO log. Rename the field from `args_preview` to `args` (it is no longer a preview).
- **`tool_call complete` (the post-call line):** log the FULL result text — drop the `truncate_utf8_for_log(&preview, MAX_PREVIEW_BYTES_INFO)` call; emit the full joined result string directly. Rename the field from `preview` to `result`. Keep `result_bytes`.
- The separate TRACE-only `tool_call result (full payload)` branch is now redundant (INFO logs full) — remove it. (If you prefer to keep a single emission point, fine — the invariant is: the full result appears at INFO, untruncated, exactly once.)
- Rename `format_call_result_for_preview` → `format_call_result_text` (it joins text content blocks and returns `(total_bytes, full_text)`; it does NOT truncate — keep it, just drop the misleading "preview" name). Update its doc comment (remove "caller is responsible for truncating").
- Update the now-wrong comments that describe "info-level callers see a UTF-8-safe truncation" / "secret hygiene at info level" / "full payload only at trace level".
- Remove the now-unused import line `use ...::{serialize_json_bounded, truncate_utf8_for_log, MAX_ARGS_BYTES_INFO, MAX_PREVIEW_BYTES_INFO}`.

In `crates/swissarmyhammer-tools/src/mcp/tracing_util.rs`:
- Delete `truncate_utf8_for_log`, `serialize_json_bounded`, `MAX_ARGS_BYTES_INFO`, `MAX_PREVIEW_BYTES_INFO`, and ALL their unit tests. First grep the whole workspace to confirm nothing else uses them; if some other crate does, repoint/remove that usage too (the directive is project-wide: no log truncation anywhere). If the module ends up empty, delete the module file and its `mod tracing_util;` declaration.

## Acceptance Criteria
- [x] The `tool_call args` and `tool_call complete` INFO logs emit the FULL args and FULL result — no byte caps, no `...[+N more bytes]`, no `truncate_utf8_for_log`.
- [x] `truncate_utf8_for_log`, `serialize_json_bounded`, `MAX_ARGS_BYTES_INFO`, `MAX_PREVIEW_BYTES_INFO` no longer exist in the workspace (grep returns nothing); no truncation helper remains.
- [x] No dead code / unused imports; `cargo build -p swissarmyhammer-tools` and `cargo clippy -p swissarmyhammer-tools --all-targets -- -D warnings` clean.
- [x] `cargo test -p swissarmyhammer-tools` green (delete/adjust the truncation tests that no longer apply; do not leave tests asserting truncation). NOTE: one pre-existing, unrelated failure remains — `mcp::tools::skill::tests::test_skill_use_renders_test_skill_body` (stale deployed skill content; verified failing on the clean baseline BEFORE this change). Not in scope.

## Tests
- [x] A test that drives a tool call whose result exceeds the OLD 256-byte cap and asserts the emitted `tool_call complete` log contains the FULL result text (via `tracing-test`), proving no truncation — i.e. it would have failed under the old cap. Added `tool_call_complete_log_emits_full_result_untruncated` in `tests/rmcp_integration.rs`; confirmed RED under truncation, GREEN after removal.
- [x] `cargo test -p swissarmyhammer-tools` green (modulo the pre-existing unrelated skill test noted above).

## Workflow
- Use `/tdd` — write the "full result is logged, not truncated" assertion first (it fails today), then remove the truncation. Then delete the dead utilities and their tests. Verify clippy clean (the import/const removal must leave nothing unused). Do NOT reintroduce any cap or preview anywhere.