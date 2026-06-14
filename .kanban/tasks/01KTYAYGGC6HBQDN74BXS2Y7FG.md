---
assignees:
- claude-code
depends_on:
- 01KTY8ZV79NX25KJ1BF4CTKPCR
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffa380
project: local-review
title: 'feat(claude-agent): implement the session-fork ACP extension (claude --resume --fork-session)'
---
## What

Companion to 01KTY8ZV79NX25KJ1BF4CTKPCR (llama-agent session fork): the SAME ACP fork extension, implemented by the claude backend, so the review fleet has ONE code path — prime → confirm → fork → prompt — with no backend branching. The fork contract (extension method / `_meta.forkFrom`, response reporting state attachment, `state_status`, pin) is defined by the llama task; this task makes crates/claude-agent honor it.

Implementation sketch (verify against the actual claude CLI version and claude_process.rs):

- [x] Fork = spawn the child session's claude process with `--resume <parent-session-id> --fork-session` (the CLI's native conversation-fork primitive; `spawn_for_resume` in crates/claude-agent/src/claude_process.rs is the precedent). Map the resulting CLI session to the new ACP session id; clone whatever agent-side session bookkeeping (conversation_manager/session.rs) the resume path already maintains.
- [x] `state_status` for a claude session reports saved=true once the parent's prime turn completed (state lives in the CLI's session files; there is no KV cache to evict) with promptTokens unknown/None — the contract must allow that. `pin` is a no-op that succeeds.
- [x] Fork response reports state attachment truthfully: if `--resume --fork-session` fails (unknown session, CLI too old), return the same distinguishable error the llama side returns so the fleet's fallback path is backend-agnostic.
- [x] Prefix-cache note (no code, verify + document): forks replay the identical first turn, so Anthropic's server-side prompt caching covers the shared prefix automatically. If claude-agent constructs API-level requests anywhere (rather than delegating to the CLI), confirm the prefix lands cache-eligible; do NOT build a custom caching layer.

## Acceptance Criteria

- [x] Through the same pool client code that drives llama forks, a claude-backend session can be primed, forked N times, and each fork prompted independently — identical call sequence, no backend conditionals in the caller.
- [x] Fork of an unknown/expired parent fails with the same error shape as llama-agent's, and the fleet fallback path treats both identically.

## Tests

- [x] claude-agent unit tests with a fake/scripted claude process (the crate's existing process-test seam; <10s, no real CLI): fork spawns with `--resume <parent> --fork-session`; session bookkeeping cloned; unknown-parent error shape matches the contract; state_status/pin contract behavior.
- [x] Contract test shared with the llama implementation if a seam exists (acp-conformance is the natural home): both agents satisfy the same fork-extension conformance checks. → No natural seam; rationale in Implementation Notes.
- [x] `cargo test -p claude-agent` green.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

## Implementation Notes (2026-06-12)

**Real-CLI verification first** (per task instruction): `claude --help` shows `--fork-session`; verified end-to-end against the installed CLI that `claude --print --resume <parent-uuid> --fork-session --session-id <child-uuid>` (a) is an accepted flag combination, (b) clones the parent's conversation (the fork answered with content only the parent's turn contained), and (c) lands the fork on the *specified* child UUID (a subsequent `--resume <child-uuid>` continued it). Also verified: a parent primed under `--no-session-persistence` CANNOT be resumed or forked ("No conversation found with session ID") — which drove a deliberate production change below.

Shipped (all in crates/claude-agent, plus reuse of the committed wire contract in agent-client-protocol-extras::session_fork — no new wire types):

- **`src/claude_process.rs`** — a public `ConversationAttachment` enum (`New` → `--session-id <uuid>`, `Resume` → `--resume <uuid>`, `Fork{parent}` → `--resume <parent-uuid> --fork-session --session-id <child-uuid>`) carried directly on `SpawnConfig.attachment` (default `New`) and consumed by `build_base_command`. The child's CLI uuid stays the deterministic `SessionId::to_uuid_string` mapping, so forks are themselves resumable/forkable with no stored uuid map.
- **`--no-session-persistence` moved from the always-on CLAUDE_CLI_ARGS to ephemeral mode only** (`configure_ephemeral_mode`, where every doc layer already said it belonged: "ephemeral = haiku + no session persistence"). This is REQUIRED for fork: the CLI can only fork/resume a transcript it persisted. Side effect: it also un-breaks the existing `session/resume`/`session/load` machinery in production, which re-spawns `--resume <uuid>` and could never have found a transcript while persistence was globally disabled. Durable sessions now write CLI transcripts under ~/.claude; ephemeral (validator-style) sessions still don't.
- **`src/claude.rs`** — `ClaudeClient::fork_process(parent, config)` beside `resume_process`: spawns the forked child process, no init trigger (same rationale as resume — it would inject a spurious turn), and detects the CLI's immediate-exit failure mode via the shared `early_exit_detail` helper (exit status + stderr). Returns `ForkAttachError` distinguishing `Rejected` (CLI cleanly refused the fork — parent transcript missing / `--fork-session` unsupported) from `Spawn` (environment failure: binary missing, spawn I/O).
- **`src/session_fork.rs` (new)** — claude's side of the shared contract, mirroring llama's handler shape: `fork_session` (resolve parent → never-fork-blind gate → spawn forked CLI process FIRST (the fallible state-attach; no half-created child) → clone the in-memory `Session` under a fresh ULID → wire transcript recorder + persist durable SessionRecord). Error mapping matches llama exactly: clean miss / non-ULID id → `invalid_params` + `data.error=fork_parent_not_found`; parent without a completed turn, ephemeral mode, or the CLI cleanly rejecting the fork → `fork_parent_state_unavailable`; session-store failure OR forked-process spawn/environment failure → retryable `-32603` internal error, never masquerading as a parent-state condition. `session_state_status`: `saved = has_first_exchange(session) && !ephemeral`, counts None, `pinned` false. `pin_session`: no-op success reporting effective state. Response `prefix_tokens=None`, `state_attached=true`.
- **`src/agent_trait_impl.rs`** — `ext_method` routes `session/fork` / `session/state_status` / `session/pin` (matched on the shared extras consts) to small handlers; all `fs/*`, `terminal/*`, and editor wire method names are now named constants shared by dispatch arms and handlers.
- **Prefix-cache note (no code)**: verified claude-agent constructs no Anthropic-API requests anywhere — all generation goes through the spawned claude CLI, which manages server-side prompt caching itself; forks replay the identical prefix so caching applies automatically. Documented in the session_fork module doc.
- **Drive-by bug fix (exposed by the new tests)**: `session_validation::validate_directory_permissions` parked the process in every probed cwd. Now probes traversability with a side-effect-free `access(2)`/`X_OK` check (no CWD mutation at all) and preserves the OS error as the error `source`.
- **Test support**: one canonical `StateDirGuard` (XDG_STATE_HOME isolation) hosted in `agent-client-protocol-extras::test_support` behind a `test-support` feature; claude-agent and llama-agent re-export it.

**Shared acp-conformance fork test: deliberately not added.** A meaningful fork conformance check needs a *primed* parent (a completed prompt turn), which requires real backend generation — the conformance crate's paired llama/claude integration fixtures already run minutes-long, while the wire shapes are pinned by unit tests in agent-client-protocol-extras and both agents' handler tests assert the identical error kinds/codes. The fleet task (01KTY91Y7AJRPJNBCVTV59HCJJ) is the natural end-to-end proof of the single client code path against both backends.

## Review Findings (2026-06-12 14:12)

> ⚠️ 9/90 review tasks failed — results are INCOMPLETE.

> Reviewer note: this review swept the whole uncommitted working tree; findings under `crates/llama-agent/**` belong to the companion llama fork/resume tasks' changes, not this task's claude-agent scope. Judgment calls evaluated: (1) the skipped shared acp-conformance fork test rationale HOLDS — the engine raised no contrary finding. (2) The two drive-by fixes are sound and red→green tested, not scope creep.

> **Disposition (2026-06-12, second pass):** all claude-agent findings fixed in place; cheap llama-agent findings fixed; substantive llama-agent refactors deferred to follow-up task **01KTYR99HEKMRWB4SJPN71T6FK** (one task, per scope note — these belong to the companion llama tasks' code, and each is a sizable internal refactor). Verification at the bottom.

### Warnings
- [x] `crates/claude-agent/src/claude.rs:196` — `fork_process` runtime-checked `fork_from`. **Fixed:** signature is now `fork_process(&self, parent: SessionId, mut config: SpawnConfig)` — it sets `config.attachment = ConversationAttachment::Fork { parent }` internally (mirroring `resume_process` forcing `Resume`), so a parentless fork call no longer compiles; the stringly `AgentError::Internal` check is gone.
- [x] `crates/claude-agent/src/claude_process.rs:141` — ambiguous `resume: bool` + `fork_from: Option<SessionId>`. **Fixed:** replaced by a single `attachment: ConversationAttachment` field (enum now `pub`, `Fork` carries `SessionId` instead of a pre-rendered uuid String, `#[default] New`). `conversation_attachment()`, the precedence rule/doc, and its dedicated test are deleted; `build_base_command` renders the parent uuid itself.
- [x] `crates/claude-agent/src/session_fork.rs:122` — spawn failures conflated with parent-state-unavailable. **Fixed (headline):** `ClaudeClient::fork_process` returns `Result<(), ForkAttachError>` with `Rejected { detail }` (CLI started then exited immediately — its clean "parent transcript cannot seed a fork" answer) vs `Spawn(AgentError)` (binary missing, spawn I/O), mirroring the store lookups' Err-vs-clean-miss discipline. `fork_attach_error` maps `Rejected` → `invalid_params` + `FORK_PARENT_STATE_UNAVAILABLE` and `Spawn` → retryable `-32603` internal error with no parent-state kind. Regression tests both directions (`test_fork_attach_rejection_maps_to_state_unavailable`, `test_fork_attach_spawn_failure_maps_to_internal_error`, watched red as compile-fail first). The dead early-exited child process is also removed from the process manager.
- [x] `crates/claude-agent/src/session_fork.rs:174` — duplicated session resolution. **Fixed (further than asked):** there were actually THREE copies (agent.rs `resolve_session` was the third). One canonical `ClaudeAgent::resolve_session_with(session_id, not_found)` in agent.rs now backs the core `resolve_session`, `resolve_fork_parent`, and `resolve_extension_session`; the fork-specific ephemeral/first-exchange checks layer on top.
- [x] `crates/claude-agent/src/session_validation.rs:91` — CWD-mutating probe. **Fixed:** traversability is probed side-effect-free via `access(2)` with `X_OK` (libc, honoring euid/ACLs; Windows: `read_dir` success already implies traversal). The set_current_dir/restore dance and the `#[serial]` requirement are deleted; the regression test now asserts the CWD is never mutated.
- [x] `crates/claude-agent/src/session_validation.rs:99` — flattened error source. **Fixed:** `WorkingDirectoryPermissionDenied` carries `#[source] source: Arc<std::io::Error>` (Arc because the enum derives Clone); both failure arms bind the error. Red→green test `test_validate_working_directory_no_execute_preserves_source` (0o600 dir → PermissionDenied with `Error::source()` populated).
- [x] `crates/claude-agent/src/test_support.rs:14` (×3 findings) — third/duplicate `StateDirGuard`. **Fixed:** one canonical `StateDirGuard` now lives in `agent-client-protocol-extras::test_support` (the crate that owns `SessionStore`), gated `#[cfg(any(test, feature = "test-support"))]` exactly like acp-conformance's harness, with `Debug` + `Default` + the SAFETY contract. claude-agent's `test_support.rs` and llama-agent's `acp::test_utils` re-export it; extras' own `with_temp_state` and both claude integration-test `with_temp_state` helpers are now thin wrappers over the guard.
- [x] `crates/llama-agent/src/acp/server.rs:1906` (×2) — `new_session` decomposition → deferred to follow-up 01KTYR99HEKMRWB4SJPN71T6FK (substantive refactor of the companion llama task's code).
- [x] `crates/llama-agent/src/acp/server.rs:3250` — `ext_method` decomposition → deferred to follow-up 01KTYR99HEKMRWB4SJPN71T6FK.
- [x] `crates/llama-agent/src/acp/test_utils.rs:104` — duplicate `StateDirGuard`. **Fixed:** replaced by a `pub use` of the canonical extras guard (see above).
- [x] `crates/llama-agent/tests/integration/real_model_helpers.rs:27` (×2) — real-model config consolidation + skip-heuristic removal → deferred to follow-up 01KTYR99HEKMRWB4SJPN71T6FK (touches six real-model test files from earlier tasks).

### Nits
- [x] `crates/claude-agent/src/agent.rs:3237` — **Fixed:** named `const LIST_PAGE_LIMIT: usize = 10;` with the "any page size large enough to surface a stray record" rationale.
- [x] `crates/claude-agent/src/agent_trait_impl.rs:505` — **Fixed:** `FS_READ_TEXT_FILE_METHOD`, `FS_WRITE_TEXT_FILE_METHOD`, `TERMINAL_{OUTPUT,RELEASE,WAIT_FOR_EXIT,KILL,CREATE}_METHOD`, `EDITOR_UPDATE_BUFFERS_METHOD` constants (same style as the extras session-fork consts), used in both the match arms and the handlers' capability/parse calls — each wire name defined exactly once.
- [x] `crates/claude-agent/src/claude.rs:234` — **Fixed:** `early_exit_detail` (renamed from `early_exit_stderr`) captures the exit status via a new `ClaudeProcess::exit_status()` accessor and combines it with drained stderr; a CLI that writes nothing to stderr still reports `exit status: N`. `stderr_suffix` deleted (detail is never empty now).
- [x] `crates/claude-agent/src/claude_process.rs:1161` — **Fixed:** `test_base_command_loads_filesystem_setting_sources` and `test_base_command_retains_core_streamjson_args` use the `arg_value` helper instead of hand-rolled position lookups.
- [x] `crates/claude-agent/src/session_fork.rs:48` — **Fixed:** the builder moved to `acp_error::session_error` (canonical `{sessionId, error: <kind>}` shape, documented as such) and `ClaudeAgent::session_not_found_error` is now a one-line delegation to it.
- [x] `crates/claude-agent/src/session_fork.rs:393` — **Fixed:** all the module's `#[tokio::test]` functions carry the crate's `test_` prefix, and the `test_config` helper is renamed `headless_config` so the prefix unambiguously marks tests.
- [x] `crates/llama-agent/src/acp/server.rs:1906` — `new_session` rustdoc → deferred to follow-up 01KTYR99HEKMRWB4SJPN71T6FK (written together with the decomposition it documents).
- [x] `crates/llama-agent/src/acp/server.rs:3167` — `require_capability` message struct → deferred to follow-up 01KTYR99HEKMRWB4SJPN71T6FK.
- [x] `crates/llama-agent/src/acp/server.rs:3870` — **Fixed:** all six test-server builders now call the shared `acp::test_utils::test_agent_config(SessionConfig::default())` (zero `batch_size: 512` literals remain in server.rs).
- [x] `crates/llama-agent/src/acp/session_fork.rs:48` — kind newtype → deferred to follow-up 01KTYR99HEKMRWB4SJPN71T6FK with an explicit note that it must move in lockstep with claude-agent's now-canonical `session_error` (same shape, no duplicate-but-different divergence).
- [x] `crates/llama-agent/src/acp/session_fork.rs:351` — **Fixed** (via the acp_integration.rs:978 sibling finding's better resolution): `test_cwd` hoisted into `acp::test_utils` where it now has many callers across two test binaries, instead of inlining.
- [x] `crates/llama-agent/src/acp/session_fork.rs:365` — **Fixed:** the test module uses the shared `test_utils::test_agent_config`, whose `test_model_config` takes every non-load-bearing knob from `..ModelConfig::default()` — the 512 literal is gone.
- [x] `crates/llama-agent/src/acp/session_fork.rs:435` — **Fixed:** `const SEEDED_STATE_BYTES: usize = 64;` used in both `primed_parent` and the `status.bytes` assertion.
- [x] `crates/llama-agent/src/acp/test_utils.rs:104` — **Fixed:** the canonical extras guard derives `Debug` (and `Default`).
- [x] `crates/llama-agent/tests/acp_integration.rs:40` — **Fixed:** both `build_server`/`create_test_server` helpers use the shared `test_agent_config` (no inline `ModelConfig` literal at all).
- [x] `crates/llama-agent/tests/acp_integration.rs:978` — **Fixed:** every `PathBuf::from("/tmp")` in the file (both modules, 10 sites) now uses the shared `test_utils::test_cwd()`.
- [x] `crates/llama-agent/tests/integration/real_model_helpers.rs:35/:37/:45` — config-literal constants → deferred to follow-up 01KTYR99HEKMRWB4SJPN71T6FK (bundled with the file's consolidation).
- [x] `crates/llama-agent/tests/integration/real_model_helpers.rs:109` — `text_prompt` dedupe → deferred to follow-up 01KTYR99HEKMRWB4SJPN71T6FK.

## Review-Fix Verification (2026-06-12, fresh)

- `cargo test -p agent-client-protocol-extras -p claude-agent` — all green (extras 241+50+1+1; claude-agent 736 lib + 317 integration + 3 + 6).
- `cargo test -p llama-agent` — lib 1098, acp_integration 19, agent_tests 107, coverage_tests 225, hook_evaluator 9, all green. One real-model test (`acp_hooks_real_model::pre_tool_use_deny_blocks_real_read_file_through_live_loop`, a file this task never touched) hit its NO_HANG_BUDGET timeout under full-suite load and **passed on isolated rerun** — the documented real-model flake mode, unrelated to these changes.
- `cargo clippy -p agent-client-protocol-extras -p claude-agent -p llama-agent --all-targets -- -D warnings` — clean (0 warnings). `cargo check -p acp-conformance --all-targets` (downstream consumer of `llama_agent::acp::test_utils`) — clean. `cargo fmt --check` on all three crates — clean.
- New red→green tests this pass: `test_fork_attach_rejection_maps_to_state_unavailable` / `test_fork_attach_spawn_failure_maps_to_internal_error` (compile-fail RED on the new `ForkAttachError` API), `test_validate_working_directory_no_execute_preserves_source` (assertion RED on the missing error source). All unit tests stay milliseconds; no real CLI or model in unit scope.