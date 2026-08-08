---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kz77tavvernevn9v385p53ya
  text: |-
    Swept all 135 `#[error("Capital...")]` attributes across the 12 files named in the task, plus 3 hand-rolled Display-equivalent format! strings in `content_capability_validator.rs::to_acp_error` that duplicate the enum's message text.

    Kept capitalized (per rule): all-caps acronyms (MCP, MIME, URI, SSRF, IO, ID), CamelCase identifiers (LoadSession, ResourceLink), and proper nouns (Claude, Base64 — treated as a standard/proper-noun encoding name, consistent with Git/Claude).

    Scope decision: did not rewrite construction-site String arguments passed into `AgentError::*(...)`/other enum variants (the "224 sites" in the task's Size section) — those land mid-message (after the enum's own "prefix: " text), so they do not affect the Display message's first character, which is what `builtin/validators/rust/rules/error-handling.md` and the task's "Rule to apply" section actually govern. Verified via `rg '#\[error\("[A-Z]' crates/claude-agent/src` — the only remaining acceptance-criterion check — that this is the full and correct scope.

    Fixed every test whose assertion broke as a result (found by re-running the suite, not just by inspection):
    - `src/error.rs` — 9 `assert_eq!(...to_string()...)` in `test_error_display` + `test_agent_error_to_acp_error_conversion`
    - `src/session_errors.rs` — `protocol_error.message.contains("Invalid session ID format")` → lowercase
    - `src/acp_error_conversion.rs` — 4 assertions (`test_base64_error_conversion`, `test_size_exceeded_error_conversion`, `test_security_violation_no_info_disclosure`, `test_content_processing_error_invalid_structure`, `test_content_processing_error_invalid_base64`)
    - `src/content_security_integration_tests.rs` — one `.contains(...)` updated for consistency (test also has lowercase-substring OR-fallbacks so it would not have failed, but the string no longer matched reality)
    - `src/tools.rs` — one speculative `.contains("Invalid session ID format")` OR-branch (dead in practice; `SessionSetupError::InvalidSessionId` is never constructed outside its own test) updated for correctness
    - `tests/integration/coverage_tests.rs` — `test_agent_error_server_error_display` and `test_session_id_error_invalid_display` (outside `src/`, caught only by running the suite)

    Verification:
    - `rg '#\[error\("[A-Z]' crates/claude-agent/src` → only acronym/CamelCase/proper-noun starts remain (35 lines, all expected).
    - `cargo build -p claude-agent` clean.
    - `cargo nextest run -p claude-agent` → 1087 passed, 0 failed.
    - `cargo clippy -p claude-agent --all-targets -- -D warnings` → clean.
    - `cargo check -p swissarmyhammer-validators -p swissarmyhammer-agent -p acp-conformance` (the three workspace crates that depend on claude-agent) → clean, no downstream breakage.

    step: implement
    outcome: changed
    evidence: 15 files — crates/claude-agent/src/{error.rs, session_errors.rs, content_block_processor.rs, content_security_validator.rs, acp_error_conversion.rs, base64_processor.rs, path_validator.rs, mime_type_validator.rs, content_capability_validator.rs, base64_validation.rs, session.rs, size_validator.rs, tools.rs, content_security_integration_tests.rs}, crates/claude-agent/tests/integration/coverage_tests.rs
    task: ^f70h1pr
  timestamp: 2026-08-04T20:35:37.467683+00:00
- actor: claude-code
  id: 01kz788wrxjq1wjzv99e6fhb2x
  text: |-
    ### test — green
    - evidence: `cargo nextest run --workspace` — 13501 tests run, 13501 passed, 0 failed, 0 skipped (3 slow, 2 leaky). `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean, 0 warnings.
    - also checked: no `#[ignore]` tests in the repo. Cross-checked all 124 old (uppercase) claude-agent error strings against the full workspace, and specifically against the 4 dependent crates (swissarmyhammer-agent, acp-conformance, swissarmyhammer-validators, swissarmyhammer-tools) — no stale assertions found outside claude-agent itself. All claude-agent Display impls confirmed lowercase.
    - next: none, suite is green.
  timestamp: 2026-08-04T20:43:34.557339+00:00
- actor: claude-code
  id: 01kz789xk6jbn2pra2sx4pr8tw
  text: |-
    ### test — green
    - evidence: `cargo nextest run --workspace` — 13501 tests run, 13501 passed, 0 failed, 0 skipped. `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean, 0 warnings.
    - acceptance check: `rg '#\[error\("[A-Z]' crates/claude-agent/src` returns only acronym (MCP, MIME, DoS, URI, SSRF, IO), CamelCase-identifier (LoadSession), and proper-noun (Claude) starts — matches the rule.
    - no `#[ignore]` tests found anywhere in the workspace.
    - checked swissarmyhammer-validators, swissarmyhammer-agent, and acp-conformance for stale exact-text assertions on claude-agent error messages — none found. Their `to_string()`/`contains()` assertions test each crate's own error types, not claude-agent's.
    - next: none, task is clean.
  timestamp: 2026-08-04T20:44:08.166225+00:00
- actor: claude-code
  id: 01kz78bpqjpkr3py9tg1tqd4jw
  text: |-
    ### commit — changed
    - evidence: e5ba6695f fix(claude-agent): lowercase capitalized error Display messages
    - next: ready for review

    step: commit
    outcome: changed
    evidence: e5ba6695f fix(claude-agent): lowercase capitalized error Display messages
    task: ^f70h1pr
  timestamp: 2026-08-04T20:45:06.674213+00:00
- actor: claude-code
  id: 01kz7bss19mcghbe5knk170njr
  text: |-
    ### review — stuck
    - evidence: review sha HEAD~1..HEAD returned 138 confirmed findings. 112 kept as open checklist items (doc gaps, duplication, cognitive complexity/nesting, magic numbers, naming). 13 findings conflict with the task's own documented acronym/CamelCase/proper-noun exception (MCP, MIME, Base64, LoadSession, ULID) and were dropped from the checklist and recorded under "Conflicts" instead — the commit consistently and deliberately preserved those per `git diff HEAD~1..HEAD`, and the blanket validator rule has no such carve-out. 13 findings on pre-existing, untouched test code (coverage_tests.rs magic numbers/timeouts, content_security_integration_tests.rs, session.rs test constants) were dropped per the test-refactor exemption.
    - next: a person must reconcile `builtin/validators/rust/rules/error-handling.md` (blanket lowercase) with the task's documented acronym exception, then either update the validator or confirm the exception stands. Remaining 112 findings still need fixes before this task can move to done.
  timestamp: 2026-08-04T21:45:13.513633+00:00
- actor: claude-code
  id: 01kz7bttnc3t65geqveva87rdb
  text: |-
    ### finish iteration 1 — stuck
    - implement: changed — 135 #[error(...)] Display messages lowercased across 15 files in crates/claude-agent, per this task's documented acronym/CamelCase/proper-noun exceptions
    - test: green — cargo nextest run --workspace, 13501 passed; clippy clean
    - commit: 018b7217b
    - review: stuck — true conflict found. The generic `error-handling` validator rule has no carve-out for all-caps acronyms (MCP, MIME, Base64), CamelCase identifiers (LoadSession), or proper nouns, and flags 13 sites this task's own description explicitly says must stay capitalized. Per the Findings Are Requirements doctrine, this is a true conflict (a rule that fights a documented contract) — recorded as a blocker, task left stuck in review, no self-resolution attempted. 112 other findings are legitimate independent fixes but are not applied because the task-level conflict halts the loop per protocol ("mark the task stuck, and stop"). A human needs to reconcile builtin/validators/rust/rules/error-handling.md with this task's documented exceptions before work resumes.
  timestamp: 2026-08-04T21:45:47.948798+00:00
- actor: claude-code
  id: 01kzh1bzdvpfcxj3cavh09j6a0
  text: |-
    ### conflict — resolved by the user

    The user read the conflict recorded in iteration 1 and gave the decision: add the carve-out to the builtin rule.

    `builtin/validators/rust/rules/error-handling.md:20` now reads:

    > `Display` messages on errors: lowercase, no trailing punctuation. Do not change the first word when it is an all-caps acronym (`MCP`, `JSON`, `I/O`, `URL`), a CamelCase identifier (`LoadSession`), or a proper noun (`Git`, `Claude`). Keep acronyms in the middle of a message as written.

    That file is the only copy of the text in the repo.

    Effect: the 13 items in the "Conflicts" section are correct as written. The code does not change for them. The blocker is removed and the loop can continue on the 112 open findings.
  timestamp: 2026-08-08T15:55:19.867598+00:00
- actor: claude-code
  id: 01kzh7dxvhkhntccts3crfk3bz
  text: |-
    Worked the 112 open review findings, and closed the 13 conflict items with no code change.

    ## Conflicts section — closed, no code change

    The user amended `builtin/validators/rust/rules/error-handling.md` with the acronym / CamelCase / proper-noun carve-out. All 13 items are correct as written. I changed no code for them, marked each `- [x]`, and rewrote the section heading to `### Conflicts — resolved by the user, rule amended, no code change`, quoting the amended rule.

    Note: `review dump validators` still returns the pre-amendment text of that rule (it reads a deployed copy, not `builtin/`). The source file carries the carve-out. That copy is not this task's concern.

    ## The 112 findings

    **Duplication consolidated**
    - `acp_error_conversion.rs` — one generic `add_error_context<E: ToJsonRpcError>` plus `insert_error_context_fields`. All five `convert_*_error_to_acp` functions are now one-line delegations, which also removes the depth-4 nesting from all five.
    - `error.rs` — one `serialize_error_as_display` helper; both `Serialize` impls call it.
    - `base64_processor.rs` — one `validate_enhanced_security(&self, data, content_type)`; the three decode methods call it.
    - `content_block_processor.rs` — new private `ContentAccumulator` with `accumulate` / `accumulate_fallback` / `into_summary`; both batch paths fold through it.
    - `mime_type_validator.rs` — one `validate_mime_type_for_category` behind all three public validators, and one `validate_format_matches_mime` behind the image and audio matchers, driven by `IMAGE_MIME_FORMATS` / `AUDIO_MIME_FORMATS` tables.

    **Complexity and nesting**
    - `content_block_processor.rs::process_content_block_internal` split into `process_image_content`, `process_audio_content`, `process_embedded_resource` (which dispatches to `process_text_resource` / `process_blob_resource`) and `process_resource_link`, plus `describe_source`, `describe_mime_type` and `optional_uri`.
    - `process_content_block_with_retry` flattened: the non-retryable case is a match guard, and the backoff moved to `sleep_before_retry` / `log_retry_success`.
    - `content_security_validator.rs::validate_resource_content` split into `validate_text_resource`, `validate_blob_resource` and `validate_blob_mime_type_consistency`.
    - `path_validator.rs::validate_permissions` split into `permission_metadata`, `permission_name`, `applicable_mode_bits`, `validate_unix_permissions` and `validate_windows_permissions`.

    Verified against the tool rule the `complexity-rust` validator runs (`CLIPPY_CONF_DIR` with `excessive-nesting-threshold = 6`, plus `too_many_lines`, `too_many_arguments`, `type_complexity`): none of the six refactored files reports. The hits that remain are in `mcp.rs`, `agent_commands.rs`, `mcp_error_handling.rs`, `terminal_manager.rs`, `tool_types.rs` and `session_errors.rs` — files this task never touched.

    **Magic numbers**
    - JSON-RPC codes now come from the crate's existing `json_rpc_codes` module (`INVALID_REQUEST`, `PARSE_ERROR`, `SERVER_ERROR`, `INVALID_PARAMS`, `METHOD_NOT_FOUND`, `INTERNAL_ERROR`). Discovery worth recording: `crates/claude-agent/src/json_rpc_codes.rs` already defined every constant the findings ask for, with the same meanings. Defining a second `JSON_RPC_*` set in `error.rs` would have been the duplication the findings object to, so the existing module is used instead — in `error.rs`, `base64_processor.rs`, `mime_type_validator.rs`, `content_security_validator.rs` and `content_block_processor.rs`.
    - `base64_processor.rs` — `MIN_HEURISTIC_DATA_LEN`, `NULL_BYTE_THRESHOLD_RATIO`, `DOS_HEADER_MIN_SIZE`, `ELF_HEADER_MIN_SIZE`, `MACHO_PARTIAL_HEADER_MIN_SIZE`, `MACHO_FULL_HEADER_MIN_SIZE`, and an `EXECUTABLE_SIGNATURES` table that replaces the four parallel `if` arms.
    - `content_security_validator.rs` — `STRICT_MAX_CONTENT_ARRAY_LENGTH`, `STRICT_RATE_LIMIT_REQUESTS_PER_MINUTE`, `MODERATE_MAX_CONTENT_ARRAY_LENGTH`, `MODERATE_RATE_LIMIT_REQUESTS_PER_MINUTE`, `BASE64_DECODED_BYTES_PER_GROUP`, `BASE64_ENCODED_CHARS_PER_GROUP` behind `estimated_base64_decoded_size()`, `RESOURCE_CONTENT_SIZE_ESTIMATE`, `RESOURCE_LINK_SIZE_ESTIMATE`, `CONTENT_SNIFF_SAMPLE_BASE64_CHARS`, `MIN_DATA_LENGTH_FOR_REPETITION_CHECK`, `REPETITION_SAMPLE_LEN`, `MAX_REPETITION_COUNT`, `OPAQUE_BINARY_MIME_TYPE`.
    - `content_block_processor.rs` — `MAX_RETRIES`, `MS_PER_SECOND`, `MAX_BACKOFF_MS`, `BACKOFF_BASE`, `DEFAULT_BLOB_MIME_TYPE`.
    - `session.rs` — `DEFAULT_CLEANUP_INTERVAL_SECS`, `DEFAULT_MAX_SESSION_AGE_SECS`.
    - `path_validator.rs` — `OWNER_MODE_BITS`, `GROUP_MODE_BITS`, `OTHER_MODE_BITS`.

    **Documentation** — every public item in the five named files now carries a doc comment, enums and struct fields included, not only the lines the findings cited. Measured with the same lint the `missing-docs-rust` validator runs (`cargo clippy -p claude-agent --lib -- -W missing_docs`): `base64_processor.rs`, `base64_validation.rs`, `content_block_processor.rs`, `content_security_validator.rs` and `mime_type_validator.rs` report zero. The crate still holds 758 `missing_docs` elsewhere (99 in `session_errors.rs`, 31 in `config.rs`, 27 in `acp_error_conversion.rs` and so on) — pre-existing debt in files these findings never named.

    **Debug** — added to `Base64Processor`, `ContentBlockProcessor` and `ContentCapabilityValidator`. `MimeTypeValidator` also needed `Debug`, because `Base64Processor` holds one and could not derive `Debug` without it.

    **Rename** — `SessionId::ulid_string()` is now `to_ulid_string()`. The one call site outside the definition is `tests/integration/coverage_tests.rs`, whose test is renamed to match. `rg ulid_string` across `crates/` shows no stale caller.

    ## One behavior change worth naming

    `process_image_content` now renders an image whose `uri` is `Some("")` as `(embedded)` instead of `from ` with a trailing space. The text resource and blob resource paths already treated an empty URI that way, and the shared `describe_source` helper makes all three agree. No test covered the old shape.

    ## Verification

    - `cargo build -p claude-agent` — clean.
    - `cargo nextest run -p claude-agent` — 1087 tests run, 1087 passed, 0 skipped.
    - `cargo clippy -p claude-agent --all-targets -- -D warnings` — clean, exit 0.
    - `cargo check -p swissarmyhammer-validators -p swissarmyhammer-agent -p acp-conformance --all-targets` — clean, exit 0.

    ## Environment note for the next agent

    Three stale processes held the cargo build-directory lock for over seven hours: a hung `cmake` building `llama.cpp` (started 04:58), its parent `cargo check --workspace` from rust-analyzer, and two of my own queued clippy runs. Every build blocked on `Blocking waiting for file lock on build directory` until they were killed. Check `ps` for a long-lived `cmake ... llama.cpp` before concluding that a build is merely slow.

    ### implement — changed
    - evidence: 12 files — crates/claude-agent/src/{acp_error_conversion.rs, base64_processor.rs, base64_validation.rs, content_block_processor.rs, content_capability_validator.rs, content_security_validator.rs, error.rs, mime_type_validator.rs, path_validator.rs, session.rs}, crates/claude-agent/tests/integration/coverage_tests.rs; builtin/validators/rust/rules/error-handling.md carries the user's amendment and was not touched by this step. All 125 checklist items marked `- [x]`.
    - next: ready for `/review`
  timestamp: 2026-08-08T17:41:15.249476+00:00
- actor: claude-code
  id: 01kzh7p3q9sm04cbnvbcvyz7sg
  text: |-
    ### conflict fix — the deployed validator copy also needed the amendment

    `builtin/validators/rust/rules/error-handling.md` is NOT what the review engine reads. `review list validators` reports `source_layer: user` and `path: /Users/wballard/.validators/rust`. The engine reads the deployed copy under `~/.validators/`, so an edit to `builtin/` alone changes nothing for `/review`.

    Both copies now carry the carve-out:

    - `builtin/validators/rust/rules/error-handling.md:20` — the repo source of truth.
    - `/Users/wballard/.validators/rust/rules/error-handling.md:20` — the copy the engine reads.

    Verified with `review get validator rust`: the `error-handling` rule body now returns the amended text with the acronym, CamelCase and proper-noun carve-out. The 13 conflict items cannot be raised again by this rule.

    Note for a later card: `~/.validators/` is deployed, not generated on demand. An amendment to `builtin/` needs a redeploy, or the two copies drift.
  timestamp: 2026-08-08T17:45:43.401539+00:00
- actor: claude-code
  id: 01kzh84761s9egtcqhvkb1b83m
  text: |-
    ### test — green
    - evidence: `cargo nextest run --workspace` — 13645 tests run: 13645 passed (14 slow), 0 skipped. `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean, 0 warnings.
    - checked the public rename `ulid_string()` -> `to_ulid_string()` for workspace-wide breakage: grep for `.ulid_string(` and `ulid_string` across the whole repo found no stray old-name callers outside claude-agent (the only other matches are the unrelated `generate_monotonic_ulid_string` function in swissarmyhammer-common, and the already-renamed `to_ulid_string` usages inside claude-agent itself). No fixes needed.
    - no stale cargo/cmake build-lock processes found before the run (only rust-analyzer and sccache processes were present).
    - next: leave in doing for review.
  timestamp: 2026-08-08T17:53:25.697036+00:00
- actor: claude-code
  id: 01kzh885de7s47x1skgacw3w8s
  text: |-
    ### commit — changed
    - evidence: f75efff68 fix(claude-agent): close 112 review findings on error messages and validators
    - files: crates/claude-agent/src/{acp_error_conversion,base64_processor,base64_validation,content_block_processor,content_capability_validator,content_security_validator,error,mime_type_validator,path_validator,session}.rs, crates/claude-agent/tests/integration/coverage_tests.rs, builtin/validators/rust/rules/error-handling.md, .kanban/tasks/01KYZ2CQF0TWW0CMZ1YF70H1PR.{md,jsonl}
    - next: local commit only, not pushed. Task stays in doing.
  timestamp: 2026-08-08T17:55:34.958760+00:00
- actor: claude-code
  id: 01kzhajw02mcn6vtwnaymztnyq
  text: |-
    ### review — findings

    - scope: `review sha HEAD~1..HEAD` — the checkpoint commit `f75efff68` only, not the accumulated task diff. Verified `git rev-parse HEAD` = `f75efff682a4baa7be9f591460983e364caba912` before the run.
    - evidence: engine counts `findings: 123, confirmed: 126, refuted: 20, attempted: 44, failed: 0, skipped: 1`. 111 recorded as open checklist items in the new `## Review Findings (2026-08-08 12:56)` section. The 125 items from the 2026-08-04 section stay checked and untouched.
    - the resolved conflict did NOT recur. `review get validator rust` shows the deployed `error-handling` rule carries the acronym / CamelCase / proper-noun carve-out, and the engine raised no lowercase-Display finding this round. `MCP server error`, `MIME type format validation failed`, `Base64 security violation` and `LoadSession capability not supported` were not reported.
    - 12 findings dropped by the skill's pre-existing-test exemption, listed under a "Dropped" heading in the new section: 9 in `coverage_tests.rs` (this commit changed only the `to_ulid_string` rename there), 1 in the `content_security_validator.rs` test module (the diff touches nothing below line 1035), 2 in the `session.rs` test module.
    - one file is a review coverage gap, not a clean result: `crates/claude-agent/tests/integration/coverage_tests.rs` is 365706 rendered bytes against the 262144-byte per-file cap, so the `duplication` validator could not read it.
    - the new findings are mostly in territory the last round did not name: `missing documentation` on enum variants and struct fields (not only the public items the 2026-08-04 round cited), Rust API design (`get_` prefixes, concrete `Vec`/`PathBuf`/`String` parameters), three case-sensitive MIME comparisons that RFC 2045 says must be case-insensitive, and duplication in the helpers the last round introduced.
    - next: task stays in `review` with 111 open findings.

    step: review
    outcome: findings
    evidence: 111 findings — crates/claude-agent/src/{acp_error_conversion.rs, content_block_processor.rs, content_capability_validator.rs, content_security_validator.rs, error.rs, mime_type_validator.rs, path_validator.rs, session.rs}
    task: ^f70h1pr
  timestamp: 2026-08-08T18:36:22.914887+00:00
- actor: claude-code
  id: 01kzhamhp38gpagkw9q1vhf1qy
  text: |-
    ### finish iteration 2 — findings

    - conflict: resolved by the user. The rule now carves out all-caps acronyms, CamelCase identifiers and proper nouns. Amended in BOTH `builtin/validators/rust/rules/error-handling.md` and the deployed `/Users/wballard/.validators/rust/rules/error-handling.md` that the engine actually reads. The 13 conflict items are checked, with no code change.
    - implement: changed — 12 files, 1447 insertions, 775 deletions. Closed all 112 open findings from the 2026-08-04 round.
    - test: green — `cargo nextest run --workspace` 13645 passed, 0 skipped; `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean.
    - commit: f75efff68 fix(claude-agent): close 112 review findings on error messages and validators
    - review: findings — 111 open, recorded in a new `## Review Findings (2026-08-08 12:56)` section. Engine counts: findings 123, confirmed 126, refuted 20, attempted 44, failed 0, skipped 1.

    Guardrail check: NOT triggered. No finding repeats from iteration 1 — every item from the 2026-08-04 section is closed and the 111 new items name different lines and different rules. Two iterations, two different finding sets, so the loop continues.

    The new findings group as:

    - 63 `missing documentation` on enum variants and struct fields. The last round cited only public items, so the cause was removed at a narrower level than the rule reaches.
    - Rust API design items the last round never raised: the `get_` prefix on `get_turn_request_count`/`get_turn_token_count`, and concrete `Vec<PathBuf>`, `PathBuf`, `String`, `Vec<AvailableCommand>` parameters that the rule says must be generic.
    - 3 correctness findings: case-sensitive MIME comparisons that RFC 2045 requires be case-insensitive, at `content_security_validator.rs:885`, `mime_type_validator.rs:363` and `mime_type_validator.rs:501`. The `:885` site is in code THIS commit introduced. These are the highest-value items in the set.
    - Fresh duplication inside the helpers iteration 2 added — `process_image_content`/`process_audio_content`, `process_text_resource`/`process_blob_resource`, and the `validate_*_mime_type` pair.

    Open coverage gap, recorded so it is not read as a clean pass: `crates/claude-agent/tests/integration/coverage_tests.rs` is 365706 rendered bytes against the 262144-byte per-file cap, so the `duplication` validator could not read it.
  timestamp: 2026-08-08T18:37:17.891773+00:00
- actor: claude-code
  id: 01kzhbwpgkq19va3bsgjgjrgr1
  text: |-
    Iteration 3. Closed all 111 open findings in the `## Review Findings (2026-08-08 12:56)` section. The 2026-08-04 section stays checked and untouched.

    ## The three correctness bugs came first, with TDD

    RFC 2045 makes MIME types case-insensitive. Four sites compared them case-sensitively — the three the findings named, plus one the findings did not, which is the same cause in the same file.

    - `content_security_validator.rs` — `mime_type == OPAQUE_BINARY_MIME_TYPE` is now `eq_ignore_ascii_case`.
    - `mime_type_validator.rs` — the allow-list lookup, the deny-list lookup (`is_mime_type_blocked`, NOT cited by any finding but the same defect) and the format-table `find` all ignore ASCII case now.

    The allow-list and deny-list lookups share one helper, `contains_mime_type`, which compares both sides with `eq_ignore_ascii_case`. That is stronger than lowercasing the query: `MimeTypePolicy`'s fields are `pub`, so a caller can insert an uppercase entry, and a query-only normalisation would still miss it.

    RED first, verified: four new tests failed with the expected assertions before the fix and pass after.
    - `test_blob_mime_type_consistency_skips_uppercase_opaque_binary` — a PNG payload declared `Application/Octet-Stream` was read as a spoofed content type.
    - `test_allow_list_lookup_is_case_insensitive` — `IMAGE/PNG`, `Audio/Wav`, `Text/Plain`.
    - `test_deny_list_lookup_is_case_insensitive` — `Application/JavaScript`.
    - `test_format_check_runs_for_uppercase_mime_type` — `AUDIO/WAV` now reaches the magic-byte check.

    ## Duplication

    - `mime_type_validator.rs` — the `validate_*_mime_type` trio and the `validate_*_format_matches_mime` pair are gone as parallel code. A `MimeCategory` const table (`IMAGE`, `AUDIO`, `RESOURCE`) holds the category name, the allow-list accessor, the reported categories, the suggestion text and an optional `FormatSpec`. One `validate_for_category` reads the table; each public method is a one-line delegation. `detect_image_format` / `detect_audio_format` became free functions, which is why their test call sites lost the `validator.` receiver.
    - `content_block_processor.rs` — the image/audio pair now share `decode_media_payload`, `decoded_content_metadata` and `build_media_content`. The text/blob resource pair now share `validate_and_record_resource_uri` and `resource_text_representation`.
    - `content_capability_validator.rs` — the Text and ResourceLink arms merged into one pattern. Image, Audio and Resource became an `OptionalCapability` const table read by one `check_optional_capability`. `supported_content_types` reads the same table instead of three parallel `if` blocks.

    ## Rust API design

    - `get_turn_request_count()` -> `turn_request_count()`, `get_turn_token_count()` -> `turn_token_count()`. Grepped the whole workspace: the only call sites outside the definition are 6 assertions in `tests/integration/coverage_tests.rs`, all updated. Two more `get_` getters that no finding named fell to the same rule in files I was already editing: `get_content_type_key` (now `ProcessedContentType::counting_key`) and `get_supported_content_types` (now `supported_content_types`).
    - `PathValidator::with_allowed_roots`, `with_blocked_paths`, `with_allowed_and_blocked` and the private `canonicalize_roots` take `impl IntoIterator<Item = PathBuf>`.
    - `Session::new` takes `impl AsRef<Path>`; `Message::new` takes `impl Into<String>`; both `update_available_commands` methods take `impl IntoIterator<Item = AvailableCommand>`.
    - Worth recording: every existing call site already passed `PathBuf`, `String` or `Vec`, so widening the bounds needed no call-site edit at all. Only the two renamed getters rippled.
    - `ContentBlockProcessor::new_with_config` no longer takes adjacent bools. A new `ContentValidationConfig` carries the five limits and switches as named fields, and `EnhancedSecurityConfig` now composes it rather than restating them.

    ## Documentation

    All eight named files now report ZERO under the lint the `missing-docs-rust` validator runs (`cargo clippy -p claude-agent --lib -- -W missing_docs`), down from 28 in `acp_error_conversion.rs`, 17 in `error.rs`, 13 in `session.rs`, 13 in `path_validator.rs` and 10 in `content_capability_validator.rs`. Enum variants and struct fields included, whole file swept, not only the cited lines.

    Debt that remains is in files no finding named, so it stays out of scope: `session_errors.rs` (100), `config.rs` (31), `terminal_manager.rs` (17), `mcp.rs` (11), `tools.rs` (9), `size_validator.rs` (9), `lib.rs` (7), `conversation_manager.rs` (7), `permissions.rs` (2), `protocol_translator.rs` (1).

    ## Magic numbers

    - `mime_type_validator.rs` — `IMAGE_HEADER_MIN_SIZE`, `PNG_SIGNATURE_SIZE`, `GIF_SIGNATURE_SIZE`, `RIFF_HEADER_SIZE`, `RIFF_SIGNATURE_SIZE`, `RIFF_FORMAT_OFFSET`, `AUDIO_HEADER_MIN_SIZE`, `AAC_HEADER_MIN_SIZE`, `FRAME_SYNC_FIRST_BYTE`, `MP3_SYNC_MASK`, `MP3_SYNC_PATTERN`, `AAC_SYNC_MASK`, `AAC_SYNC_PATTERN`. The repeated RIFF and frame-sync arithmetic collapsed into `is_riff_container` and `has_frame_sync`.
    - `content_security_validator.rs` — `ELF_EXECUTABLE_BASE64_PREFIX` and `PE_EXECUTABLE_BASE64_PREFIXES`. The finding named only `f0VMR`; `TVq` and `TVo` are the same cause in the same function, so they are named too, and the test data derives from the constants instead of restating them.
    - `acp_error_conversion.rs` — 20 hardcoded JSON-RPC codes replaced with `INVALID_PARAMS` / `INTERNAL_ERROR` from the crate's existing `json_rpc_codes` module. `grep -- "-326"` on that file now returns nothing.

    ## The `reuse` finding — investigated, they cannot share code

    `path_validator.rs:451` asked whether `validate_unix_permissions` duplicates `check_binary_permissions` in `apps/swissarmyhammer-cli/src/commands/doctor/checks.rs:82`. It cannot, for two independent reasons, and I recorded both as a doc comment on the method so the next reviewer does not repeat the investigation.

    1. Structure. `check_binary_permissions` is a private free function inside `apps/swissarmyhammer-cli`, a binary crate. `Cargo.toml` carries no dependency edge in either direction between that app and `crates/claude-agent`. A library cannot call a private item in a downstream application without inverting the dependency, which `ARCHITECTURE.md` forbids.
    2. Semantics. They answer different questions. `check_binary_permissions` tests whether ANY execute bit is set (`mode & 0o111`), to advise `chmod +x`, and reports by pushing a diagnostic `Check` record; a metadata read failure is silently ignored. `validate_unix_permissions` tests whether THIS process holds each requested permission, selecting the owner, group or other triple by comparing the file's uid/gid against the process uid/gid, and returns a typed error. Generalising one into the other would change a user-facing doctor message and lose the octal mode it prints.

    ## One thing worth knowing for the next round

    `ProcessedContentType` is now generated by a `processed_content_types!` macro that declares the enum and its counting keys from one table. Finding `:1179` asked for exactly this ("use a macro to generate both the enum variants and the mapping"), because Rust cannot index a const array by an enum variant that carries a payload. Variant doc comments survive through `$(#[$variant_doc:meta])*`. Adding a content kind is now one row.

    ## Verification

    - `cargo build -p claude-agent --all-targets` — clean, zero warnings.
    - `cargo nextest run -p claude-agent` — 1091 tests run, 1091 passed, 0 skipped (1087 before; the 4 new ones are the case-insensitivity regression tests).
    - `cargo clippy -p claude-agent --all-targets -- -D warnings` — clean, exit 0.
    - `cargo check -p swissarmyhammer-validators -p swissarmyhammer-agent -p acp-conformance --all-targets` — clean, exit 0.
    - `cargo fmt -p claude-agent -- --check` — clean.
    - `cargo clippy -p claude-agent --lib -- -W missing_docs` — zero for all eight named files.

    Environment note: no stale `cargo` or `cmake` process held the build lock this time. Only two `rust-analyzer` processes were running, and they did not block anything.

    ### implement — changed
    - evidence: 12 files — crates/claude-agent/src/{acp_error_conversion.rs, base64_processor.rs, content_block_processor.rs, content_capability_validator.rs, content_security_integration_tests.rs, content_security_validator.rs, error.rs, mime_type_validator.rs, path_validator.rs, session.rs}, crates/claude-agent/tests/integration/coverage_tests.rs. All 111 checklist items in the 2026-08-08 section marked `- [x]`. `base64_processor.rs` carries a one-hunk `cargo fmt` normalisation only.
    - next: ready for `/review`
  timestamp: 2026-08-08T18:59:13.555918+00:00
- actor: claude-code
  id: 01kzhc7390n56nqj034h7kekyy
  text: |-
    ### test — green

    I ran the full workspace test gate. All tests pass. No warnings. No skipped tests.

    Steps:
    1. I checked for old cargo or cmake processes. I found none. I did not need to kill any process.
    2. I searched the whole workspace for the renamed symbols: `get_turn_request_count`, `get_turn_token_count`, `turn_request_count`, `turn_token_count`, `ContentValidationConfig`, `new_with_config`, `ProcessedContentType`, `processed_content_types!`, `contains_mime_type`. All uses stay inside `crates/claude-agent`. The one match outside this crate (`GitHubConverter::new_with_config` in `crates/markdowndown`) is a different, unrelated method. The API change does not break other crates.
    3. I ran `cargo nextest run --workspace`. Result: 13649 tests run, 13649 passed, 0 skipped, 0 failed. Duration: 126.786s.
    4. I ran `cargo clippy --workspace --all-targets --all-features -- -D warnings`. Result: clean build, no warning lines, no error lines, exit code 0. This run also compiled and checked `swissarmyhammer-tools`, which depends on `claude-agent` and was not part of the earlier crate-scope check.

    - evidence: `cargo nextest run --workspace` — 13649 passed, 0 failed, 0 skipped; `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean, 0 warnings
    - next: The task can move to review.
  timestamp: 2026-08-08T19:04:54.304125+00:00
- actor: claude-code
  id: 01kzhc97zs3ggf8vhgab7nsvet
  text: |-
    ### commit — changed
    - evidence: 5f8a07a8e fix(claude-agent): make MIME checks case-insensitive per RFC 2045, close 111 review findings
    - confirmed HEAD before commit was f75efff68 (as expected); working tree matched iteration 3 scope only (MIME/content validators, error conversion, session, path validator, kanban task state); no scratch or unrelated files; no secrets in diff
    - next: leave in doing; awaiting review step
  timestamp: 2026-08-08T19:06:04.665237+00:00
- actor: claude-code
  id: 01kzhe4vdr87akxwpw07ktkw8f
  text: |-
    ### review — findings

    - scope: `review sha HEAD~1..HEAD` — the checkpoint commit `5f8a07a8e` only, not the accumulated task diff. Verified `git rev-parse HEAD` = `5f8a07a8e9a8d959008c05a50b9ddb2149df8d3f` before the run.
    - evidence: engine counts `findings: 39, confirmed: 39, refuted: 24, attempted: 44, failed: 0, skipped: 1`. 19 recorded as open checklist items in the new `## Review Findings (2026-08-08 14:06)` section. The 236 items in the two earlier sections stay checked and untouched.
    - the finding count fell from 111 to 19. The engine is working, and it is finding new ground each round, not the same ground.

    ## Guardrail answer 1 — repeats

    No open finding repeats. None of the 19 open file:line sites appears in the `## Review Findings (2026-08-04 15:45)` section or the `## Review Findings (2026-08-08 12:56)` section. I compared each site against both sections by an exact `file:line` match.

    Six DROPPED findings repeat, and only from the 12:56 round: `coverage_tests.rs:1`, `:336`, `:540`, `:563`, `:724`, `:1430`. They recur because the skill's pre-existing-test exemption drops them, so no round made them requirements and no round fixed them. They are on their second round, not their third. The 2026-08-04 section names `coverage_tests.rs:161`, `:167`, `:227` and `:2399`, which are the resolved MCP-acronym conflict items; none of those returned.

    Nothing has survived three rounds. The guardrail does NOT trip.

    ## Guardrail answer 2 — the over-cap file recurred

    `crates/claude-agent/tests/integration/coverage_tests.rs` was skipped again. It is 365790 rendered bytes against the 262144-byte per-file cap, so the `duplication` validator could not read it. Last round it was 365706 bytes. It grew by 84 bytes and is still 103646 bytes over.

    This is a review coverage gap, NOT a clean pass on that file. The `duplication` validator has now failed to read it two rounds in a row. Nothing in the commit made it smaller.

    ## The resolved conflict did not return

    The engine raised no lowercase-Display finding. `MCP`, `MIME`, `Base64` and `LoadSession` at the start of Display messages were not reported. The carve-out in the deployed rule is holding.

    ## What the 19 open findings are

    - 7 in `acp_error_conversion.rs` — the five `convert_*_error_to_acp` wrappers are still near-verbatim copies of one generic delegation, plus missing `Clone` on `ContentProcessingError` and missing `PartialEq`/`Eq` on `ErrorContext`.
    - 7 in `base64_processor.rs` — four duplicate `SizeValidator` constructions, the `decode_image_data`/`decode_audio_data` pair, adjacent bool parameters in `new_with_config`, and two same-typed size parameters that invite a swap. Note: `ContentValidationConfig` was introduced this round for `ContentBlockProcessor`; the finding at `:295` asks for the same treatment in `Base64Processor`, which did not get it.
    - 2 in `content_block_processor.rs` — missing `Clone` on `EnhancedSecurityConfig` and `ProcessedContent`.
    - 2 in `content_security_validator.rs` — the `strict`/`moderate`/`permissive` policy builders are three near-identical blocks, and two match arms run the same statement.
    - 1 in `session.rs:305` — `Session::new` panics on a non-absolute path. The error-handling rule reserves panics for internal invariant violations. This is a separate rule from the resolved capitalisation carve-out.

    A theme worth naming for the next iteration: the duplication findings sit in the SAME functions the last two rounds consolidated elsewhere in the file. Round 2 built `ContentValidationConfig` and shared media helpers; round 3 built `MimeCategory` and `OptionalCapability` tables. Neither round applied the same treatment to `acp_error_conversion.rs`'s five wrappers, `base64_processor.rs`'s four constructors, or `content_security_validator.rs`'s three policy builders. The cause was removed file by file, not everywhere it occurs.

    - next: task stays in `review` with 19 open findings.

    step: review
    outcome: findings
    evidence: 19 findings — crates/claude-agent/src/acp_error_conversion.rs:26, :210, :269, :277, :285, :293, :301; crates/claude-agent/src/base64_processor.rs:293, :295, :299, :325, :351, :449, :490; crates/claude-agent/src/content_block_processor.rs:56, :266; crates/claude-agent/src/content_security_validator.rs:369, :651; crates/claude-agent/src/session.rs:305. Plus one coverage gap: crates/claude-agent/tests/integration/coverage_tests.rs not read by the duplication validator, 365790 bytes over the 262144 cap.
    task: ^f70h1pr
  timestamp: 2026-08-08T19:38:37.880446+00:00
- actor: claude-code
  id: 01kzhe73j788ykwbvnbrbk26ch
  text: |-
    ### finish iteration 3 — findings

    - implement: changed — 11 source files, 1043 insertions, 466 deletions. Closed all 111 findings from the 12:56 round. Three RFC 2045 correctness bugs fixed TDD-first, plus a fourth site (`is_mime_type_blocked`) that no finding named but that carried the same cause.
    - test: green — `cargo nextest run --workspace` 13649 passed, 0 skipped; `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean. Workspace grep confirmed the public renames reach no other crate.
    - commit: 5f8a07a8e fix(claude-agent): make MIME checks case-insensitive per RFC 2045, close 111 review findings
    - review: findings — 19 open in a new `## Review Findings (2026-08-08 14:06)` section. Engine counts: findings 39, confirmed 39, refuted 24, attempted 44, failed 0, skipped 1. 20 dropped by the pre-existing-test exemption.

    Guardrail check: NOT triggered. The reviewer matched all 19 open findings against both prior sections; every one returned zero prior occurrences. Nothing has survived three rounds.

    Six DROPPED findings repeat from the 12:56 round (`coverage_tests.rs:1`, `:336`, `:540`, `:563`, `:724`, `:1430`). They recur because the written exemption drops them, so no round ever made them requirements. That is second-round, and they are not requirements, so they do not arm the guardrail.

    Trend: 112 → 111 → 19 open findings. The loop is converging.

    Open items for iteration 4:

    - `session.rs:305` panics on a non-absolute path. The error-handling rule reserves panics for internal invariant violations; a bad path is an expected failure mode. This is the one correctness item in the set.
    - The remaining duplication findings name code the last two rounds consolidated ELSEWHERE. Round 2 built `ContentValidationConfig` and the shared media helpers; round 3 built the `MimeCategory` and `OptionalCapability` tables. Neither round applied that treatment to the five `convert_*_error_to_acp` wrappers, the four duplicate `SizeValidator` constructions, or the three `SecurityPolicy` builders. The cause was removed file by file instead of everywhere it occurs — iteration 4 must sweep the pattern across the crate, not one file at a time.

    Standing coverage gap, second round running: `crates/claude-agent/tests/integration/coverage_tests.rs` is 365790 rendered bytes against the 262144-byte cap, so the `duplication` validator still cannot read it. It grew 84 bytes this round and sits 103646 bytes over. This file has never been read by that validator. Splitting it deserves its own card.
  timestamp: 2026-08-08T19:39:51.751670+00:00
- actor: claude-code
  id: 01kzhfbymn3xn5ct1qrhbsdgn5
  text: |-
    Iteration 4 — closed all 19 open findings in the `## Review Findings (2026-08-08 14:06)` section. The two earlier sections were already closed and were not touched.

    **Correctness item first — `session.rs:305`.** `Session::new` now returns `SessionSetupResult<Self>` instead of panicking on a relative path. TDD: rewrote `test_session_creation_with_relative_path_panics` as `test_session_creation_with_relative_path_returns_error`, watched it fail (`expected Session, found Result`), then changed the signature.
    - The absolute-path gate moved into `session_validation::require_absolute_working_directory`, so `Session::new` and `validate_working_directory` report the same typed error for the same path. The platform example string and the `absolute_path` requirement token are now named constants.
    - Blast radius: 17 call sites, all in claude-agent. Production sites are `SessionManager::create_session` (maps through a new shared `working_directory_rejected`, which also removed the duplicated "Working directory validation failed" format string) and `ClaudeAgent::rehydrate_in_memory_session` (maps to `SessionRestoreError::Corrupt`). The other 15 are tests and take `.expect(...)`. Nothing outside claude-agent calls it.

    **Crate-wide sweeps of the three named causes.**

    1. *The five `convert_*_error_to_acp` wrappers.* All five deleted. `add_error_context` was already the generic function the finding asked for, so keeping a sixth identical delegate would recreate the cause; it is renamed `convert_error_to_acp` and is now the only converter. The four wrapped error types were imported only for the wrappers, so those imports moved into the test module. Grepped the whole crate for the shape: `agent.rs::convert_session_setup_error_to_acp_error` is a real per-variant mapping, not a wrapper, and stays.

    2. *The four duplicate `SizeValidator` constructions.* Swept beyond base64_processor.rs. Every constructor of `Base64Processor` and of `ContentBlockProcessor` now lands in one private `from_parts`, so the `SizeValidator::new(SizeLimits { .. })` block exists once per file instead of four times and three times. `Default` for both types delegates there too. Crate-wide there are now five `SizeValidator::new` sites, each in a different file and each setting a different limit field — no file has two.

    3. *The three `SecurityPolicy` builders.* Replaced by a `SecurityPreset` table with three `const` rows and one `SecurityPolicy::from_preset`. `strict`, `moderate` and `permissive` are one line each. The five heuristic switches plus rate limiting were true/true/false in lockstep across the three presets, so the row carries one `enable_heuristics` field; the policy's own fields stay individually public and settable. URI patterns, IP ranges and the permissive rate budget became named constants.

    **Other findings.** `ContentProcessingError` derives `Clone`; `ErrorContext` derives `PartialEq, Eq`; `EnhancedSecurityConfig` and `ProcessedContent` derive `Clone`. `Base64Processor::new_with_config` and `with_enhanced_security_config` take one `Base64ValidationConfig` in place of two same-typed byte counts and two adjacent bools. `decode_image_data` and `decode_audio_data` are one-line forwards into `decode_media_data`, driven by a `MediaKind` table that mirrors the existing `MimeCategory` table in mime_type_validator.rs. The duplicated `ContentBlock::Resource` and `_` arms in `validate_content_blocks_security` became one expression match with a single `_` arm.

    **Deliberate non-changes, with reasons.**
    - The finding text for the converter suggested `ContentBlock::Resource(_) | _ =>`. That trips `clippy::wildcard_in_or_patterns` and would fail the `-D warnings` gate, so the two arms were consolidated into the single `_` arm instead. Same requirement, no new warning.
    - `SizeLimits::default/strict/permissive` have the same three-preset shape but contain no construction logic — each is a five-field data literal, already one row. Converting them would relocate code, not reduce it. `MimeTypePolicy::strict/moderate/permissive` already form a delta chain (moderate builds on strict, permissive on moderate), so there is no duplicated block there either.
    - `MimeTypeValidator` and `ContentSecurityValidator` preset methods are forwarding one-liners into the shared `new`, which the Rust duplication carve-out exempts.

    **New public API surface added:** `Base64ValidationConfig` (Debug, Clone, PartialEq, Eq, Default, `with_max_size`), `ContentValidationConfig::default` and `::with_resource_limit`, and `convert_error_to_acp`. `ContentValidationConfig` also gained `PartialEq, Eq` to match.

    Gates, all run after the final edit:
    - `cargo build -p claude-agent --all-targets` — clean
    - `cargo nextest run -p claude-agent` — 1091 run, 1091 passed, 0 skipped
    - `cargo clippy -p claude-agent --all-targets -- -D warnings` — clean
    - `cargo check -p swissarmyhammer-validators -p swissarmyhammer-agent -p acp-conformance --all-targets` — clean
    - `cargo fmt --check` — clean

    Note for the next round: `crates/claude-agent/tests/integration/coverage_tests.rs` is still over the per-file prompt cap, so the `duplication` validator still cannot read it. The four `Session::new` call sites in it grew by one line each. Splitting that file still needs its own card.

    ### implement — changed
    - evidence: 9 files — crates/claude-agent/src/session.rs, session_validation.rs, session_resume.rs, acp_error_conversion.rs, base64_processor.rs, content_block_processor.rs, content_security_validator.rs, agent.rs, tests/integration/coverage_tests.rs; 19 of 19 findings flipped to `- [x]`; 1091 tests pass
    - next: /review
  timestamp: 2026-08-08T19:59:59.125242+00:00
- actor: claude-code
  id: 01kzhg4jfbdxdn625gf6jfqm42
  text: |-
    ### test — green

    Workspace gate for iteration 4 (uncommitted, `Session::new` signature change + related deletions in claude-agent). Ran from repo root `/Users/wballard/github/swissarmyhammer/swissarmyhammer-main`.

    **Containment checks (before running the gate):**
    - `Session::new(` — grep `\bSession::new\(` workspace-wide. All 8 call sites are inside `crates/claude-agent` (`src/session.rs` x7, `src/session_resume.rs`, `src/agent.rs`, `tests/integration/coverage_tests.rs`). None outside the crate. Confirms the implementer's claim.
    - Deleted `convert_*_error_to_acp` (5 old names: `convert_protocol_error_to_acp`, `convert_io_error_to_acp`, `convert_validation_error_to_acp`, `convert_tool_error_to_acp`, `convert_permission_error_to_acp`) — zero matches anywhere in the workspace.
    - New `convert_error_to_acp` — defined and used only inside `crates/claude-agent/src/acp_error_conversion.rs`.
    - `from_preset` — found in `crates/claude-agent/src/content_security_validator.rs` (the new `SecurityPreset` table, as expected) and separately in `crates/markdowndown` (`html_config_from_preset`, an unrelated pre-existing function on a different type). No conflict.
    - `Base64Processor` / `ContentBlockProcessor` — only referenced inside `crates/claude-agent`. `swissarmyhammer-tools` depends on `claude-agent` (`Cargo.toml` workspace dep) but does not reference these constructors directly, so it only needed to compile against the new crate version — verified below.

    **Gate commands run:**
    1. `cargo nextest run --workspace` — 13649 tests run, 13649 passed, 0 skipped, 0 failed. Confirmed `swissarmyhammer-tools` tests (unit + `code_context_mcp_e2e_test` + `tools_tests` incl. `qwen_embedding_*` e2e) are present and passing in this run (e.g. `mcp::tools::code_context::tests::test_register_code_context_tools`, `code_context_mcp_e2e_test test_mcp_detects_deleted_files`, `tools_tests integration::semantic_search_e2e::qwen_embedding_semantic_search_e2e`).
    2. `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean, exit 0, 0 warnings. `swissarmyhammer-tools` compiled in this run ("Checking claude-agent" then downstream crates including `swissarmyhammer-tools` build confirmed).

    No failures found. No fixes needed. Task left in `doing`, not committed, not pushed, per instructions.
  timestamp: 2026-08-08T20:13:25.867112+00:00
position_column: doing
position_ordinal: '8380'
title: Lowercase the capitalized error Display messages in claude-agent
---
`builtin/validators/rust/rules/error-handling.md` states: Display messages on errors are lowercase, with no trailing punctuation.

`^p4mp9n6` swept swissarmyhammer-tools, mirdan and agent-client-protocol-extras to completion, and fixed the one named claude-agent site (`tools.rs` `unknown tool: {}`). The rest of claude-agent still starts its error messages with a capital.

## Size

- 135 `#[error("Capital...")]` attributes across 12 files. The biggest are `src/error.rs` (29), `src/session_errors.rs` (27), `src/content_block_processor.rs` (16), `src/content_security_validator.rs` (13), `src/acp_error_conversion.rs` (13), `src/base64_processor.rs` (12), `src/path_validator.rs` (11).
- 224 `AgentError::*(...)` construction sites, concentrated in `src/mcp.rs`.

## Rule to apply

Lowercase the first character unless the first word is:

- an all-caps acronym — `MCP`, `JSON`, `I/O`, `ACP`, `HTTP`, `SSE`, `URL`;
- a CamelCase identifier — `LoadSession`, `HookEvaluator`;
- a proper noun — `Git`, `Claude`.

Also strip a trailing full stop.

Leave capitalized, as `^p4mp9n6` did:

- the UI titles in `src/tool_classification.rs`, and the `assert_eq!(title, "Unknown tool")` tests that pin them;
- log lines (`tracing::*`);
- `.expect()` panic text.

## Risk

claude-agent errors surface as ACP protocol error payloads. Several tests assert the exact `to_string()` — for example `src/error.rs` `assert_eq!(err.to_string(), "Permission denied: access denied")` and `src/session_errors.rs` `assert_eq!(data["details"], "Missing required fields")`. Every such test must change in the same commit.

## Acceptance

- `rg '#\[error\("[A-Z]' crates/claude-agent/src` returns only acronym, CamelCase-identifier and proper-noun starts.
- `cargo nextest run -p claude-agent` green.
- `cargo clippy -p claude-agent --all-targets -- -D warnings` clean. #bug

## Review Findings (2026-08-04 15:45)

> ⚠️ 1/45 review tasks failed — results are INCOMPLETE.

- [x] `crates/claude-agent/src/acp_error_conversion.rs:182` — Function has condition-nesting depth of 4, which meets the gate threshold of 4 or more. The deeply nested if-let pattern makes the function harder to follow. Flatten nesting by extracting helper functions or using early returns for error cases.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:205` — Function has condition-nesting depth of 4, which meets the gate threshold of 4 or more. Refactor nested conditionals to reduce depth.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:205` — convert_base64_error_to_acp is a near-verbatim copy of convert_content_security_error_to_acp (line 182). Both contain identical context-addition logic (lines 189–198 and 212–221) that could drift if modified in one function but not the other. The only difference is the input error type. Extract a shared generic helper function that accepts any type implementing ToJsonRpcError, e.g., `fn add_error_context<T: ToJsonRpcError>(error: T, context: Option<ErrorContext>) -> JsonRpcError`. Replace all five convert_*_error_to_acp functions (lines 182, 205, 228, 251, 273) with calls to this helper.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:228` — Function has condition-nesting depth of 4, which meets the gate threshold of 4 or more. Refactor nested conditionals to reduce depth.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:228` — convert_mime_type_error_to_acp is a near-verbatim copy of convert_content_security_error_to_acp. Only the error type parameter differs; all other logic is identical and maintainable only if kept perfectly in sync across five copies. Consolidate into a single generic helper function as described above.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:251` — Function has condition-nesting depth of 4, which meets the gate threshold of 4 or more. Refactor nested conditionals to reduce depth.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:251` — convert_content_block_error_to_acp is a near-verbatim copy of convert_content_security_error_to_acp. Only the error type parameter differs. Consolidate into a single generic helper function.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:273` — Function has condition-nesting depth of 4, which meets the gate threshold of 4 or more. Refactor nested conditionals to reduce depth.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:273` — convert_content_processing_error_to_acp is a near-verbatim copy of convert_content_security_error_to_acp. Only the error type parameter differs; the context-addition logic is identical. Consolidate into a single generic helper function to eliminate maintenance burden and drift risk across five copies.
- [x] `crates/claude-agent/src/base64_processor.rs:13` — Public enum Base64ProcessorError lacks documentation. Document each variant's meaning. Add doc comment explaining the enum's error scenarios.
- [x] `crates/claude-agent/src/base64_processor.rs:139` — Public struct Base64Processor lacks documentation. Public types should explain their purpose and construction. Add doc comment explaining the processor's role and security features.
- [x] `crates/claude-agent/src/base64_processor.rs:139` — Public struct `Base64Processor` lacks doc comment. Add a doc comment explaining the struct's purpose and role in base64 processing.
- [x] `crates/claude-agent/src/base64_processor.rs:139` — Public struct `Base64Processor` does not implement `Debug`. All public types must implement Debug. Add `Debug` to the derive macro: `#[derive(Clone, Debug)]`.
- [x] `crates/claude-agent/src/base64_processor.rs:189` — Public function Base64Processor::new lacks documentation. Add doc comment.
- [x] `crates/claude-agent/src/base64_processor.rs:189` — Public method `new` lacks doc comment. Add a doc comment explaining constructor parameters and behavior.
- [x] `crates/claude-agent/src/base64_processor.rs:201` — Public function Base64Processor::new_with_config lacks documentation. Add doc comment.
- [x] `crates/claude-agent/src/base64_processor.rs:201` — Public method `new_with_config` lacks doc comment. Add a doc comment explaining the method's purpose and configuration parameters.
- [x] `crates/claude-agent/src/base64_processor.rs:225` — Public function Base64Processor::with_enhanced_security lacks documentation. Add doc comment.
- [x] `crates/claude-agent/src/base64_processor.rs:225` — Public method `with_enhanced_security` lacks doc comment. Add a doc comment explaining enhanced security configuration.
- [x] `crates/claude-agent/src/base64_processor.rs:242` — Public function Base64Processor::with_enhanced_security_config lacks documentation. Add doc comment.
- [x] `crates/claude-agent/src/base64_processor.rs:242` — Public method `with_enhanced_security_config` lacks doc comment. Add a doc comment explaining the method's configuration behavior.
- [x] `crates/claude-agent/src/base64_processor.rs:309` — Implicit 50% null-byte threshold checked via division by `2` lacks an explanation. The threshold for detecting suspicious null patterns should be explicitly named. Extract to a named constant like `const NULL_BYTE_THRESHOLD_RATIO: usize = 2;` and add a comment explaining the 50% suspicious pattern threshold.
- [x] `crates/claude-agent/src/base64_processor.rs:317` — Hardcoded minimum buffer size `4` for ELF header check should be a named constant. This represents the minimum bytes needed to read the '\x7fELF' magic signature. Extract `4` to a named constant like `const ELF_HEADER_MIN_SIZE: usize = 4;`.
- [x] `crates/claude-agent/src/base64_processor.rs:323` — Hardcoded minimum buffer size `4` for full Mach-O header check should be a named constant. This represents the minimum bytes needed to read the full Mach-O magic signature. Extract `4` to a named constant like `const MACHO_FULL_HEADER_MIN_SIZE: usize = 4;`.
- [x] `crates/claude-agent/src/base64_processor.rs:330` — Public function Base64Processor::decode_image_data lacks documentation. Add doc comment explaining parameters and error conditions.
- [x] `crates/claude-agent/src/base64_processor.rs:330` — Public method `decode_image_data` lacks doc comment. Add a doc comment explaining the method's purpose, parameters, and possible errors.
- [x] `crates/claude-agent/src/base64_processor.rs:339` — Enhanced security validation block (lines 339–343) is a near-verbatim copy of lines 373–377 and 414–418 in decode_audio_data and decode_blob_data. All three blocks are identical except for the content type string ('image', 'audio', 'blob'). This logic could drift if modified in one place but not the others. Extract a shared helper function: `fn validate_enhanced_security(&self, data: &str, content_type: &str) -> Result<(), Base64ProcessorError>` and call it from all three decode methods.
- [x] `crates/claude-agent/src/base64_processor.rs:364` — Public function Base64Processor::decode_audio_data lacks documentation. Add doc comment.
- [x] `crates/claude-agent/src/base64_processor.rs:364` — Public method `decode_audio_data` lacks doc comment. Add a doc comment explaining the method's purpose and error conditions.
- [x] `crates/claude-agent/src/base64_processor.rs:398` — Public function Base64Processor::decode_blob_data lacks documentation. Add doc comment.
- [x] `crates/claude-agent/src/base64_processor.rs:398` — Public method `decode_blob_data` lacks doc comment. Add a doc comment explaining the method's purpose for generic blob decoding.
- [x] `crates/claude-agent/src/base64_validation.rs:26` — Public enum Base64ValidationError lacks documentation. Add doc comment explaining each error variant.
- [x] `crates/claude-agent/src/base64_validation.rs:26` — Public enum `Base64ValidationError` lacks doc comment. Add a doc comment explaining the error enum's purpose and variants.
- [x] `crates/claude-agent/src/content_block_processor.rs:26` — Public enum ContentBlockProcessorError lacks documentation. Add doc comment explaining error variants.
- [x] `crates/claude-agent/src/content_block_processor.rs:26` — Public enum `ContentBlockProcessorError` lacks doc comment. Add a doc comment explaining the error type and its variants.
- [x] `crates/claude-agent/src/content_block_processor.rs:177` — Public struct ProcessedContent lacks documentation. Add doc comment explaining the struct's fields and purpose.
- [x] `crates/claude-agent/src/content_block_processor.rs:177` — Public struct `ProcessedContent` lacks doc comment. Add a doc comment explaining the struct's fields and purpose.
- [x] `crates/claude-agent/src/content_block_processor.rs:186` — Public enum ProcessedContentType lacks documentation. Add doc comment.
- [x] `crates/claude-agent/src/content_block_processor.rs:186` — Public enum `ProcessedContentType` lacks doc comment. Add a doc comment explaining the content type variants.
- [x] `crates/claude-agent/src/content_block_processor.rs:208` — Public struct ContentBlockProcessor lacks documentation. Add doc comment explaining the processor's role.
- [x] `crates/claude-agent/src/content_block_processor.rs:208` — Public struct `ContentBlockProcessor` lacks doc comment. Add a doc comment explaining the processor's role in content block handling.
- [x] `crates/claude-agent/src/content_block_processor.rs:208` — Public struct `ContentBlockProcessor` does not implement `Debug`. All public types must implement Debug. Add a derive clause with Debug: `#[derive(Debug)]` or implement Debug manually.
- [x] `crates/claude-agent/src/content_block_processor.rs:242` — Public function ContentBlockProcessor::new lacks documentation. Add doc comment.
- [x] `crates/claude-agent/src/content_block_processor.rs:242` — Public method `new` lacks doc comment. Add a doc comment explaining constructor parameters and behavior.
- [x] `crates/claude-agent/src/content_block_processor.rs:260` — Public function ContentBlockProcessor::new_with_config lacks documentation. Add doc comment.
- [x] `crates/claude-agent/src/content_block_processor.rs:260` — Public method `new_with_config` lacks doc comment. Add a doc comment explaining configuration parameters.
- [x] `crates/claude-agent/src/content_block_processor.rs:284` — Public function ContentBlockProcessor::with_enhanced_security lacks documentation. Add doc comment.
- [x] `crates/claude-agent/src/content_block_processor.rs:284` — Public method `with_enhanced_security` lacks doc comment. Add a doc comment explaining enhanced security configuration.
- [x] `crates/claude-agent/src/content_block_processor.rs:304` — Public function ContentBlockProcessor::with_enhanced_security_config lacks documentation. Add doc comment.
- [x] `crates/claude-agent/src/content_block_processor.rs:304` — Public method `with_enhanced_security_config` lacks doc comment. Add a doc comment explaining the configuration method's purpose.
- [x] `crates/claude-agent/src/content_block_processor.rs:432` — Function process_content_block_internal has cognitive complexity of 42, which far exceeds the gate of 15. The function handles 5 different content block types with complex nested matching and processing logic (24 branches with max nesting depth 3), making it difficult to reason about and test. Extract processing logic for each content block type into separate helper functions (e.g., process_text_content is already extracted; do similar for Image, Audio, Resource, ResourceLink). This will reduce the main function's complexity and improve testability.
- [x] `crates/claude-agent/src/content_block_processor.rs:803` — Content accumulation logic (lines 803–816) is a verbatim copy of lines 852–865 in process_content_blocks_with_recovery. Both blocks perform identical accumulation of processed content results, text, binary flags, and type counts. This logic could drift if modified in one function but not the other. Extract a shared helper function: `fn accumulate_processed_content(&mut self, processed: ProcessedContent) { ... }` that updates text_content, has_binary_content, total_size, content_type_counts, and processed_contents. Call this helper from both process_content_blocks_strict and process_content_blocks_with_recovery.
- [x] `crates/claude-agent/src/content_block_processor.rs:848` — Hardcoded retry count `3` should be a named constant. Retry limits are configuration values that affect behavior and maintainability. Extract `3` to a named constant like `const MAX_RETRIES: u32 = 3;` at the top of the impl block or module.
- [x] `crates/claude-agent/src/content_block_processor.rs:907` — Function process_content_block_with_retry has cognitive complexity of 15 and condition-nesting depth of 4, both meeting their respective gates. The retry loop with nested error handling and conditional checks makes the function harder to follow. Extract the retry decision logic and error classification into separate helper functions to reduce nesting depth.
- [x] `crates/claude-agent/src/content_block_processor.rs:917` — Hardcoded maximum backoff limit `10000` (milliseconds) should be a named constant. Time limits are configuration values affecting retry behavior. Extract `10000` to a named constant like `const MAX_BACKOFF_MS: u64 = 10000;`.
- [x] `crates/claude-agent/src/content_block_processor.rs:917` — Hardcoded millisecond-to-second conversion factor `1000` should be a named constant. While conventional, using a constant makes the intent explicit. Extract `1000` to a named constant like `const MS_PER_SECOND: u64 = 1000;`.
- [x] `crates/claude-agent/src/content_capability_validator.rs:93` — Public struct `ContentCapabilityValidator` does not implement `Debug`. All public types must implement Debug. Add a derive clause: `#[derive(Debug)]` to the struct.
- [x] `crates/claude-agent/src/content_security_validator.rs:17` — Public enum ContentSecurityError lacks a doc comment; all public items must be documented per Rust conventions. Add a doc comment above the enum explaining its purpose and the errors it represents.
- [x] `crates/claude-agent/src/content_security_validator.rs:179` — Public enum `SecurityLevel` lacks documentation explaining the three validation levels (Strict/Moderate/Permissive). Add doc comment documenting each security level variant and when to use each.
- [x] `crates/claude-agent/src/content_security_validator.rs:179` — Public enum SecurityLevel lacks a doc comment; all public items must be documented. Add a doc comment explaining the three security levels and their intended use.
- [x] `crates/claude-agent/src/content_security_validator.rs:191` — Public struct `SecurityPolicy` lacks documentation explaining configuration options for content security validation. Add doc comment documenting the policy configuration and its fields.
- [x] `crates/claude-agent/src/content_security_validator.rs:191` — Public struct SecurityPolicy lacks a doc comment; all public items must be documented. Add a doc comment explaining SecurityPolicy's role in configuring security validation.
- [x] `crates/claude-agent/src/content_security_validator.rs:210` — Public method `SecurityPolicy::strict()` lacks documentation. Add doc comment explaining that this creates a strict security policy with tighter constraints.
- [x] `crates/claude-agent/src/content_security_validator.rs:218` — Hardcoded limit 10 for strict policy max_content_array_length lacks explanation. Define `const STRICT_MAX_CONTENT_ARRAY_LENGTH: usize = 10;` at module level.
- [x] `crates/claude-agent/src/content_security_validator.rs:241` — Hardcoded limit 60 for strict policy rate_limit_requests_per_minute lacks explanation. Define `const STRICT_RATE_LIMIT_REQUESTS_PER_MINUTE: u32 = 60;` at module level.
- [x] `crates/claude-agent/src/content_security_validator.rs:245` — Public method `SecurityPolicy::moderate()` lacks documentation. Add doc comment explaining that this creates a moderate security policy balancing security and flexibility.
- [x] `crates/claude-agent/src/content_security_validator.rs:255` — Hardcoded limit 50 for moderate policy max_content_array_length lacks explanation. Define `const MODERATE_MAX_CONTENT_ARRAY_LENGTH: usize = 50;` at module level.
- [x] `crates/claude-agent/src/content_security_validator.rs:266` — Hardcoded limit 300 for moderate policy rate_limit_requests_per_minute lacks explanation. Define `const MODERATE_RATE_LIMIT_REQUESTS_PER_MINUTE: u32 = 300;` at module level.
- [x] `crates/claude-agent/src/content_security_validator.rs:270` — Public method `SecurityPolicy::permissive()` lacks documentation. Add doc comment explaining that this creates a permissive security policy with minimal restrictions.
- [x] `crates/claude-agent/src/content_security_validator.rs:299` — Public struct `ContentSecurityValidator` lacks documentation explaining its purpose and usage. Add doc comment documenting the validator's role in ACP content validation.
- [x] `crates/claude-agent/src/content_security_validator.rs:299` — Public struct ContentSecurityValidator lacks a doc comment; all public items must be documented. Add a doc comment explaining ContentSecurityValidator's purpose and typical usage.
- [x] `crates/claude-agent/src/content_security_validator.rs:328` — Public method `ContentSecurityValidator::new()` lacks documentation. Add doc comment explaining that this creates a validator from a custom security policy and documenting error cases.
- [x] `crates/claude-agent/src/content_security_validator.rs:328` — Public method ContentSecurityValidator::new lacks a doc comment; all public items must be documented. Add a doc comment explaining that this creates a new validator with the given policy and returns an error if regex patterns are invalid.
- [x] `crates/claude-agent/src/content_security_validator.rs:356` — Public method `ContentSecurityValidator::strict()` lacks documentation. Add doc comment explaining that this creates a strict validator with tighter security constraints.
- [x] `crates/claude-agent/src/content_security_validator.rs:356` — Public method ContentSecurityValidator::strict lacks a doc comment; all public items must be documented. Add a doc comment explaining this creates a validator with strict security policy.
- [x] `crates/claude-agent/src/content_security_validator.rs:360` — Public method `ContentSecurityValidator::moderate()` lacks documentation. Add doc comment explaining that this creates a moderate validator balancing security and flexibility.
- [x] `crates/claude-agent/src/content_security_validator.rs:360` — Public method ContentSecurityValidator::moderate lacks a doc comment; all public items must be documented. Add a doc comment explaining this creates a validator with moderate security policy.
- [x] `crates/claude-agent/src/content_security_validator.rs:364` — Public method `ContentSecurityValidator::permissive()` lacks documentation. Add doc comment explaining that this creates a permissive validator with minimal restrictions.
- [x] `crates/claude-agent/src/content_security_validator.rs:364` — Public method ContentSecurityValidator::permissive lacks a doc comment; all public items must be documented. Add a doc comment explaining this creates a validator with permissive security policy.
- [x] `crates/claude-agent/src/content_security_validator.rs:368` — Public method `ContentSecurityValidator::policy()` lacks documentation. Add doc comment explaining that this returns the underlying security policy.
- [x] `crates/claude-agent/src/content_security_validator.rs:368` — Public method ContentSecurityValidator::policy lacks a doc comment; all public items must be documented. Add a doc comment explaining this returns a reference to the underlying SecurityPolicy.
- [x] `crates/claude-agent/src/content_security_validator.rs:457` — Unexplained magic numbers 3 and 4 for base64 decoding math (Audio block). Add comment explaining base64 ratio: `// Base64 encoded size is ~4/3 of actual size` or extract to named constant.
- [x] `crates/claude-agent/src/content_security_validator.rs:505` — Unexplained magic numbers 3 and 4 for base64 decoding estimation. Add inline comment explaining base64 math or extract to named constant like `BASE64_DECODE_RATIO: f64 = 3.0 / 4.0;`.
- [x] `crates/claude-agent/src/content_security_validator.rs:623` — Function exceeds cognitive complexity gate: `validate_resource_content` has complexity 22 (gate 15) with condition-nesting depth 5 (gate 4). The complexity stems from nested pattern matching on resource types with multiple validation branches per arm. Refactor to reduce nesting depth by extracting validation logic for text resources and blob resources into separate helper methods.
- [x] `crates/claude-agent/src/content_security_validator.rs:815` — Unexplained hardcoded threshold 100 for minimum data length. Define `const MIN_DATA_LENGTH_FOR_REPETITION_CHECK: usize = 100;` with explanatory comment.
- [x] `crates/claude-agent/src/error.rs:163` — Hardcoded JSON-RPC error code -32600 should be a named constant. Define `const JSON_RPC_INVALID_REQUEST: i32 = -32600;` following JSON-RPC 2.0 spec.
- [x] `crates/claude-agent/src/error.rs:165` — Hardcoded JSON-RPC error code -32000 should be a named constant. Define `const JSON_RPC_SERVER_ERROR: i32 = -32000;` following JSON-RPC 2.0 spec.
- [x] `crates/claude-agent/src/error.rs:229` — Verbatim duplication of Serialize impl for McpError (lines 148–158); both implementations are identical except for the struct name string parameter. This is one generic function with an argument waiting to be extracted, not N separate impls that must be kept in lockstep. Extract a generic helper function `fn serialize_error<S, T: Display>(serializer: S, struct_name: &str, error: &T) -> Result<S::Ok, S::Error> where S: serde::Serializer { ... }` and call it from both impls. This eliminates the duplication and ensures any future fix is applied once, not twice.
- [x] `crates/claude-agent/src/error.rs:244` — Hardcoded JSON-RPC error code -32000 should be a named constant. Use named constant `JSON_RPC_SERVER_ERROR` defined at module level.
- [x] `crates/claude-agent/src/error.rs:253` — Hardcoded JSON-RPC error code -32000 should use named constant. Use named constant `JSON_RPC_SERVER_ERROR` instead of -32000.
- [x] `crates/claude-agent/src/error.rs:255` — Hardcoded JSON-RPC error code -32603 should use named constant. Use named constant `JSON_RPC_INTERNAL_ERROR` instead of -32603.
- [x] `crates/claude-agent/src/mime_type_validator.rs:7` — Public enum `MimeTypeValidationError` lacks documentation explaining MIME type validation failures. Add doc comment documenting the error variants and what validation failures they represent.
- [x] `crates/claude-agent/src/mime_type_validator.rs:7` — Public enum MimeTypeValidationError lacks a doc comment; all public items must be documented. Add a doc comment explaining the types of MIME type validation errors that can occur.
- [x] `crates/claude-agent/src/mime_type_validator.rs:107` — Public struct `MimeTypePolicy` lacks documentation explaining content validation policy configuration. Add doc comment documenting the policy structure and configuration fields.
- [x] `crates/claude-agent/src/mime_type_validator.rs:107` — Public struct MimeTypePolicy lacks a doc comment; all public items must be documented. Add a doc comment explaining MimeTypePolicy's role in configuring MIME type validation rules.
- [x] `crates/claude-agent/src/mime_type_validator.rs:221` — Public struct `MimeTypeValidator` lacks documentation explaining its purpose and usage. Add doc comment documenting the validator's role in MIME type validation.
- [x] `crates/claude-agent/src/mime_type_validator.rs:221` — Public struct MimeTypeValidator lacks a doc comment; all public items must be documented. Add a doc comment explaining MimeTypeValidator's purpose and usage in validating MIME types.
- [x] `crates/claude-agent/src/mime_type_validator.rs:261` — Public method `validate_image_mime_type()` lacks documentation explaining parameters and validation behavior. Add doc comment documenting the mime_type parameter, data parameter, validation rules, and return value.
- [x] `crates/claude-agent/src/mime_type_validator.rs:261` — Public method MimeTypeValidator::validate_image_mime_type lacks a doc comment; all public items must be documented. Add a doc comment explaining this validates MIME type for image content and optionally validates format matching.
- [x] `crates/claude-agent/src/mime_type_validator.rs:295` — Public method `validate_audio_mime_type()` lacks documentation explaining parameters and validation behavior. Add doc comment documenting the mime_type parameter, data parameter, validation rules, and return value.
- [x] `crates/claude-agent/src/mime_type_validator.rs:295` — Near-verbatim duplication of validate_audio_mime_type with validate_image_mime_type (lines 261–293); both implementations are identical except for the allowed types set (`allowed_audio_types` vs `allowed_image_types`), the category name ("audio" vs "image"), and the format validation function called. This is one function with three parameters that should be extracted. Extract a generic `fn validate_mime_type_for_category(&self, mime_type: &str, data: Option<&[u8]>, category: &str, allowed_types: &HashSet<String>, validate_format: fn(&self, &[u8], &str) -> Result<(), MimeTypeValidationError>) -> Result<(), MimeTypeValidationError>` method. Call it from both `validate_image_mime_type` and `validate_audio_mime_type` with the appropriate arguments. This eliminates the duplication and ensures bugs are fixed once.
- [x] `crates/claude-agent/src/mime_type_validator.rs:295` — validate_audio_mime_type() reinvents validate_image_mime_type() logic: both check security blocking, validate against an allowlist (allowed_audio_types vs allowed_image_types), optionally call a format-validation method (validate_audio_format_matches_mime vs validate_image_format_matches_mime), and return identical error structures. The only differences are the category name and policy field. Should have parameterized or extended the first method instead of copying it. Consolidate validate_image_mime_type and validate_audio_mime_type into a single parameterized method that accepts the category name, allowed-types HashSet ref, and format-validation callback; or extract the common logic into a helper and call it from both.
- [x] `crates/claude-agent/src/mime_type_validator.rs:295` — Public method MimeTypeValidator::validate_audio_mime_type lacks a doc comment; all public items must be documented. Add a doc comment explaining this validates MIME type for audio content and optionally validates format matching.
- [x] `crates/claude-agent/src/mime_type_validator.rs:329` — Public method `validate_resource_mime_type()` lacks documentation explaining parameters and validation behavior. Add doc comment documenting the mime_type parameter and validation rules.
- [x] `crates/claude-agent/src/mime_type_validator.rs:329` — validate_resource_mime_type() copies the security-check and allowlist-validation logic from validate_image_mime_type (lines 261–293) without extracting a shared helper: security blocking check → type allowlist check → optional format validation. The only difference is that resource_mime_type omits format validation. Three similar methods with minor variations signal that a unified validator should have been written once. Extract the common validation pattern into a generic helper; or generalize validate_image_mime_type to handle all three categories by accepting category, allowed_types set, and optional format-validator closure.
- [x] `crates/claude-agent/src/mime_type_validator.rs:329` — Public method MimeTypeValidator::validate_resource_mime_type lacks a doc comment; all public items must be documented. Add a doc comment explaining this validates MIME type for embedded resource content without binary format validation.
- [x] `crates/claude-agent/src/mime_type_validator.rs:416` — Near-verbatim duplication of validate_audio_format_matches_mime with validate_image_format_matches_mime (lines 379–414); both implementations are identical except for the detection function called (`detect_audio_format` vs `detect_image_format`) and the mime_type-to-format mapping. This is one function with parameters that should be extracted. Extract a generic `fn validate_format_matches_mime(&self, data: &[u8], mime_type: &str, detect_fn: fn(&self, &[u8]) -> Option<String>, format_map: &[(String, String)]) -> Result<(), MimeTypeValidationError>` method. Call it from both `validate_image_format_matches_mime` and `validate_audio_format_matches_mime` with the appropriate detection function and format mapping. This eliminates the duplication.
- [x] `crates/claude-agent/src/mime_type_validator.rs:416` — validate_audio_format_matches_mime() reinvents validate_image_format_matches_mime() (lines 379–414): both detect format from binary data, map MIME type to expected format, match on (expected, detected) tuples, and return FormatMismatch errors on mismatch. Only differences are the MIME→format mapping and which detect function (detect_audio_format vs detect_image_format). Should have parameterized this logic instead of copying it. Consolidate into a single validate_format_matches_mime(data, mime_type, mime_to_format_map, detect_fn) method that handles both image and audio by accepting the MIME-type mapping and detect function as parameters.
- [x] `crates/claude-agent/src/path_validator.rs:388` — Function exceeds cognitive complexity gate: `validate_permissions` has complexity 21 (gate 15). The complexity stems from nested conditional logic for permission checking across platform-specific code paths (Unix and Windows) with multiple permission type branches. Refactor permission validation logic into separate helper functions for Unix and Windows platforms, extracting the permission bit checking logic into dedicated methods.
- [x] `crates/claude-agent/src/session.rs:99` — Method `ulid_string()` performs an expensive operation (String allocation from borrowed reference) but does not follow the `to_` naming convention. Per the conversion naming rule: `to_` prefix for expensive borrow→owned conversions. Should be `to_ulid_string()` for consistency with `to_uuid_string()` and API expectations. Rename `ulid_string()` to `to_ulid_string()` to follow the API design convention for expensive borrow→owned conversions. Update all call sites and tests accordingly.
- [x] `crates/claude-agent/src/session.rs:464` — Hardcoded timeout value 300 (seconds) is a magic number that configures production behavior and should be a named constant, not a comment-explained literal. Define a named constant `const DEFAULT_CLEANUP_INTERVAL_SECS: u64 = 300;` and use `Duration::from_secs(DEFAULT_CLEANUP_INTERVAL_SECS)` instead.
- [x] `crates/claude-agent/src/session.rs:465` — Hardcoded timeout value 3600 (seconds) is a magic number that configures production behavior and should be a named constant, not a comment-explained literal. Define a named constant `const DEFAULT_MAX_SESSION_AGE_SECS: u64 = 3600;` and use `Duration::from_secs(DEFAULT_MAX_SESSION_AGE_SECS)` instead.

### Conflicts — resolved by the user, rule amended, no code change

The user read the conflict and decided it: the builtin rule now carries the carve-out. `builtin/validators/rust/rules/error-handling.md` reads:

> `Display` messages on errors: lowercase, no trailing punctuation. Do not change the first word when it is an all-caps acronym (`MCP`, `JSON`, `I/O`, `URL`), a CamelCase identifier (`LoadSession`), or a proper noun (`Git`, `Claude`). Keep acronyms in the middle of a message as written.

Every item below is therefore correct as it stands. The code does not change for any of them, and each is checked to record that decision.

- [x] `crates/claude-agent/src/error.rs:52` — validator flags `MCP server stdin not available` as violating the lowercase rule; conflicts with the task's stated all-caps-acronym exception (MCP is listed by name).
- [x] `crates/claude-agent/src/error.rs:73` — validator flags `MCP server error`; same MCP-acronym conflict.
- [x] `crates/claude-agent/src/error.rs:87` — validator flags `MCP connection closed unexpectedly`; same MCP-acronym conflict.
- [x] `crates/claude-agent/src/error.rs:94` — validator flags `MCP response missing result field`; same MCP-acronym conflict.
- [x] `crates/claude-agent/src/error.rs:101` — validator flags `MCP server initialization failed`; same MCP-acronym conflict.
- [x] `crates/claude-agent/src/error.rs:108` — validator flags `MCP server tools list request failed`; same MCP-acronym conflict.
- [x] `crates/claude-agent/src/mime_type_validator.rs:21` — validator flags `MIME type format validation failed`; MIME is an all-caps acronym under the same task exception, and was left capitalized everywhere else in this same file by this commit.
- [x] `crates/claude-agent/src/content_security_validator.rs:35` — validator flags `Base64 security violation`; this commit left `Base64` capitalized here while lowering every sibling variant in the same enum, matching the task's proper-noun/acronym exception.
- [x] `crates/claude-agent/src/session_errors.rs:149` — validator flags `LoadSession capability not supported`; `LoadSession` is named explicitly as a CamelCase-identifier exception in the task text.
- [x] `crates/claude-agent/tests/integration/coverage_tests.rs:161` — validator flags the `"MCP server process crashed"` assertion; mirrors the untouched, correctly-capitalized `McpError` Display text.
- [x] `crates/claude-agent/tests/integration/coverage_tests.rs:167` — validator flags the `"MCP server error"` assertion; same MCP-acronym conflict.
- [x] `crates/claude-agent/tests/integration/coverage_tests.rs:227` — validator flags the `"MCP message serialization"` assertion; same MCP-acronym conflict.
- [x] `crates/claude-agent/tests/integration/coverage_tests.rs:2399` — validator flags the `"invalid ULID"` assertion; ULID mid-string was deliberately preserved after lowering only the leading word, matching the acronym exception.

## Review Findings (2026-08-08 12:56)

> Scope: `review sha HEAD~1..HEAD` — the checkpoint commit `f75efff68` only, not the accumulated task diff.

> ⚠️ 1 file(s) not reviewed — the rendered prompt would exceed the agent's prompt cap:
> - `crates/claude-agent/tests/integration/coverage_tests.rs` — 365706 rendered bytes, over the 262144-byte per-file cap; not reviewed by: duplication (split the file)

The `error-handling` rule now carries the acronym / CamelCase / proper-noun carve-out, and the engine raised no lowercase-Display finding. The conflict recorded on 2026-08-04 did not recur.

- [x] `crates/claude-agent/src/acp_error_conversion.rs:24` — missing documentation for a variant.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:27` — missing documentation for a variant.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:28` — missing documentation for a struct field.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:29` — missing documentation for a struct field.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:33` — missing documentation for a variant.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:36` — missing documentation for a variant.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:36` — missing documentation for a struct field.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:39` — missing documentation for a variant.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:39` — missing documentation for a struct field.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:42` — missing documentation for a variant.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:42` — missing documentation for a struct field.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:45` — missing documentation for a variant.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:45` — missing documentation for a struct field.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:48` — missing documentation for a variant.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:51` — missing documentation for a variant.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:54` — missing documentation for a variant.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:54` — missing documentation for a struct field.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:57` — missing documentation for a variant.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:57` — missing documentation for a struct field.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:60` — missing documentation for a variant.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:60` — missing documentation for a struct field.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:63` — missing documentation for a variant.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:63` — missing documentation for a struct field.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:79` — Hardcoded JSON-RPC error code -32602 appears in a match arm, and the same value is defined as the constant INVALID_PARAMS elsewhere in the codebase (used in base64_processor.rs). The magic number should be replaced with the named constant to avoid repetition and ensure consistency. Import the constants from json_rpc_codes at the top of the file (`use crate::json_rpc_codes::{INTERNAL_ERROR, INVALID_PARAMS};`) and replace the hardcoded -32602 with INVALID_PARAMS on line 79 and -32603 with INTERNAL_ERROR on line 80.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:80` — Hardcoded JSON-RPC error code -32603 appears in a match arm, and the same value is defined as the constant INTERNAL_ERROR elsewhere in the codebase (used in base64_processor.rs). The magic number should be replaced with the named constant to avoid repetition and ensure consistency. Import the constants from json_rpc_codes at the top of the file (`use crate::json_rpc_codes::{INTERNAL_ERROR, INVALID_PARAMS};`) and replace the hardcoded -32603 with INTERNAL_ERROR on line 80.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:80` — Hardcoded JSON-RPC error code -32603 (Internal error) should use the named constant INTERNAL_ERROR from the json_rpc_codes module. Import INTERNAL_ERROR from crate::json_rpc_codes and replace `-32603` with `INTERNAL_ERROR`.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:162` — missing documentation for a struct field.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:163` — missing documentation for a struct field.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:164` — missing documentation for a struct field.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:165` — missing documentation for a struct field.
- [x] `crates/claude-agent/src/content_block_processor.rs:423` — Function has adjacent bool parameters (enable_uri_validation at parameter 3 and enable_capability_validation at parameter 4), making call sites unreadable: e.g., `new_with_config(proc, size, true, false, ...)` obscures which bool configures what. Extract configuration flags into a struct or enum: create a `ValidatorConfig { uri_validation: bool, capability_validation: bool, batch_recovery: bool }` and pass a single config parameter, or use a builder pattern for this constructor with multiple optional settings.
- [x] `crates/claude-agent/src/content_block_processor.rs:663` — Metadata initialization and insertion is duplicated identically in process_image_content (663-665) and process_audio_content (706-708); both create metadata HashMap and insert mime_type and data_size fields identically, differing only in variable names. Extract into a helper function: `fn create_decoded_content_metadata(mime_type: String, decoded_size: usize) -> HashMap<String, String>` that both process_image_content and process_audio_content can call.
- [x] `crates/claude-agent/src/content_block_processor.rs:681` — ProcessedContent construction is duplicated identically in process_image_content (681-689) and process_audio_content (716-724), differing only by content_type variant and the source object for mime_type and size_bytes fields. Extract into a helper method: `fn build_media_content(content_type: ProcessedContentType, text_representation: String, decoded_data: Vec<u8>, metadata: HashMap<String, String>, original_size: usize) -> ProcessedContent` that both functions call, reducing the 9-line duplication to a single function call.
- [x] `crates/claude-agent/src/content_block_processor.rs:758` — URI validation logic is duplicated identically in process_text_resource (758-760) and process_blob_resource (805-807); both validate the same way, differing only in variable names. Extract URI validation into a helper function: `fn validate_and_record_uri(&mut self, uri: &str, metadata: &mut HashMap<String, String>) -> Result<(), ContentBlockProcessorError>` that both functions can call.
- [x] `crates/claude-agent/src/content_block_processor.rs:763` — URI and MIME type metadata extraction is duplicated identically in process_text_resource (763-768) and process_blob_resource (824-829); both insert uri and mime_type metadata the same way, differing only in variable names. Extract metadata insertion into a helper function: `fn extract_resource_metadata(metadata: &mut HashMap<String, String>, uri: &str, mime_type: Option<&str>)` that both functions can call after URI validation.
- [x] `crates/claude-agent/src/content_block_processor.rs:778` — Text representation format string is duplicated identically in process_text_resource (778-783) and process_blob_resource (834-839), differing only by the resource type literal string and the size variable name. Extract into a helper method: `fn create_resource_text_representation(resource_type: &str, mime_type: Option<&str>, uri: &str, size: usize) -> String` that both functions call, replacing the 6-line duplicated format! calls with a single function call.
- [x] `crates/claude-agent/src/content_block_processor.rs:1179` — The `get_content_type_key` method (lines 1179–1187) is a match over ProcessedContentType variants where each arm has identical logic (return a string constant). The arms differ only in the enum variant matched and the constant string returned. This is a table (variant → key string) written as a match expression. Refactor to express the mapping as a data structure. Options: (1) Define a const array of tuples mapping variant names to key strings; (2) Use a helper function that builds the mapping once and reuses it; (3) Use a macro to generate both the enum variants and the mapping. This separates the data (the variant→key pairs) from the control flow (the match).
- [x] `crates/claude-agent/src/content_capability_validator.rs:15` — missing documentation for a variant.
- [x] `crates/claude-agent/src/content_capability_validator.rs:16` — missing documentation for a struct field.
- [x] `crates/claude-agent/src/content_capability_validator.rs:17` — missing documentation for a struct field.
- [x] `crates/claude-agent/src/content_capability_validator.rs:18` — missing documentation for a struct field.
- [x] `crates/claude-agent/src/content_capability_validator.rs:19` — missing documentation for a struct field.
- [x] `crates/claude-agent/src/content_capability_validator.rs:23` — missing documentation for a variant.
- [x] `crates/claude-agent/src/content_capability_validator.rs:23` — missing documentation for a struct field.
- [x] `crates/claude-agent/src/content_capability_validator.rs:26` — missing documentation for a variant.
- [x] `crates/claude-agent/src/content_capability_validator.rs:27` — missing documentation for a struct field.
- [x] `crates/claude-agent/src/content_capability_validator.rs:28` — missing documentation for a struct field.
- [x] `crates/claude-agent/src/content_capability_validator.rs:124` — Match arms for always-allowed content types are duplicated identically in Text (124-128) and ResourceLink (130-134) cases; both contain identical structure with debug message and Ok() return, differing only by the content type name in the debug string. Extract into a helper method: `fn allow_baseline_content_type(type_name: &str) -> Result<(), ContentCapabilityError> { debug!("{} content always allowed", type_name); Ok(()) }` and call it from both match arms, or consolidate the two arms into a single pattern `ContentBlock::Text(_) | ContentBlock::ResourceLink(_) => { ... }`.
- [x] `crates/claude-agent/src/content_capability_validator.rs:136` — The Image, Audio, and Resource branches of this match (lines 136–179) follow an identical control-flow pattern; only the constants differ (capability field name, content type string, and required capability name). This is a table written as parallel code, not a single code path interpreting data. Extract the capability checks into a table—either a match over a static array of tuples or a helper method that maps each content type to its capability field name and required capability string. This reduces the risk of drift between the three parallel arms and makes adding new content types require only a data entry, not parallel code duplication.
- [x] `crates/claude-agent/src/content_capability_validator.rs:166` — Capability validation logic is duplicated identically for Resource (166-179), Image (136-149), and Audio (151-164) match arms; all contain the same if-else pattern checking a capability flag and conditionally returning an error, differing only by capability field name, content_type string, and required_capability string. Extract into a helper method: `fn check_optional_capability(&self, enabled: bool, content_type: &str, capability_name: &str) -> Result<(), ContentCapabilityError>` and call it from each match arm, reducing the three 14-line blocks to three one-line calls.
- [x] `crates/claude-agent/src/content_security_validator.rs:885` — The new comparison `mime_type == OPAQUE_BINARY_MIME_TYPE` is case-sensitive, but MIME types are case-insensitive per RFC 2045. If a caller passes 'Application/Octet-Stream' (uppercase), the comparison fails to match the constant 'application/octet-stream' (lowercase), and the optimization to skip validation silently fails. No test covers uppercase MIME types. Use case-insensitive comparison: `if mime_type.eq_ignore_ascii_case(OPAQUE_BINARY_MIME_TYPE)`. Add one test passing 'Application/Octet-Stream' and verify the function returns Ok().
- [x] `crates/claude-agent/src/content_security_validator.rs:1017` — Magic byte string 'f0VMR' (ELF executable header) hardcoded without explanation or named constant. This is a specific binary signature that should be extracted. Define `const ELF_EXECUTABLE_HEADER: &str = "f0VMR";` and use it here and in line 1173 test data.
- [x] `crates/claude-agent/src/error.rs:32` — missing documentation for a struct field.
- [x] `crates/claude-agent/src/error.rs:33` — missing documentation for a struct field.
- [x] `crates/claude-agent/src/error.rs:34` — missing documentation for a struct field.
- [x] `crates/claude-agent/src/error.rs:206` — missing documentation for a variant.
- [x] `crates/claude-agent/src/error.rs:209` — missing documentation for a variant.
- [x] `crates/claude-agent/src/error.rs:212` — missing documentation for a variant.
- [x] `crates/claude-agent/src/error.rs:215` — missing documentation for a variant.
- [x] `crates/claude-agent/src/error.rs:218` — missing documentation for a variant.
- [x] `crates/claude-agent/src/error.rs:221` — missing documentation for a variant.
- [x] `crates/claude-agent/src/error.rs:224` — missing documentation for a variant.
- [x] `crates/claude-agent/src/error.rs:227` — missing documentation for a variant.
- [x] `crates/claude-agent/src/error.rs:230` — missing documentation for a variant.
- [x] `crates/claude-agent/src/error.rs:233` — missing documentation for a variant.
- [x] `crates/claude-agent/src/error.rs:236` — missing documentation for a variant.
- [x] `crates/claude-agent/src/error.rs:239` — missing documentation for a variant.
- [x] `crates/claude-agent/src/error.rs:242` — missing documentation for a variant.
- [x] `crates/claude-agent/src/error.rs:245` — missing documentation for a variant.
- [x] `crates/claude-agent/src/mime_type_validator.rs:363` — The case-sensitive set lookup `!allowed_types.contains(mime_type)` will reject uppercase MIME types even though MIME types are case-insensitive per RFC 2045. The allowed_types HashSet contains lowercase entries (line 195: 'image/png'), so 'Image/PNG' is silently rejected as unsupported. No test covers uppercase MIME types. Normalize before lookup or use case-insensitive comparison. Add one test passing 'IMAGE/PNG' and verify it is accepted.
- [x] `crates/claude-agent/src/mime_type_validator.rs:422` — validate_audio_mime_type (lines 422–435) is a near-verbatim copy of validate_image_mime_type (lines 393–406), differing only in the category string ("audio" vs "image"), the policy field (&self.policy.allowed_audio_types vs &self.policy.allowed_image_types), the allowed_categories slice (&["audio"] vs &["image"]), and the format validation function (Self::validate_audio_format_matches_mime vs Self::validate_image_format_matches_mime). These are one function with parameters. Refactor into a single parameterized helper method that accepts the category name, policy field, allowed_categories, and format-validation function as arguments.
- [x] `crates/claude-agent/src/mime_type_validator.rs:501` — The case-sensitive equality comparison `*mime == mime_type` in the find() closure will fail to match uppercase MIME types even though MIME types are case-insensitive per RFC 2045. The mime_formats table (line 14–28) contains lowercase keys, so 'AUDIO/WAV' will not match 'audio/wav', and format validation is skipped. No test covers uppercase MIME types. Use case-insensitive comparison: `mime.eq_ignore_ascii_case(mime_type)` in the find closure. Add one test passing 'AUDIO/WAV' and verify format validation runs correctly.
- [x] `crates/claude-agent/src/mime_type_validator.rs:534` — validate_audio_format_matches_mime (lines 534–546) is a near-verbatim copy of validate_image_format_matches_mime (lines 520–532), differing only in the detect function passed (Self::detect_audio_format vs Self::detect_image_format) and the format table (AUDIO_MIME_FORMATS vs IMAGE_MIME_FORMATS). These are one function with parameters, not two methods. Extract a shared helper method that accepts the detector function and format table as parameters, eliminating the copy.
- [x] `crates/claude-agent/src/mime_type_validator.rs:549` — Magic number 2 hardcoded as minimum image data length without named constant. This represents the minimum bytes needed for JPEG format detection. Define `const IMAGE_HEADER_MIN_SIZE: usize = 2;` and replace `data.len() < 2` with `data.len() < IMAGE_HEADER_MIN_SIZE`.
- [x] `crates/claude-agent/src/mime_type_validator.rs:579` — Multiple magic numbers hardcoded for WebP format detection: 12 for RIFF header size, 4 and 8 for byte offsets. These should be named constants. Define constants: `const WEBP_RIFF_HEADER_SIZE: usize = 12; const RIFF_SIGNATURE_SIZE: usize = 4; const RIFF_FORMAT_OFFSET: usize = 8;` and use them here.
- [x] `crates/claude-agent/src/mime_type_validator.rs:596` — Multiple magic numbers hardcoded for WAV/RIFF format detection: 12 for RIFF size, 4 and 8 for byte offsets. These should be named constants. Reuse constants from WebP detection or define `const WAV_RIFF_HEADER_SIZE: usize = 12;` and use consistent offset constants.
- [x] `crates/claude-agent/src/mime_type_validator.rs:605` — Magic number 7 hardcoded as minimum AAC ADTS data length, and 0xF0 used as bit mask without explanation. These should be named constants. Define `const AAC_HEADER_MIN_SIZE: usize = 7; const AAC_SYNC_MASK: u8 = 0xF0; const AAC_SYNC_PATTERN: u8 = 0xF0;` and use them.
- [x] `crates/claude-agent/src/path_validator.rs:46` — missing documentation for a variant.
- [x] `crates/claude-agent/src/path_validator.rs:49` — missing documentation for a variant.
- [x] `crates/claude-agent/src/path_validator.rs:52` — missing documentation for a variant.
- [x] `crates/claude-agent/src/path_validator.rs:55` — missing documentation for a variant.
- [x] `crates/claude-agent/src/path_validator.rs:58` — missing documentation for a variant.
- [x] `crates/claude-agent/src/path_validator.rs:61` — missing documentation for a variant.
- [x] `crates/claude-agent/src/path_validator.rs:64` — missing documentation for a variant.
- [x] `crates/claude-agent/src/path_validator.rs:67` — missing documentation for a variant.
- [x] `crates/claude-agent/src/path_validator.rs:70` — missing documentation for a variant.
- [x] `crates/claude-agent/src/path_validator.rs:73` — missing documentation for a variant.
- [x] `crates/claude-agent/src/path_validator.rs:76` — missing documentation for a variant.
- [x] `crates/claude-agent/src/path_validator.rs:76` — missing documentation for a struct field.
- [x] `crates/claude-agent/src/path_validator.rs:122` — Method accepts concrete `Vec<PathBuf>` instead of a generic type. The Rust API design principle is to accept trait bounds like `impl IntoIterator<Item=PathBuf>` to allow callers to pass any iterable, not just `Vec`. This provides flexibility without runtime cost. Change signature to accept a generic: `pub fn with_allowed_roots<I: IntoIterator<Item=PathBuf>>(roots: I) -> Self` and collect inside: `Self { allowed_roots: Self::canonicalize_roots(roots.into_iter().collect()), ..Self::new() }`.
- [x] `crates/claude-agent/src/path_validator.rs:130` — Method accepts concrete `Vec<PathBuf>` instead of a generic type, violating the principle of accepting generics not concrete types. Change to: `pub fn with_blocked_paths<I: IntoIterator<Item=PathBuf>>(blocked: I) -> Self` and collect inside.
- [x] `crates/claude-agent/src/path_validator.rs:138` — Method accepts concrete `Vec<PathBuf>` for both parameters instead of generic types, violating the principle of accepting generics not concrete types. Change to: `pub fn with_allowed_and_blocked<A: IntoIterator<Item=PathBuf>, B: IntoIterator<Item=PathBuf>>(allowed: A, blocked: B) -> Self` and collect both inside.
- [x] `crates/claude-agent/src/path_validator.rs:451` — validate_unix_permissions implements Unix file permission validation logic that is 0.90 similar to existing check_binary_permissions in apps/swissarmyhammer-cli/src/commands/doctor/checks.rs; the high similarity suggests the author should verify whether existing code can be reused or extended instead of reimplementing. Investigate apps/swissarmyhammer-cli/src/commands/doctor/checks.rs:82 check_binary_permissions to determine whether it can be reused or extended to provide validate_unix_permissions functionality instead of reimplementing.
- [x] `crates/claude-agent/src/session.rs:200` — missing documentation for a struct field.
- [x] `crates/claude-agent/src/session.rs:206` — missing documentation for a struct field.
- [x] `crates/claude-agent/src/session.rs:207` — missing documentation for a struct field.
- [x] `crates/claude-agent/src/session.rs:208` — missing documentation for a struct field.
- [x] `crates/claude-agent/src/session.rs:209` — missing documentation for a struct field.
- [x] `crates/claude-agent/src/session.rs:210` — missing documentation for a struct field.
- [x] `crates/claude-agent/src/session.rs:211` — missing documentation for a struct field.
- [x] `crates/claude-agent/src/session.rs:288` — Method accepts concrete `PathBuf` for a path parameter. The Rust API design principle is to accept generic trait bounds like `impl AsRef<Path>` to allow callers to pass `&str`, `&Path`, `String`, or `PathBuf`. This follows the standard library pattern used by `File::open` and `env::set_current_dir`. Change signature to: `pub fn new(id: SessionId, cwd: impl AsRef<Path>) -> Self` and convert inside: `let cwd = cwd.as_ref().to_path_buf();`.
- [x] `crates/claude-agent/src/session.rs:329` — Method accepts concrete `Vec<AvailableCommand>` instead of a generic type. The Rust API design principle is to accept trait bounds like `impl IntoIterator<Item=AvailableCommand>` to allow callers to pass any iterable. Change signature to accept a generic: `pub fn update_available_commands<I: IntoIterator<Item=agent_client_protocol::schema::AvailableCommand>>(&mut self, commands: I)` and collect inside: `self.available_commands = commands.into_iter().collect();`.
- [x] `crates/claude-agent/src/session.rs:382` — Method uses `get_` prefix on a simple getter. The Rust API design principle is to use the field name directly without the `get_` prefix: use `turn_request_count()` instead of `get_turn_request_count()`. Rename to: `pub fn turn_request_count(&self) -> u64`.
- [x] `crates/claude-agent/src/session.rs:387` — Method uses `get_` prefix on a simple getter. The Rust API design principle is to use the field name directly without the `get_` prefix: use `turn_token_count()` instead of `get_turn_token_count()`. Rename to: `pub fn turn_token_count(&self) -> u64`.
- [x] `crates/claude-agent/src/session.rs:395` — missing documentation for a struct field.
- [x] `crates/claude-agent/src/session.rs:396` — missing documentation for a struct field.
- [x] `crates/claude-agent/src/session.rs:411` — Method accepts concrete `String` instead of accepting generic types. The Rust API design principle is to use `impl Into<String>` to allow callers to pass `&str`, `String`, or other types that convert to `String`. Change signature to: `pub fn new(role: MessageRole, content: impl Into<String>) -> Self` and convert inside: `let content = content.into();`.
- [x] `crates/claude-agent/src/session.rs:445` — missing documentation for a variant.
- [x] `crates/claude-agent/src/session.rs:446` — missing documentation for a variant.
- [x] `crates/claude-agent/src/session.rs:447` — missing documentation for a variant.
- [x] `crates/claude-agent/src/session.rs:674` — Method accepts concrete `Vec<AvailableCommand>` instead of a generic type. The Rust API design principle is to accept trait bounds like `impl IntoIterator<Item=AvailableCommand>` to allow callers to pass any iterable. Change signature to accept a generic: `pub fn update_available_commands<I: IntoIterator<Item=agent_client_protocol::schema::AvailableCommand>>(&self, session_id: &SessionId, commands: I)` and collect inside when calling the session method.

### Dropped by the review skill's pre-existing-test exemption

The engine confirmed 123 findings. The 12 below ask to restyle test code that already existed, so the skill drops them and they are not requirements. `git diff HEAD~1..HEAD` shows this commit changed only one line pair in `coverage_tests.rs` (the `to_ulid_string` rename) and touched nothing below line 1035 of `content_security_validator.rs` or inside the `session.rs` test module.

- `crates/claude-agent/tests/integration/coverage_tests.rs:1` — split the over-cap test file into smaller modules.
- `crates/claude-agent/tests/integration/coverage_tests.rs:313`, `:328`, `:336`, `:540`, `:563`, `:724`, `:1429`, `:1430` — name the hardcoded test timeouts and buffer sizes.
- `crates/claude-agent/src/content_security_validator.rs:1157` — derive the test's block count from `MODERATE_MAX_CONTENT_ARRAY_LENGTH`.
- `crates/claude-agent/src/session.rs:1213`, `:1223` — name the test cleanup interval and expiration wait.
## Review Findings (2026-08-08 14:06)

> Scope: `review sha HEAD~1..HEAD` — the checkpoint commit `5f8a07a8e` only, not the accumulated task diff.
> Engine counts: findings 39, confirmed 39, refuted 24, attempted 44, failed 0, skipped 1.

> WARNING — 1 file was not reviewed, because the rendered prompt goes over the agent prompt cap:
> - `crates/claude-agent/tests/integration/coverage_tests.rs` — 365790 rendered bytes, over the 262144-byte per-file cap; not read by: duplication (split the file).
>
> This is a coverage gap, not a clean pass. The same file was over the cap in the 2026-08-08 12:56 round at 365706 bytes. It grew by 84 bytes and is still over.

- [x] `crates/claude-agent/src/acp_error_conversion.rs:26` — `ContentProcessingError` is a public error type that should implement `Clone` for consistency and to allow downstream code to clone errors when needed. The related `Base64ProcessorError` in the same codebase derives `Clone` (line 58 of base64_processor.rs), establishing the pattern. Add `Clone` to the derive list: `#[derive(Debug, Error, Clone)]` at line 26.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:210` — `ErrorContext` is a public data structure that should implement `PartialEq` and `Eq` for consistency with standard traits. Downstream crates cannot implement these traits themselves due to the orphan rule, so they must be provided here. Add `PartialEq` and `Eq` to the derive list: `#[derive(Debug, Clone, PartialEq, Eq)]` at line 210.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:269` — Five converter functions (convert_*_error_to_acp) are near-verbatim copies that differ only in the error type parameter. Each wraps add_error_context with identical logic — this is one function with a generic argument. Extract a single generic function: `pub fn convert_error_to_acp<E: ToJsonRpcError>(error: E, context: Option<ErrorContext>) -> JsonRpcError { add_error_context(error, context) }`. Delete the five wrapper functions and update call sites.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:277` — convert_base64_error_to_acp duplicates the structure of convert_content_security_error_to_acp and three others—identical wrapper logic, only the error type varies. Consolidate into one generic function; see line 269 finding.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:285` — convert_mime_type_error_to_acp is a near-verbatim copy of the other four converter functions, differing only in the error type parameter. Consolidate into one generic function; see line 269 finding.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:293` — convert_content_block_error_to_acp repeats the same wrapper pattern as the other four functions, each a pass-through to add_error_context. Consolidate into one generic function; see line 269 finding.
- [x] `crates/claude-agent/src/acp_error_conversion.rs:301` — convert_content_processing_error_to_acp is the fifth near-verbatim copy of the converter pattern, all delegating identically to add_error_context. Consolidate into one generic function; see line 269 finding.
- [x] `crates/claude-agent/src/base64_processor.rs:293` — `max_size: usize` and `max_memory_usage: usize` are parameters with different semantic meanings but identical types. Using newtypes prevents accidental parameter swapping and documents intent at compile time. Define newtypes for each parameter (e.g., `struct MaxBase64Size(usize)` and `struct MaxDecodedSize(usize)`), or use a single config struct parameter instead of multiple primitive parameters.
- [x] `crates/claude-agent/src/base64_processor.rs:295` — Adjacent bool parameters in `new_with_config` reduce readability and type safety. `enable_capability_validation: bool, enable_security_validation: bool` should use an enum or config struct so callers cannot accidentally swap them. Replace the two adjacent bool parameters with a configuration struct or enum. For example, create a `struct ValidationConfig { capability: bool, security: bool }` and pass a single parameter of that type, or use `ContentValidationConfig` if it's already defined elsewhere in the codebase.
- [x] `crates/claude-agent/src/base64_processor.rs:299` — SizeValidator creation duplicated here in new_with_config; same as line 275 and two others. Extract into helper; see line 275 finding.
- [x] `crates/claude-agent/src/base64_processor.rs:325` — SizeValidator creation duplicated here in with_enhanced_security; same block as the other three constructors. Extract into helper; see line 275 finding.
- [x] `crates/claude-agent/src/base64_processor.rs:351` — SizeValidator creation duplicated here in with_enhanced_security_config; fourth verbatim copy of the same pattern. Extract into helper; see line 275 finding.
- [x] `crates/claude-agent/src/base64_processor.rs:449` — decode_image_data and decode_audio_data are near-verbatim copies that differ only in the capability checked ('image' vs 'audio'), the content_type parameter passed to validate_enhanced_security, and which MIME validator method is called (validate_image_mime_type vs validate_audio_mime_type). These should be consolidated into a single generic helper. Extract a shared helper method that takes the capability, content_type, and a closure/function pointer to the appropriate MIME validator. Replace both decode_image_data and decode_audio_data with calls to this helper.
- [x] `crates/claude-agent/src/base64_processor.rs:490` — decode_audio_data is a near-verbatim copy of decode_image_data, differing only in the capability string ('audio' vs 'image') and the MIME type validator method called. Extract shared helper; see line 449 finding.
- [x] `crates/claude-agent/src/content_block_processor.rs:56` — EnhancedSecurityConfig is a public struct with Clone-able fields (ContentValidationConfig is Clone, ContentSecurityValidator is Clone as evidenced by line 38's `.clone()` call) but only derives Debug, not Clone. Public types must implement all applicable traits to allow downstream crates to add them via the orphan rule. Change line 55 from `#[derive(Debug)]` to `#[derive(Debug, Clone)]`.
- [x] `crates/claude-agent/src/content_block_processor.rs:266` — ProcessedContent is a public struct with all Clone-able fields (ProcessedContentType: Clone, String: Clone, Option<Vec<u8>>: Clone, HashMap: Clone, usize: Clone) but only derives Debug, not Clone. Public types must implement all applicable traits. Change line 265 from `#[derive(Debug)]` to `#[derive(Debug, Clone)]`.
- [x] `crates/claude-agent/src/content_security_validator.rs:369` — SecurityPolicy::strict() and SecurityPolicy::moderate() (line 406) and SecurityPolicy::permissive() (line 433) are near-identical blocks that differ only in literal values: different security level, size limits, scheme allowlists, boolean flags, and pattern lists. These three builders should be one function parameterized by their differing values. Extract a shared builder function that accepts the differing parameters (level, max sizes, allowed schemes, enabled heuristics, blocked patterns, rate limits) and returns the configured SecurityPolicy. Call it three times with the preset values for strict, moderate, and permissive configurations, reducing ~90 lines of duplication to ~30.
- [x] `crates/claude-agent/src/content_security_validator.rs:651` — Lines 651–652 (ContentBlock::Resource arm) and lines 657–660 (default catch-all arm) both execute the identical statement: `total_estimated_size += RESOURCE_CONTENT_SIZE_ESTIMATE;`. Two match arms doing the exact same thing should be consolidated into one pattern. Consolidate the two match arms using the or pattern: `ContentBlock::Resource(_) | _ => { total_estimated_size += RESOURCE_CONTENT_SIZE_ESTIMATE; }`. This eliminates the redundant code while preserving intent (Resource types and unknown types both charge the conservative estimate).
- [x] `crates/claude-agent/src/session.rs:305` — Panic on bad input (non-absolute path). Per error-handling rule: 'Panics are for bugs only — internal invariant violations. Never panic on expected failure modes (bad input, missing files, network errors).' A non-absolute path is an expected failure mode, not an internal bug. Change Session::new signature to return Result<Self, SessionError> and propagate the validation error, or rely solely on SessionManager::create_session's validation (line 526) which already validates paths and converts to AgentError. If Session::new must remain infallible, remove the panic and rely on the caller's validation at a higher level.

### Dropped by the review skill's pre-existing-test exemption

The engine confirmed 39 findings. The 20 below ask to restyle test code that already existed, so the skill drops them and they are not requirements.

Evidence that each site pre-dates this commit, from `git diff HEAD~1..HEAD`:

- `path_validator.rs` — the test module opens at line 596; the commit's only hunks are at lines 42, 118, 155 and 447, all above it. Nothing in the test module changed.
- `session.rs` — the test module opens at line 792; every hunk in the commit is above it.
- `coverage_tests.rs` — one hunk only, at line 1570 (the turn-counter getter rename).
- `content_security_integration_tests.rs` — the only change nests the pre-existing literals inside `ContentValidationConfig`. The literals themselves are unchanged.

Dropped items:

- `crates/claude-agent/src/content_security_integration_tests.rs:42` — extract the repeated capability name strings as constants.
- `crates/claude-agent/src/content_security_integration_tests.rs:66`, `:79`, `:95`, `:108`, `:212`, `:274`, `:443`, `:451` — name the hardcoded test size limits, array sizes, iteration counts and performance thresholds.
- `crates/claude-agent/src/path_validator.rs:1073` — name the test permission mode `0o000`.
- `crates/claude-agent/src/path_validator.rs:1303`, `:1309`, `:1316` — name the test permission mode `0o644`.
- `crates/claude-agent/src/session.rs:1272` — name the test cleanup-task startup wait.
- `crates/claude-agent/tests/integration/coverage_tests.rs:1` — split the over-cap test file into smaller modules.
- `crates/claude-agent/tests/integration/coverage_tests.rs:336`, `:540`, `:563`, `:724`, `:1430` — name the hardcoded test timeouts and buffer sizes.

### Repeat check against the two earlier rounds

No open finding repeats. None of the 19 file:line sites above appears in the `## Review Findings (2026-08-04 15:45)` section or the `## Review Findings (2026-08-08 12:56)` section.

Six DROPPED findings repeat, all from the 2026-08-08 12:56 dropped list, and from that list only: `coverage_tests.rs:1`, `:336`, `:540`, `:563`, `:724`, `:1430`. They recur because the written exemption drops them, so no round ever made them requirements and no round ever fixed them. They are on their second round, not their third. The 2026-08-04 section names `coverage_tests.rs:161`, `:167`, `:227` and `:2399` only, which are the resolved MCP-acronym conflict items, and none of those returned.

The resolved conflict did not return. The engine raised no lowercase-Display finding this round.
