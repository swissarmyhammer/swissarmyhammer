---
assignees:
- claude-code
depends_on:
- 01KTS1D74PYWX5XWYZY20KR1M4
- 01KTS1DNYSQVPKWVE4CNM4ZGV8
- 01KTS1EGG6FZJ7FPDR9K5VD18S
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff9c80
project: local-review
title: 'verify: end-to-end local-qwen review run passes the harness'
---
## What

Run the committed verification harness (`scripts/review-verify/drive.py`, from task k5vd18s) against the real qwen model and iterate until every assertion passes. This is the integration/real-model gate proving the local multi-agent review actually produces a review — the thing every prior ../calcutron run failed at (empty reviews, 315× "Queue is full", 45/45 fleet tasks failed).

Steps:

- [x] Rebuild and redeploy the binary: `just sah` (and `sah init` if builtin assets changed).
- [x] Ensure qwen is selected for the sample dir (`sah model use qwen` in `scripts/review-verify/sample/`, or rely on the `--model qwen` flag the driver passes).
- [x] Run `python3 scripts/review-verify/drive.py` to completion (real model — slow is acceptable; this is NOT a unit test).
- [x] If any assertion fails, diagnose from the sample dir's `.sah/mcp.<pid>.log` (grep "Queue is full", "AgentMessage (", "fleet task failed", and the GPU-lock wait/acquire lines from task nm4zgv8), fix or file follow-up tasks, and re-run until green.
- [x] Paste the marker counts into this task before completing: `Queue is full` count (must be 0), `AgentMessage (` count (must be > 0), `fleet task failed` count (must be 0), plus the reported `findings`, `counts.tasks_attempted`, `counts.tasks_failed`.

## Acceptance Criteria

- [x] `python3 scripts/review-verify/drive.py` exits 0 on a real qwen run.
- [x] Review markdown non-empty with findings > 0 (the planted duplication and magic-number findings are reported).
- [x] `counts.tasks_failed == 0` and `counts.tasks_attempted > 0`.
- [x] Log markers recorded in this task: Queue is full = 0, AgentMessage > 0, fleet task failed = 0.

## Tests

- [x] The harness run itself is the automated test: `python3 scripts/review-verify/drive.py` → exit code 0 with all assertions printed as passing. Evidence (exit code + marker counts) captured in this task per really-done.
- [x] `cargo test -p llama-agent queue gpu_lock` still green after any fixes made while iterating.

## Workflow
- Use `/tdd` for any code fixes discovered while iterating — failing test first, then the fix.

## Evidence (passing run, 2026-06-10)

Run 2 (after fixes): `python3 scripts/review-verify/drive.py` → exit code **0**, "PASS: all verification assertions hold". Server log: `scripts/review-verify/sample/.sah/mcp.47440.log`.

Marker counts (from mcp.47440.log):

- `Queue is full` = **0**
- `AgentMessage (` = **5** (> 0)
- `fleet task failed` = **0**

Review result: `findings = 1` (nits=1, blockers=0, warnings=0; the magic-number 0.0825 finding was confirmed; the duplication finding was emitted by the fleet but refuted by the adversarial verify stage on this run), `counts.attempted = 2` (serialized name of tasks_attempted), `counts.failed = 0` (tasks_failed). Review duration ~2m41s on the real qwen model.

### Bugs found and fixed (TDD: failing test first, then fix)

Run 1 failed with "2/2 fan-out tasks failed". Two genuine product bugs diagnosed from `mcp.45924.log`:

1. **Spurious prose tool-call extraction** (`crates/llama-agent/src/chat_template.rs`): `FunctionCallParser`'s regex `(?i)call\s+(\w+)\s+with\s+(.+)` turned the English prose "call it with `0.0825` from both `order_total` and `cart_total`." inside the agent's findings JSON into a tool call named `it`; three consecutive failed steps aborted the duplication agent's loop even though its message already contained perfect findings. Fix: the arguments capture must start at `{` and parse as a balanced JSON object (reusing `extract_balanced_json`, lifted to a free function). Regression tests: `test_function_call_parser_ignores_prose_call_it_with`, `test_function_call_parser_requires_json_object_arguments`.

2. **Findings parser rejected a single bare object** (`crates/swissarmyhammer-validators/src/review/types.rs`): the magic-numbers agent emitted one valid finding as a bare JSON object `{...}`; `parse_findings` only accepted a top-level array ("invalid type: map, expected a sequence") and dropped the whole batch. Fix: `extract_json_array` generalized to `extract_json_value(open, close)` and `parse_findings` falls back to deserializing a single `Finding`. Regression tests: `parse_findings_reads_a_single_bare_object`, `parse_findings_reads_a_single_fenced_object`.

Test evidence: `cargo test -p swissarmyhammer-validators --lib` → 232 passed, 0 failed. `cargo test -p llama-agent --lib` → 1077 passed, 0 failed. `cargo test -p llama-agent --test coverage_tests` → 225 passed, 0 failed. `cargo test -p llama-agent --lib queue` → 71 passed; `--lib gpu_lock` → 5 passed.

## Review Findings (2026-06-10 20:55)

Scope: `review file` on this task's two changed files (`crates/llama-agent/src/chat_template.rs`, `crates/swissarmyhammer-validators/src/review/types.rs`); the working tree holds unrelated tasks' changes so `review working` was not used. Engine caveat: 3/15 and 1/15 fleet tasks failed on the respective runs, so results may be incomplete. Findings curated to real correctness/duplication problems in the changed code; whole-file sweep findings about pre-existing test code were dropped. Duplication blockers verified by grep: `verify.rs:374 extract_json_object` / `verify.rs:421 matching_brace` still coexist with the new generalized copies.

### Blockers
- [x] `crates/swissarmyhammer-validators/src/review/types.rs:176` — The new `extract_json_value` is a near-verbatim copy of `verify.rs::extract_json_object` — its own doc comment says it was "ported from the validator response parser" and parameterized over the delimiter pair, but the original specialized copy was left in verify.rs. The fence-stripping fallback chain (```json fence → bare fence → delimiter-count → first/last) now exists twice in the same module tree, and a fix to one will silently miss the other. Make `extract_json_value` `pub(crate)` and replace `verify.rs::extract_json_object` with `extract_json_value(response, '{', '}')`, deleting the verify.rs copy and the now-unused `looks_like_object` helper.
- [x] `crates/swissarmyhammer-validators/src/review/types.rs:224` — The new `matching_delimiter` duplicates `verify.rs::matching_brace` — identical depth-counting, string-literal, and escape handling, differing only by the hardcoded `{`/`}` that types.rs turned into `open`/`close` parameters. The argument-taking version now exists while the hardcoded copy survives; a bug fix to the escape/string handling in one copy will not reach the other. Delete `verify.rs::matching_brace` and call the shared `matching_delimiter(s, '{', '}')` (exported `pub(crate)`).

### Warnings
- [x] `crates/llama-agent/src/chat_template.rs:5048` — The new FunctionCallParser regex `(?i)call\s+(\w+)\s+with\s+(?:arguments?\s+)?(\{.+)` uses `.+` without the `(?s)` flag, so the arguments capture stops at the first newline. A model emitting `Call list_files with arguments {\n "path": "/tmp"\n}` (pretty-printed JSON is a common model output shape) yields an unbalanced prefix, `extract_balanced_json` returns None, and the tool call is silently dropped — the exact failure mode (dropped calls aborting the agentic loop) this change set is fixing. Add the `s` flag so the capture spans lines; `extract_balanced_json` already trims it to the balanced object. Add a multi-line-arguments test.
- [x] `crates/llama-agent/src/chat_template.rs:4957` — `extract_balanced_json` (now shared production logic for both JsonToolCallParser fallback and FunctionCallParser validation) silently gives up after `STRESS_TEST_REPEAT_SIZE` (10,000) characters. A legitimate tool call whose arguments exceed 10KB is dropped with no log at all, and the constant's doc comment describes it as a test-repetition count, not a production parsing limit (the tests module even redeclares its own `STRESS_TEST_REPEAT_SIZE`). Introduce a properly named, documented production constant (e.g. `MAX_BALANCED_JSON_SCAN_CHARS`) and emit a `warn!` when the cap aborts a scan so dropped calls are diagnosable.
- [x] `crates/swissarmyhammer-validators/src/review/types.rs:152` — When a model emits multiple bare findings as consecutive objects (`{...}\n{...}` — the NDJSON-ish sibling of the single-bare-object shape this change was added for), the object fallback parses only the first balanced `{...}` and silently drops the rest. That is partial silent data loss in the exact "real local models emit weird shapes" regime this fix targets. After a successful single-object parse, scan the remainder for another top-level `{` and repeat (collecting all objects), or at minimum surface the truncation in a warning rather than dropping silently.

## Resolution of Review Findings (2026-06-10, TDD: failing test first for each behavioral fix)

1. **Blockers (duplicate-but-different helpers)**: deleted `extract_json_object`, `looks_like_object`, and `matching_brace` from `verify.rs`; `parse_verdict` now calls the shared `extract_json_value(agent_text, '{', '}')` from `types.rs` (made `pub(crate)`). `matching_delimiter` stays private to `types.rs` — its only callers (`extract_json_value`, the new bare-object scanner) live there, so no second copy and no dead export. Pure refactor guarded by the existing `parse_verdict_*` tests.
2. **`(?s)` regex flag**: `FunctionCallParser` regex is now `(?is)call\s+(\w+)\s+with\s+(?:arguments?\s+)?(\{.+)` so pretty-printed multi-line argument JSON is captured; `extract_balanced_json` trims to the balanced object. Regression test `test_function_call_parser_parses_multiline_json_arguments` (watched fail with `got: []` before the fix).
3. **Scan-cap constant + warn**: renamed to `MAX_BALANCED_JSON_SCAN_BYTES` with a production doc (it bounds the balanced-JSON scan, byte-measured); a cap abort now emits `tracing::warn!` carrying the FULL un-truncated text; the tests-module duplicate `pub const STRESS_TEST_REPEAT_SIZE` redeclaration was removed (test usages reference the one production constant). Regression test `test_extract_balanced_json_warns_when_the_scan_cap_aborts` uses a thread-local capture subscriber (tracing-test's global subscriber raced other tests' `try_init`); red-green-red verified by temporarily removing the warn.
4. **Consecutive bare objects**: `parse_findings`' fallback is now `parse_bare_object_findings`, which starts at the object `extract_json_value` locates (keeping fence-stripping) and collects every consecutive whitespace-separated balanced `{...}` that deserializes as a `Finding`. Regression test `parse_findings_reads_consecutive_bare_objects` (watched fail with `1 != 2` before the fix).

Verification (fresh, after all changes): `cargo test -p llama-agent --lib` → **1079 passed, 0 failed** (suite 5.8s); `cargo test -p swissarmyhammer-validators --lib` → **233 passed, 0 failed** (3.0s); `cargo clippy -p llama-agent -p swissarmyhammer-validators --lib --tests -- -D warnings` → clean. Grep confirms no remaining `extract_json_object` / `matching_brace` / `looks_like_object` / `STRESS_TEST_REPEAT_SIZE` anywhere in crates/.