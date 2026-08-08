---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzgaay20tec27dz2aq18c6v7
  text: |-
    Research done. The card's cited line numbers came from the `../swissarmyhammer-main` checkout and no longer match this branch, but the defect is real. Corrected root cause:

    - The STREAMING path is already fixed (commit a067b678c, 2026-06-19). `agent_prompt_handling::build_streaming_response` already does `meta_map.insert("cache_usage", usage.to_meta_json())`.
    - The review fleet never runs the streaming path. `review/drive.rs::run_pipeline_in_connection` sends `InitializeRequest::new(1.into()).client_capabilities(ClientCapabilities::new().fs(...))` — no `.meta(...)`, so `client_capabilities.meta` is `None`.
    - `agent_prompt_handling::should_stream` reads `client_capabilities.meta["streaming"]` and `unwrap_or(false)`. So every pool turn takes `handle_non_streaming_prompt`.
    - `process_non_streaming_chunks` never reads `chunk.cache_usage`, and `handle_non_streaming_prompt`'s final meta carries `processed`, `streaming`, `claude_response`, `session_messages` — no `cache_usage`. The value dies there.
    - `claude.rs::send_final_chunk` DOES put `result.cache_usage` on the last `MessageChunk` for both paths, so the data reaches the non-streaming loop and is dropped.

    The pool side (`validators/pool.rs`, `review/fleet/prime.rs::classify_reuse`) is complete and needs no change.

    Fix: the non-streaming path must keep the last non-empty `chunk.cache_usage` and insert it into the response `_meta` under the same `cache_usage` key, sharing one helper with the streaming path so the two cannot drift.

    Test plan: a production-path test with a scripted `claude` executable on PATH (the prevailing pattern already used by `claude.rs::spawn_process_and_consume_init_skips_trigger_when_configured`, with `PathGuard` + `#[serial_test::serial(path_env)]`), driving a real `ClaudeAgent` through initialize -> new_session -> prompt and asserting `_meta.cache_usage.cache_read_input_tokens`.
  timestamp: 2026-08-08T09:12:48.448469+00:00
- actor: claude-code
  id: 01kzgcan3r5s64kcwvqgfsj8tc
  text: |-
    Implementation landed.

    What changed:
    - `protocol_translator.rs` — new `CacheUsage::META_KEY` ("cache_usage"). One name for the wire key, shared by every writer and reader so the two sides cannot drift.
    - `agent_prompt_handling.rs` — two shared helpers: `fold_cache_usage` (last non-empty `usage` on the turn wins — the card's requirement 2) and `attach_cache_usage` (writes `META_KEY` into a response `_meta`, writes nothing when the turn reported no usage).
    - Streaming path now folds and attaches through those helpers instead of its own inline copies. Behaviour unchanged.
    - Non-streaming path — the real gap. `process_non_streaming_chunks` now folds `chunk.cache_usage` and returns it on `NonStreamingChunkOutcome::Finished`; `handle_non_streaming_prompt` attaches it to the `EndTurn` meta.
    - Same function, two sibling terminal responses that also had the usage in hand and dropped it: the refusal response (`ClaudeAgent::create_refusal_response` gained a `cache_usage` parameter) and the cancelled-after-API-response reply. Both now attach. Leaving them out would put the same "every fork ran cold" lie back into the fleet log for refused and late-cancelled tasks.
    - `lib.rs` and `validators/pool.rs` read through `META_KEY` instead of a literal.

    Tests (TDD, both watched RED first):
    - `a_warm_non_streaming_turn_reports_cache_usage_on_the_response_meta` — production path. A scripted `claude` executable on PATH answers the init handshake and one turn, ending in a `result` line whose `usage` reports `cache_read_input_tokens: 68324`. Drives a real `ClaudeAgent` through initialize -> new_session -> prompt with no `streaming` client capability (what the fleet sends), asserts `_meta.streaming == false` and `_meta.cache_usage.cache_read_input_tokens == 68324`. RED before the fix: `got {"claude_response": "ok", "processed": true, "session_messages": 2, "streaming": false}` — no cache_usage.
    - `a_refused_turn_still_reports_its_cache_usage` — same harness, refusal reply. RED before the fix: `got {"refusal_detected":true,"session_id":"..."}`.
    - `PathGuard` moved out of `claude.rs`'s test module into `crate::test_support` so both files share one copy. Its serial key changed from `path_env` to the default `#[serial]` group — the group `StateDirGuard` callers use — so a PATH mutator and an XDG_STATE_HOME mutator can never interleave.
    - The existing streaming test `test_process_stream_chunks_carries_cache_usage_to_meta` still passes, and `test_non_streaming_tool_result_emits_tool_call_update` now also asserts a usage-free stream reports no cache usage.

    Note for whoever runs the acceptance check: the fleet log line will read `reuse="warm prompt cache" cache_read_input_tokens=Some(N)` only for a process running this build. The prime stage is untouched, as the card requires.

    Two things found and NOT acted on:
    - The stub had to answer the init turn with its own `result` line. Without it `consume_remaining_init_response` burns its full wait before `new_session` returns (35s in the first draft, 0.5s after). Worth knowing for any future scripted-CLI test.
    - `crates/swissarmyhammer-mcp-proxy/.kanban` is a stray board directory, committed back in 375d20b16. Pre-existing and out of scope here.
  timestamp: 2026-08-08T09:47:36.440354+00:00
- actor: claude-code
  id: 01kzgcaz2p7m1w12b8cbrgfnax
  text: |-
    ### implement — changed
    - evidence: 7 files — crates/claude-agent/src/protocol_translator.rs, crates/claude-agent/src/agent_prompt_handling.rs, crates/claude-agent/src/agent.rs, crates/claude-agent/src/lib.rs, crates/claude-agent/src/claude.rs, crates/claude-agent/src/test_support.rs, crates/swissarmyhammer-validators/src/validators/pool.rs. Two production-path tests watched RED then GREEN against a scripted `claude` on PATH. `cargo nextest run --workspace`: 13791 passed, 0 failed, 0 skipped. `cargo fmt` applied. `cargo clippy --workspace --all-targets -- -D warnings`: clean. Known unrelated flake `collect_response_content_tests::a_lagged_collector_is_an_error_not_a_reply_with_holes` (^7mk3pnp) passed in the full run; untouched.
    - next: /review
  timestamp: 2026-08-08T09:47:46.646063+00:00
- actor: claude-code
  id: 01kzggpgz5r32yysh5n9qqmqb4
  text: |
    ### review — findings
    - scope: `review file` on the seven files this card changed inside merge `76e9e916a`. The card's work is an evil merge: `git diff 5a3f49842 76e9e916a` and `git diff 21cb9d06b 76e9e916a` give the same hunks for these files, so those hunks are the card's own lines.
    - attribution: the card's new lines are agent.rs 1075-1078/1084/1096, agent_prompt_handling.rs 129-131/135/144-174/387/449-452/468/492/539/779-783/803/1377/1381/1389/1402-1407/1429-1431/1464/1995/2002-2005/2401-2607, claude.rs 1557/1905, lib.rs 238, protocol_translator.rs 49-57, test_support.rs 25-64, pool.rs 900-901/908/1219-1222.
    - evidence: 4 open findings on this card — crates/claude-agent/src/agent_prompt_handling.rs:2423, crates/claude-agent/src/agent_prompt_handling.rs:2451, crates/claude-agent/src/test_support.rs:37, crates/claude-agent/src/test_support.rs:45.
    - engine totals per file: protocol_translator.rs 10, lib.rs 12, agent_prompt_handling.rs 31, pool.rs 32, agent.rs 0, claude.rs 17, test_support.rs 2.
    - pre-existing findings were filed as separate `tool-validators` cards: ^ycnvwyp (protocol_translator.rs, 10), ^zs3m73t (lib.rs, 12), ^m0dtzk4 (agent_prompt_handling.rs, 11), ^xypyc7g (pool.rs, 2), ^5gcr0vk (claude.rs, 16). Pre-existing findings whose subject is refactoring test code that already existed were dropped per the review skill's standing exception.
    - the engine raised no finding against `fold_cache_usage`'s last-non-empty rule, and none against attaching cache usage to the refusal reply or the cancelled-after-response reply.
    - next: fix the 4 findings on this card, then re-review.
  timestamp: 2026-08-08T11:03:59.717567+00:00
- actor: claude-code
  id: 01kzggtqm2ewgwwfc47srn5f71
  text: |
    ### finish iteration 1 — findings
    - implement: changed — 7 files. The non-streaming prompt path now attaches `cache_usage` to `PromptResponse._meta` through `attach_cache_usage`, folded by `fold_cache_usage`. The wire key is `CacheUsage::META_KEY`. The prime stage is untouched.
    - test: green — cargo nextest run --workspace, 13852 passed / 0 failed / 0 skipped; fmt, clippy -D warnings clean. That run held these changes and the main merge together.
    - commit: folded into 76e9e916a "Merge branch 'main' into review". This was not intended. A `git add -A` during the merge resolution swept the card's changes into the merge commit, so they are preserved but not separable.
    - review: findings — 4 open. crates/claude-agent/src/agent_prompt_handling.rs:2423, :2451, crates/claude-agent/src/test_support.rs:37, :45
    - engine counts: 104 findings across the 7 files; 4 attributed to this card, 51 filed to other cards, 49 dropped by the written test-code exception

    The card's diagnosis needed one correction, recorded during implementation. The streaming path already attached `cache_usage`, from commit a067b678c. The fleet never takes the streaming path, because `run_pipeline_in_connection` in `review/drive.rs` builds `ClientCapabilities` with no `.meta(...)`, so `should_stream` reads false every time. The real defect was the non-streaming path dropping the final chunk's `cache_usage`.

    One finding is a real defect, not a style point: `write_scripted_claude` at :2451 interpolates `reply` into a single-quoted `/bin/sh` string with no escaping, so a single quote in `reply` closes the quote context and injects shell commands.

    **Scope correction.** The review filed 5 cards for pre-existing findings in untouched code and tagged them `tool-validators`, because the review prompt told it to. That tag was wrong — these are general `claude-agent` code-quality items, unrelated to the tool-validators project, and leaving them tagged would stop the batch ever converging. The tag is removed from all five. The cards remain on the board: ^ycnvwyp, ^zs3m73t, ^m0dtzk4, ^xypyc7g, ^5gcr0vk.
  timestamp: 2026-08-08T11:06:17.602350+00:00
- actor: claude-code
  id: 01kzgh52geg43f34e37eke0gy3
  text: |-
    Research for the 4 open review findings.

    Finding 3 (`PathGuard` duplicates an env-var save/restore pattern in 9+ places). I found the other occurrences before designing the replacement. Every one is the same shape — capture the prior value, set or remove, restore in `Drop`:

    1. `crates/swissarmyhammer-common/src/test_utils.rs` — `IsolatedTestHome` (HOME)
    2. `crates/agent-client-protocol-extras/src/test_support.rs` — `StateDirGuard` (XDG_STATE_HOME)
    3. `crates/agent-client-protocol-extras/src/hook_settings.rs` — `HomeGuard` (HOME)
    4. `crates/claude-agent/src/test_support.rs` — `PathGuard` (PATH) — the finding's line
    5. `crates/swissarmyhammer-validators/src/validators/loader.rs` — `EnvVarGuard`
    6. `apps/swissarmyhammer-cli/src/logging.rs` — `EnvVarGuard`
    7. `apps/swissarmyhammer-cli/src/commands/doctor/checks.rs` — `PathGuard` (PATH)
    8. `apps/shelltool-cli/src/commands/doctor.rs` — `PathEnvGuard` (PATH)
    9. `crates/mirdan/src/test_support.rs` — `MirdanConfigGuard` (MIRDAN_AGENTS_CONFIG)
    10. `crates/swissarmyhammer-tools/tests/integration/mirdan_test_support.rs` — `MirdanConfigGuard`
    11. `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs` — `MirdanConfigGuard`
    12. `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs` — `MirdanConfigGuard`
    13. `crates/mirdan/src/auth.rs` — the same save/restore written inline in three tests (MIRDAN_CREDENTIALS_PATH, MIRDAN_TOKEN)

    Home for the shared guard: the finding suggests `agent_client_protocol_extras::test_support`. ARCHITECTURE.md is authoritative and says otherwise, twice: "The isolation primitives live in one place — `swissarmyhammer-common::test_utils`" and "Any test that touches the filesystem, the working directory, or an environment variable must use the guards in `swissarmyhammer-common::test_utils`". Putting the guard in the ACP extras crate would force `swissarmyhammer-cli`, `shelltool-cli`, `mirdan`, and `swissarmyhammer-tools` to depend on an agent-protocol crate for a test guard — a dependency edge the document forbids. `swissarmyhammer-common` is lower than the extras crate (extras already depends on it), every one of the 13 crates already depends on it, and its `test_utils` is an unconditional `pub mod`, so no Cargo change is needed. The finding's own text allows this: "extract to a shared location and imported" is the requirement; the crate name is its example.

    Finding 2 (shell injection in `write_scripted_claude`) is real. `reply` lands inside a single-quoted `/bin/sh` word AND inside a JSON string, and neither layer is escaped. The fix must cover both layers: build each stdout line with `serde_json` (JSON escaping) and shell-quote the whole `printf` argument with the standard `'` -> `'\''` transform. Every line the script prints goes through the same helper, so the cause leaves the function rather than the one line.

    Findings 1 and 4 are local: name the `30`, and widen `prepend` to `impl AsRef<Path>` (the shared `PathGuard` carries that signature).
  timestamp: 2026-08-08T11:11:56.430662+00:00
- actor: claude-code
  id: 01kzgj8tzjf7r5vc3v7pywxzjy
  text: |-
    All four review findings worked and checked.

    **Finding 1 — the `30` inside `SCRIPTED_TURN_TIMEOUT`.** `SCRIPTED_TURN_TIMEOUT_SECS: u64 = 30` now carries the value; `SCRIPTED_TURN_TIMEOUT` is `Duration::from_secs(SCRIPTED_TURN_TIMEOUT_SECS)`.

    **Finding 2 — shell injection in `write_scripted_claude`.** A real defect, fixed at the cause and proved.

    The reply crossed two quoting layers unescaped: a JSON string, then a single-quoted `/bin/sh` word. `write_scripted_claude` now builds each stdout line with `serde_json` and hands it to `printf_json_line`, which shell-quotes it through `sh_quote` — wrap in single quotes, rewrite each embedded `'` as `'\''`. Every operand the script carries goes through that one helper, so no value it interpolates can end a quoting context. The `{{`/`}}` format escaping is gone with it, and the two `result` lines now share `scripted_result_line`.

    RED before GREEN, watched:
    - Test `a_reply_with_shell_metacharacters_is_data_not_a_command` drives the real agent with `reply = "'; touch <canary> ; '"`, where the canary is a file in its own temp dir. It asserts the canary does not exist and that `_meta.claude_response` equals the reply verbatim.
    - RED: `the reply's shell metacharacters ran as commands: /var/folders/.../pwned exists`. The injected `touch` actually executed under the old code.
    - GREEN after the fix, with the two existing scripted-turn tests still passing.

    **Finding 3 — `PathGuard` duplicates an env-var save/restore pattern in 9+ places.** Extracted `EnvVarGuard` (`capture` / `set` / `unset` / `prepend_path`) and `PathGuard` (`prepend` / `set` / `capture`) into `swissarmyhammer_common::test_utils`, and routed twelve of the thirteen occurrences through them:

    `swissarmyhammer-common` `IsolatedTestHome`; `agent-client-protocol-extras` `StateDirGuard` and `HomeGuard`; `claude-agent` `PathGuard` (now a re-export); `swissarmyhammer-validators` `loader.rs`; `swissarmyhammer-cli` `logging.rs` and `doctor/checks.rs`; `shelltool-cli` `doctor.rs`; `mirdan` `test_support.rs` and `auth.rs`; `swissarmyhammer-tools` `shell/mod.rs`, `kanban/mod.rs`, and `tests/integration/mirdan_test_support.rs` (the three `MirdanConfigGuard` copies now use mirdan's own guard, via a `test-support` dev-dependency feature); `swissarmyhammer-config` `environment_variables_fixed.rs` and `integrations.rs`.

    Deviation from the finding's suggested location, recorded deliberately. The finding names `agent_client_protocol_extras::test_support`. ARCHITECTURE.md says twice that the isolation primitives live in `swissarmyhammer-common::test_utils` and that every test touching an environment variable must use the guards there. Putting the guard in the ACP extras crate would make `swissarmyhammer-cli`, `shelltool-cli`, `mirdan`, and `swissarmyhammer-tools` depend on an agent-protocol crate for a test guard. `swissarmyhammer-common` is below the extras crate, every affected crate already depends on it, and its `test_utils` is an unconditional `pub mod`. The finding's requirement — "extracted to a shared location and imported, not duplicated" — is met; only its example crate changed.

    Four new tests cover the shared guard, and each was mutation-checked: appending instead of prepending in `prepend_path` failed `env_var_guard_prepend_path_keeps_the_existing_entries_behind`, and restoring an absent variable to `""` instead of removing it failed `env_var_guard_restores_an_absent_variable_to_absent`.

    **Finding 4 — `prepend` takes `&Path`.** The shared `PathGuard::prepend` takes `impl AsRef<Path>`. `MirdanConfigGuard::set` gained the same widening for the same reason.

    Corrected a latent bug while routing: `mirdan/src/auth.rs`, `registry_url_default` and `credentials_path_default`, restored their variable only when it had held a value. A variable the test removed and that had been unset stayed removed with no restore at all; more importantly the `if let Some` shape silently skipped the unset branch. `EnvVarGuard` handles both branches uniformly.

    Left alone, with a card: `crates/swissarmyhammer-config/tests/integration/environment_variables.rs`. Its `IsolatedEnvTest` is not the single-variable pattern — it sweeps every `SAH_`/`SWISSARMYHAMMER_` variable and then restores the pre-test set, and the tracked-variable restore deliberately skips those prefixes. Guards drop after `Drop::drop` returns, so a naive conversion would write a swept `SAH_` variable back and break the isolation. Filed as ^f7vd3ha with the ordering contract written down.

    Two clippy fixes fell out of the widened signature: `MirdanConfigGuard::set(&write_claude_agents_config(..))` in `per_client_tool_composition.rs` and `serve_time_bash_deny.rs` no longer needs the borrow.
  timestamp: 2026-08-08T11:31:28.370360+00:00
- actor: claude-code
  id: 01kzgj9c4d8g4sg08ehmjnc2bk
  text: |-
    ### implement — changed
    - evidence: 20 files — apps/shelltool-cli/src/commands/doctor.rs, apps/swissarmyhammer-cli/src/commands/doctor/checks.rs, apps/swissarmyhammer-cli/src/logging.rs, crates/agent-client-protocol-extras/src/hook_settings.rs, crates/agent-client-protocol-extras/src/test_support.rs, crates/claude-agent/src/agent_prompt_handling.rs, crates/claude-agent/src/test_support.rs, crates/mirdan/src/agents.rs, crates/mirdan/src/auth.rs, crates/mirdan/src/test_support.rs, crates/swissarmyhammer-common/src/test_utils.rs, crates/swissarmyhammer-config/tests/integration/environment_variables_fixed.rs, crates/swissarmyhammer-config/tests/integration/integrations.rs, crates/swissarmyhammer-tools/Cargo.toml, crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs, crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs, crates/swissarmyhammer-tools/tests/integration/mirdan_test_support.rs, crates/swissarmyhammer-tools/tests/integration/per_client_tool_composition.rs, crates/swissarmyhammer-tools/tests/integration/serve_time_bash_deny.rs, crates/swissarmyhammer-validators/src/validators/loader.rs. All 4 findings checked. Shell injection proved by RED then GREEN: `a_reply_with_shell_metacharacters_is_data_not_a_command` failed with "the reply's shell metacharacters ran as commands: /var/folders/.../pwned exists" before the fix and passes after. `cargo nextest run --workspace`: 13857 passed, 0 failed, 0 skipped (was 13852; +5 new tests). `cargo test --doc -p swissarmyhammer-common`: 24 passed. `cargo fmt` applied. `cargo clippy --workspace --all-targets -- -D warnings`: exit 0.
    - next: /review
  timestamp: 2026-08-08T11:31:45.933413+00:00
- actor: claude-code
  id: 01kzgm2rs5q8kjcszp2kja0jdx
  text: |
    ### review — findings
    - scope: `review sha HEAD~2..HEAD` — d99fb9021 (shell quoting) and e678615c4 (EnvVarGuard/PathGuard, 12 of 13 sites). The range touches 20 files because the guard routing reached every crate that carried a copy.
    - engine counts: 36 findings, 36 confirmed, 29 refuted, 44 attempted, 0 failed, 1 file skipped for the prompt cap.
    - recorded: 31 open findings. Dropped 5 under the standing test-code exception, each verified pre-existing and outside every hunk in this range: agents.rs:436, auth.rs:529, loader.rs:800, mirdan/test_support.rs:116, mirdan/test_support.rs:158.
    - prior 4 findings confirmed genuinely resolved. SCRIPTED_TURN_TIMEOUT_SECS at agent_prompt_handling.rs:2422 used at :2428. sh_quote at :2438 is the POSIX `'` -> `'\''` transform, and every script operand reaches it through printf_json_line, so serde_json escapes newlines and control bytes before the shell layer sees them; printf `%s` does not reinterpret escapes in its argument. PathGuard/EnvVarGuard live once in swissarmyhammer-common::test_utils. PathGuard::prepend takes `impl AsRef<Path>` at test_utils.rs:358.
    - the engine raised zero findings against the two files these commits authored, `crates/claude-agent/src/agent_prompt_handling.rs` and `crates/swissarmyhammer-common/src/test_utils.rs`. It raised nothing about the 13th unconverted site (^f7vd3ha).
    - every open finding is on a pre-existing line in a file the guard routing touched, except `crates/claude-agent/src/test_support.rs:36`, which is the new `pub(crate) use` re-export this range wrote.
    - next: work the 31 findings, then re-review.
  timestamp: 2026-08-08T12:03:06.661291+00:00
- actor: claude-code
  id: 01kzgmar2emgmzrzr66wphz67d
  text: |-
    Worked the one finding of the 2026-08-08 06:36 section that belongs to this card, and moved the other 30 off it.

    **The finding.** `crates/claude-agent/src/test_support.rs:36` — the new `pub(crate) use swissarmyhammer_common::test_utils::PathGuard;` re-export carried its ten explanatory lines as `//`, not `///`, so the text documented nothing.

    Swept the whole file for the same cause, as a finding requires. The file holds two `pub(crate) use` re-exports, and both had the defect:

    - the `PathGuard` block the finding names (ten lines, including the bare `//` separator line), and
    - the `StateDirGuard` block above `command_args`, three lines, same mistake, not named by the engine.

    Both are now `///`. `command_args` already had a correct `///` block. The `//!` header stays `//!` — that is the module doc and is correct.

    The [`StateDirGuard`] intra-doc link inside the `PathGuard` block now resolves for real instead of sitting in an ignored comment. The module is `#[cfg(test)]`, so rustdoc never renders it; the value is that the doc-comment validator and any reader see the text attached to the item.

    **The other 30 findings moved to ^k24scye** (untagged — general code quality, not `tool-validators`, so it cannot hold the tool-validators batch open). Every one of them is on a pre-existing line in `mirdan/src/agents.rs`, `mirdan/src/auth.rs`, `swissarmyhammer-tools` `kanban/mod.rs` and `shell/mod.rs`, or `swissarmyhammer-validators/src/validators/loader.rs`. Commit `e678615c4` routed env-guard call sites through those files, which put whole files in review scope; it did not write those lines. The reviewer confirmed the engine returned zero findings against the two files these commits actually authored — `agent_prompt_handling.rs` and `swissarmyhammer-common/src/test_utils.rs`. The new card carries all 30 word for word, grouped by file, with a note that later commits move the reported line numbers.

    The description's 06:36 section now holds the single `test_support.rs:36` item, checked, plus one line pointing at ^k24scye. The older dated sections are untouched.
  timestamp: 2026-08-08T12:07:28.078731+00:00
- actor: claude-code
  id: 01kzgmb0j92cp4cpepxaf9zndd
  text: |-
    ### implement — changed
    - evidence: 1 source file — crates/claude-agent/src/test_support.rs. Both `pub(crate) use` re-exports (`StateDirGuard`, `PathGuard`) now carry `///` doc comments instead of `//`. `cargo fmt` applied. `cargo clippy --workspace --all-targets -- -D warnings`: exit 0. `cargo nextest run -p claude-agent`: 1090 passed, 0 failed, 0 skipped. The 30 pre-existing findings moved to new card ^k24scye (untagged); ^811xj0q's newest findings section now holds only the checked `test_support.rs:36` item plus the pointer line.
    - next: /review
  timestamp: 2026-08-08T12:07:36.777275+00:00
- actor: claude-code
  id: 01kzgmhceegd4ay2t8qq7my2p5
  text: |
    ### review — clean
    - evidence: `review sha HEAD~2..HEAD` — 0 findings, 0 confirmed, 0 refuted; 9 validators attempted, 0 failed, 0 skipped, no file over the prompt cap.
    - scope: `d2581a87a` (13 comment lines `//` -> `///` on the two `test_support.rs` re-exports) and `a539bbb53` (board data only). One code file changed: `crates/claude-agent/src/test_support.rs`.
    - prior findings verified resolved: `SCRIPTED_TURN_TIMEOUT_SECS` at `agent_prompt_handling.rs:2422`, used at :2428; `sh_quote` at :2438 applies the POSIX `'` -> `'\''` transform; `EnvVarGuard` at `swissarmyhammer-common/src/test_utils.rs:284` and `PathGuard` at :350 hold the extracted guard, claude-agent re-exports it; `PathGuard::prepend` takes `impl AsRef<Path>` at :358; both `test_support.rs` re-exports now carry `///` doc comments.
    - next: none. Task moves to done.
  timestamp: 2026-08-08T12:11:05.550078+00:00
- actor: claude-code
  id: 01kzgmjmjw9hbvtsah72addvfd
  text: |
    ### finish iteration 3 — clean
    - implement: changed — both `pub(crate) use` re-exports in crates/claude-agent/src/test_support.rs now use `///`. The engine named one; the sweep found the same cause on the other.
    - test: green — cargo nextest run -p claude-agent, 1090 passed / 0 failed / 0 skipped; fmt, clippy -D warnings clean. A doc-comment change did not need the full workspace suite; the prior round ran it at 13857 passed.
    - commit: d2581a87a docs(claude-agent): use doc comments for test_support re-exports (^811xj0q); a539bbb53 chore(kanban): record checkpoint and prior review-pass cards
    - review: clean — 9 validators attempted, 0 findings, 0 files over the prompt cap
    - task moved to done by the review gate

    Card closed.

    The substantive result: the review fleet's warm prefix reuse can now engage. The card's own diagnosis was wrong and was corrected during work — the streaming path already attached `cache_usage` from commit a067b678c, and the fleet never takes the streaming path, because `run_pipeline_in_connection` builds `ClientCapabilities` with no `.meta(...)`. The real defect was the non-streaming path holding the final chunk's `cache_usage` and dropping it.

    A real security defect was found and fixed on the way: `write_scripted_claude` crossed a JSON layer and a single-quoted `/bin/sh` layer with no escaping. Proved with a watched RED — a reply of `'; touch <canary> ; '` really executed the `touch` — then GREEN through `printf_json_line` and `sh_quote`.

    **Note on review scope for whoever picks up the next card.** Two rounds on this card surfaced far more findings about pre-existing code than about the change:
    - Round 1 used `review file` on 7 whole files: 104 findings, 4 belonged to the card.
    - Round 2 used the commit range, but the range spanned 20 files because the env-guard extraction routed call sites through them: 31 recorded, 1 belonged to the card.

    Use a commit range, not `review file`, and expect a wide-but-shallow refactor to pull unrelated files into scope. Findings that land on pre-existing lines belong on their own card.

    Cards split off from this one, all untagged so they do not hold the tool-validators batch open: ^ycnvwyp, ^zs3m73t, ^m0dtzk4, ^xypyc7g, ^5gcr0vk, ^k24scye, ^f7vd3ha.
  timestamp: 2026-08-08T12:11:46.652787+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffc880
title: 'review fleet: warm prefix reuse never engages — every fork runs cold'
---
DIAGNOSED 2026-08-07 from ../swissarmyhammer-main/.sah/mcp.7341.log. The forks ARE warm. The telemetry is broken. This is a log-lies defect, not a performance defect.

Evidence:
- The prime turn (15:14:05) reported `cache_read: 0, cache_creation: 68324` — the shared prefix wrote to the Anthropic prompt cache.
- Every one of the nine forked task turns reported `cache_read_input_tokens: Some(68324)` on the translator's final chunk ("Sending final chunk", claude.rs:1177). The full primed prefix came from the warm cache on every fork. The fork mechanism (`claude --resume <prime> --fork-session`) works.
- The fleet reuse line for the same tasks says `reuse="cold (no reuse)" cache_read_input_tokens=None`. The usage never reached `SessionTurn`.

Root cause (confirmed in source):
- The pool reads `cache_usage` from `PromptResponse._meta` (`validators/pool.rs` ~900-916).
- The real claude-agent never puts it there. `handle_streaming_prompt`'s response builder (`claude-agent/src/agent_prompt_handling.rs` ~618-627) inserts `streaming`, `session_id`, `output_tokens` — no `cache_usage`. The value stops at the internal final chunk (`claude.rs:1174`).
- The pool test `test_pool_turn_propagates_cache_usage_from_response` passes because its MOCK attaches the `_meta.cache_usage` key the real agent never sends. Mock drift hid the gap (see the real-path-tests rule).

Fix:
1. In claude-agent, keep the final StreamResult's `cache_usage` and insert `cache_usage: CacheUsage::to_meta_json(...)` into the PromptResponse `_meta`, for streaming and non-streaming prompts. `to_meta_json` exists; the pool already parses that exact shape.
2. A turn with several result messages keeps the last non-empty usage. Any read > 0 classifies warm; more precision is not needed.
3. The verify path reads the same `SessionTurn`; one fix covers both "degraded" log sites.

Tests:
- claude-agent production-path test: a scripted claude process emits a result line with a `usage` object (the shape at `protocol_translator.rs:1464`); assert the returned PromptResponse carries `_meta.cache_usage.cache_read_input_tokens` equal to the emitted value. This is the test the mock hid.
- Keep the existing pool test — the pool side is already proven.
- Acceptance on a real run: the fleet log line reads `reuse="warm prompt cache" cache_read_input_tokens=Some(N)`, with N near the prime's cache_creation count.

Scope correction: the first version of this card said cold forks were the dominant run cost. They are not — the API served the prefix warm on every fork. The cost of this defect is misleading telemetry and the wrong conclusions built on it. Do NOT remove the prime stage. It works.

#tool-validators

## Review Findings (2026-08-08 05:50)

Scope: `review file` on the seven files this card changed inside merge `76e9e916a`. Only findings on lines this card authored are listed here; findings on pre-existing lines were filed as separate cards.

- [x] `crates/claude-agent/src/agent_prompt_handling.rs:2423` — Timeout value `30` (seconds for scripted turn timeout) is a magic number even within a named constant; the literal should be extracted to its own constant for consistency. Define `const SCRIPTED_TURN_TIMEOUT_SECS: u64 = 30;` at module scope and use it: `Duration::from_secs(SCRIPTED_TURN_TIMEOUT_SECS)`, maintaining clarity while making the value easily parameterizable.
- [x] `crates/claude-agent/src/agent_prompt_handling.rs:2451` — The `reply` parameter is directly interpolated into a shell script via Rust format! without escaping. A single quote character in `reply` will terminate the single-quoted string context prematurely, allowing injection of arbitrary shell commands. Escape the `reply` parameter before interpolating it into the shell script. Either (1) validate that `reply` contains only safe characters (alphanumeric, spaces, basic punctuation) and reject any containing single quotes or shell metacharacters, or (2) use proper shell escaping such as replacing each single quote `'` with the sequence `'"'"'` to safely embed it within single-quoted strings.
- [x] `crates/claude-agent/src/test_support.rs:37` — PathGuard reimplements an environment-variable save-and-restore pattern already implemented in 9+ locations across the codebase with 0.86–0.97 semantic similarity. This should be extracted to a shared location and imported, not duplicated. Extract a generic `EnvVarGuard` to `agent_client_protocol_extras::test_support` (the same crate that exports `StateDirGuard` per line 11), parameterized by variable name and value type. Replace this fresh `PathGuard` with a type alias or direct import of the shared guard, or extend the existing generic if one of the nine implementations can be generalized.
- [x] `crates/claude-agent/src/test_support.rs:45` — Function parameter accepts &Path instead of impl AsRef<Path>, reducing flexibility for callers who might have PathBuf or want to pass owned values. Change signature to 'pub(crate) fn prepend(dir: impl AsRef<std::path::Path>) -> Self' and add 'let dir = dir.as_ref();' at the start of the function body to maintain the same implementation logic.

## Review Findings (2026-08-08 06:36)

Scope: `review sha HEAD~2..HEAD` — `d99fb9021` (shell quoting fix) and `e678615c4` (EnvVarGuard/PathGuard extraction, 12 of 13 sites routed). The guard extraction touches 20 files, so the range covers every crate the routing reached.

The four findings from the 2026-08-08 05:50 section verify as genuinely resolved: `SCRIPTED_TURN_TIMEOUT_SECS` exists at `agent_prompt_handling.rs:2422` and is used at :2428; `sh_quote` at :2438 applies the POSIX `'` -> `'\''` transform and every script operand reaches it through `printf_json_line`, so `serde_json` neutralizes newlines and control bytes before the shell layer; `EnvVarGuard`/`PathGuard` live once in `swissarmyhammer-common::test_utils` with claude-agent re-exporting; `PathGuard::prepend` takes `impl AsRef<Path>` at `test_utils.rs:358`. The engine raised no finding against `agent_prompt_handling.rs` or `swissarmyhammer-common/src/test_utils.rs`.

> ⚠️ 1 file(s) not reviewed — the rendered prompt would exceed the agent's prompt cap:
> - `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs` — 364079 rendered bytes, over the 262144-byte per-file cap; not reviewed by: duplication (split the file)

- [x] `crates/claude-agent/src/test_support.rs:36` — Public item `PathGuard` re-export lacks a doc comment. All public items (including `pub(crate)` re-exports) must have doc comments that start with `///` or `//!` to be picked up by documentation generation. Convert the preceding regular comments to doc comments by changing `//` to `///` on lines 26-35, or add a `///` doc comment immediately before line 36.

The other 30 findings of this section moved to card ^k24scye. They sit on pre-existing lines in files that the env-guard routing only passed through, so they are not this card's work.

Five further engine findings were dropped under the review skill's standing exception for refactoring test code that already existed, each confirmed pre-existing (outside every hunk in this range) and inside a `#[cfg(test)] mod tests` or a test-support assertion helper: `crates/mirdan/src/agents.rs:436`, `crates/mirdan/src/auth.rs:529`, `crates/swissarmyhammer-validators/src/validators/loader.rs:800`, `crates/mirdan/src/test_support.rs:116`, `crates/mirdan/src/test_support.rs:158`.
