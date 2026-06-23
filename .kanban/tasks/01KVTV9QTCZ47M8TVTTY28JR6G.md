---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvv22p1bvrbn0vd2trqmgg3g
  text: 'Picked up. Moving to doing. Plan: TDD on read/mod.rs to add `format` param (hashline default / plain), prefix lines via swissarmyhammer_hashline::tag respecting offset, leave binary untagged, emit whole-file content hash as freshness token. Will run rg blast-radius on execute_read / read op / validator read_file consumers first.'
  timestamp: 2026-06-23T20:17:20.939414+00:00
- actor: claude-code
  id: 01kvv26jcycrbp69sewh51xc8n
  text: |-
    Blast-radius rg findings. `execute_read` is called ONLY within swissarmyhammer-tools: (1) FilesTool dispatch in files/mod.rs (`op: read file`), (2) ReadFileTool wrapper read_file.rs (validator `read_file` name), (3) its own unit tests. No external crate calls it directly.

    Consumers that assert on read OUTPUT content (need updating):
    - read/mod.rs unit tests: assert text.contains("Line 2"/"Hello, world!") — update to tolerate N:HH| prefix.
    - read_file.rs unit test: asserts contains("hello world") — substring still holds with tag prefix, but verify.
    - integration file_tools_integrations.rs::test_read_with_offset_limit — uses assert_eq! exact equality on read output (3 callers: "Line 2\nLine 3", "Line 3\nLine 4\nLine 5", "Line 1\nLine 2\nLine 3"). MUST update to expect tagged output.

    NOT affected (verified): ACP `fs/read_text_file` path (separate from execute_read; acp_read_file.rs only asserts error cases). git diff `read_file_content` reads git blobs, not execute_read. grep/glob tools + session_root_resolution.rs are grep, not read. llama-agent integration tests assert tool schema/availability + that a model emits a read_file call — not exact read content; real-model gated. No programmatic re-parser of read output found, so no at-risk consumer to flag.

    Freshness-token contract (from downstream write task kvrpxpz): write guard takes optional `expected_hash` = whole-file content hash from prior read. So read must emit a stable whole-file digest. Will define deterministic whole-file hash.
  timestamp: 2026-06-23T20:19:28.286812+00:00
- actor: claude-code
  id: 01kvv38m5pnp6zxzq3qq7v3tw4
  text: |-
    Implementation landed (TDD: RED then GREEN verified). Changes:
    - crates/swissarmyhammer-tools/Cargo.toml: added swissarmyhammer-hashline workspace dep.
    - read/mod.rs: added `format` param via ParamMeta::allowed_values(&["hashline","plain"]) default hashline; ReadFormat enum + parse; ReadRequest.format field; handler now reads full file once, computes whole_file_hash (md5 hex over full bytes), windows via window_lines, tags hashline with start_line=offset.unwrap_or(1) for absolute N, prepends `#hash:<hex>\n` freshness-token line. Binary path is unreachable (read_to_string rejects non-UTF8) so binary stays untagged by construction.
    - shared_utils.rs: added pub whole_file_hash(&str)->String (shared with downstream write guard) + pub(crate) window_lines(); refactored SecureFileAccess::read's 4-arm match to use window_lines (DRY). read() signature unchanged → no external breakage.
    - read/description.md: documented `#hash:` line + format param.

    Tests added (read/mod.rs): default hashline tagging, plain opt-out, absolute N under offset, offset+limit anchors match true line, whole-file hash present+matches shared fn, hash stable across identical reads + changes with content.

    Blast-radius consumer tests UPDATED (default legitimately changed to tagged+hash-line):
    - file_tools_integrations.rs: added read_content() de-tag helper; updated test_read_with_offset_limit (asserts #hash line + tag(expected, offset)), test_read_tool_execution_success_cases, test_read_tool_empty_file/single_line/unicode, large-file line counts, test_write_then_read_workflow, test_write_read_roundtrip_properties, test_read_offset_limit_consistency_properties.
    - file_size_limits.rs: added read_body_line_count() helper; updated the two large-file line-count read tests.
    - rmcp_integration.rs::tool_call_complete_log_emits_full_result_untruncated: the leading #hash:\n now embeds a newline in the logged result so the record wraps across captured log lines (and async events interleave). Reworked to assert result_bytes==full length (dynamically computed) AND the _TAIL token (past byte 256) appears in the buffer — preserves the anti-truncation intent without relying on the result being a single physical log line.

    No at-risk consumer flagged: ACP fs/read_text_file is a separate path (only error-case tests), git read_file_content reads blobs, no programmatic re-parser of read output exists.

    Verification: cargo nextest run -p swissarmyhammer-tools = 1345 passed, 0 failed. read::/read_file:: targeted = 27 passed. swissarmyhammer-hashline = 28 passed. cargo fmt --check clean. cargo clippy -p swissarmyhammer-tools -- -D warnings = clean (exit 0).

    NOTE: hit recurring stale-test-binary issue (file mtimes reset by parallel git ops on this branch — HEAD moved to 2b4d4cc5e mid-task); had to `touch` test sources to force nextest recompile. Final results are from freshly-compiled binaries.
  timestamp: 2026-06-23T20:38:04.214237+00:00
- actor: claude-code
  id: 01kvv3kyj24frr9xdh8v1agcd4
  text: |-
    Adversarial double-check returned REVISE with one valid finding: docs claimed binary files return base64, but the read path uses read_to_string which rejects non-UTF8 with an error — there is NO base64 encode anywhere in the read flow (confirmed by reviewer grep). The task's acceptance criterion "binary returns base64 untagged" describes behavior that does not exist in this codebase; my change had expanded the inaccurate claim.

    Resolved by making docs match reality (option a): updated read/mod.rs module + handler docstrings, the `format` param description, the inline comment, read_file.rs tool description, and read/description.md to state non-UTF-8 (binary) files are rejected with an error (never base64, never tagged). The "binary is never tagged" guarantee still holds — by rejection, not by base64. Added test_binary_file_is_rejected_in_both_formats locking in that an invalid-UTF8 file errors under both hashline and plain.

    Reviewer confirmed core functionality correct + verified: hashline math, absolute-N under offset/offset+limit, freshness token, #hash: non-collision with parse_anchor (starts with #, no |), plain opt-out all sound. expected_hash consumption is correctly out of scope (downstream write task).

    Final verification (freshly compiled): cargo nextest run -p swissarmyhammer-tools = 1346 passed, 0 failed. read::/read_file:: = 28 passed (incl. new binary test). cargo test -p swissarmyhammer-tools --doc = 8 passed. swissarmyhammer-hashline = 28 passed. cargo fmt --check clean. cargo clippy -p swissarmyhammer-tools -- -D warnings clean.

    Note for downstream write/edit tasks: read currently REJECTS binary; if a base64-binary read is ever desired it must be implemented (not present today).
  timestamp: 2026-06-23T20:44:15.298100+00:00
depends_on:
- 01KVTV89QPMT2Z63H2KZ8BJ3M1
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffd680
project: file-edit-tools
title: read files — emit hashline tags + whole-file content hash
---
## What
Make `read files` emit hashline-tagged output by default so anchors are the model's path of least resistance. Edit `crates/swissarmyhammer-tools/src/mcp/tools/files/read/mod.rs` (the shared `execute_read` handler).

- Add a `format` param to `READ_FILE_PARAMS` using `ParamMeta::allowed_values(&["hashline", "plain"])`, default `hashline`. Parse it on `ReadRequest`.
- In **hashline** form, prefix each emitted line `N:HH|` using `swissarmyhammer_hashline::tag` — N is the **absolute** 1-based line number so anchors stay stable across `offset`/`limit` windows (tag the window with `start_line = offset`). In **plain** form, emit current behavior unchanged.
- Leave the binary/base64 path **untagged** regardless of `format`.
- Emit a whole-file content hash (a stable digest of full file bytes) in the read result envelope so `write files`/`edit files` can use it as a freshness token for whole-file staleness. Surface it as a small trailing/leading metadata line or structured field (pick one and document it in `description.md`).
- Add `swissarmyhammer-hashline` to `swissarmyhammer-tools` `Cargo.toml` deps.

## Acceptance Criteria
- [ ] Default read output is hashline-tagged: each text line begins `N:HH|`, N absolute even with `offset` set.
- [ ] `format: "plain"` returns the pre-existing untagged output.
- [ ] Binary files return base64 untagged in both formats.
- [ ] The read result exposes a whole-file content hash usable as a freshness token.
- [ ] `offset`/`limit` still work and the tagged N matches the true file line number.

## Tests
- [ ] Unit tests in `read/mod.rs`: default hashline tagging; `plain` opt-out; absolute N under `offset`; binary stays untagged; content hash present and stable.
- [ ] Update existing read tests that assert raw line content to tolerate/expect the tag prefix.
- [ ] `cargo test -p swissarmyhammer-tools read::` is green.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

## Review Findings (2026-06-23 14:46)

Reviewer verdict: PASS. Substantive criteria met and test-verified (21 `read::` unit tests + 172 file-tools integration tests green via `cargo nextest`). Both flagged correctness concerns investigated directly and cleared. Engine blockers refuted by inspection. Only cosmetic nits remain → advanced to `done`.

### Correctness concerns (investigated directly)
- [x] Binary base64 path — NOT a regression. Prior `SecureFileAccess::read` at HEAD already used `std::fs::read_to_string`, which errors on non-UTF-8 bytes. There was never a working base64 binary read path — "automatic base64 encoding" existed only in doc/description strings, never in code. Binary errored before and is rejected now; no behavior was dropped. Acceptance line "binary returns base64 untagged" is unsatisfiable against the actual prior code; implementation correctly rejects binary and `test_binary_file_is_rejected_in_both_formats` passes.
- [x] md5 + `#hash:` line — sound. `md5` is an existing workspace dependency (`{ workspace = true }`, used by 7+ crates), not a new heavy add. `whole_file_hash` digests full file bytes; `#hash:<hex>\n` is prepended and re-derivable downstream by `write files`/`edit files`. The hashline `N:` absolute anchors below it disambiguate the metadata line from content.

### Engine blockers — REFUTED (false positives)
The review engine reported 8 "defined twice / duplicate definition" blockers in `tests/integration/file_tools_integrations.rs`. Direct inspection shows each named fn (`extract_response_text` L153, `read_content` L172, `test_read_with_offset_limit` L759, `test_write_then_read_workflow` L2884, `test_write_read_roundtrip_properties` L4224, `test_read_offset_limit_consistency_properties` L4423) is defined exactly once. Duplicate Rust definitions would be a hard compile error; the package compiles and all 172 integration tests run once each. Disregarded.

### Cosmetic nits (non-blocking, optional cleanup)
- [ ] `read/mod.rs` — `execute_read` (~145 lines) could be split into validate/secure/read/format helpers.
- [ ] `read/mod.rs` tests — use `HASH_LINE_PREFIX` constant instead of hardcoded `"#hash:"` literals (would require making the constant `pub(crate)` in `shared_utils` to share with integration tests).
- [ ] `shared_utils.rs` — `window_lines` could carry a doc comment on 1-based offset semantics.
- [ ] `read/mod.rs` / `file_size_limits.rs` — magic numbers (1_000_000 offset cap, 100_000 line cap, test data sizes) could be named constants.
- [ ] `tests/integration` — `extract_response_text` duplicated across `file_size_limits.rs` and `file_tools_integrations.rs` could be hoisted to a shared test util module.