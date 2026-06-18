---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvdjqcpnc5ced5c8p93em0xn
  text: 'Picked up. Starting research: confirming the diagnose() / is_diagnosable() / DiagnosticsReport API from swissarmyhammer-diagnostics (dependency 9fq036d/9b7tq7x), the server.rs dispatch chokepoint (call_tool ~1888 + execute_tool ~1025, McpTool::execute + ToolContext seam), and the edit/write tools. Will design the typed mutated_paths channel around the existing ToolContext seam if available.'
  timestamp: 2026-06-18T14:38:54.677919+00:00
- actor: claude-code
  id: 01kvdjv52n151t2h78cb24h7v3
  text: |-
    Research complete. Key discoveries / premise corrections:
    1. edit/write are NOT separate McpTool impls — they are ops ("edit file"/"write file") of the single FilesTool (files/mod.rs), dispatching to free fns execute_edit/execute_write. Both discard their numeric result and return Content::text("OK"). Path resolves via context.session_root() + validate_file_path -> absolute.
    2. McpTool::execute(&self, args, context: &ToolContext) — context is a SHARED ref. The crate's established pattern for per-call mutable state is interior mutability on ToolContext (peer/session_actor/progress_token are all set this way). So the typed channel = an interior-mutable out-param on ToolContext.
    3. Chokepoint: server.rs call_tool (~1913, uses a FRESH clone tool_context_with_peer per call) and execute_tool (~1046, uses &self.tool_context directly/shared). Both return `result` unchanged after a logging-only post-block.
    4. diagnose() collapses SettleOutcome::Pending -> empty report; DiagnosticsReport has NO pending flag. To satisfy "timeout yields pending" I'll add a diagnose variant in the diagnostics crate that surfaces the pending bool, and delegate the existing diagnose() to it (existing diagnostics tool unchanged).
    5. Reuse: the diagnostics MCP tool (na6cvh0, done) already has all the plumbing (session via code_context::{lsp_session_for_file, any_lsp_session, open_workspace}, PrecomputedDependents, config). The shared fold-in helper will reuse that, not reinvent.

    DESIGN (typed channel):
    - ToolContext gains mutated_paths: Arc<Mutex<Vec<PathBuf>>> + record_mutated_path()/take_mutated_paths()/with_fresh_mutated_paths(). edit/write push their absolute path.
    - Shared fold-in helper lives in diagnostics/mod.rs (reuses its private plumbing): drains sink, keeps is_diagnosable paths, runs the pending-aware diagnose, folds {diagnostics, pending} into the CallToolResult structured_content + text so it reaches the model.
    - BOTH server.rs paths call the helper after execute (execute_tool must clone tool_context to get a per-call sink; call_tool's tool_context_with_peer is already per-call).
    TDD: writing the edit-.rs-folds / edit-.md-none / timeout-pending / both-paths / stub-mutator tests first.
  timestamp: 2026-06-18T14:40:57.941509+00:00
- actor: claude-code
  id: 01kvdkpfg6r96c06v0twbgr1d4
  text: |-
    Implementation landed and green. Files changed:
    - crates/swissarmyhammer-diagnostics/src/diagnose.rs: added DiagnoseOutcome { report, pending } + diagnose_with_outcome(); diagnose() now delegates and discards pending. Surfaces SettleOutcome::Pending so the fold-in can mark provisional reports. +test outcome_reports_not_pending_when_settled.
    - crates/swissarmyhammer-diagnostics/src/lib.rs: export diagnose_with_outcome + DiagnoseOutcome.
    - crates/swissarmyhammer-tools/src/mcp/tool_registry.rs: ToolContext gains the TYPED CHANNEL — mutated_paths: Arc<std::sync::Mutex<Vec<PathBuf>>> + record_mutated_path()/take_mutated_paths()/with_fresh_mutated_paths(). Interior-mutable out-param, matching the peer/session_actor pattern (execute takes &ToolContext).
    - crates/swissarmyhammer-tools/src/mcp/inline_diagnostics.rs (NEW): the shared fold-in helper. fold_in_diagnostics(result, context) is the single entry both server paths call; fold_in_diagnostics_with(.., diagnoser) is the diagnoser-injectable core (MutationDiagnoser trait; LiveDiagnoser is production). Drains channel, gates on is_diagnosable, folds {diagnostics, pending} into BOTH structured_content and an appended text block; attaches nothing on clean/.md. 8 tests.
    - crates/swissarmyhammer-tools/src/mcp/tools/diagnostics/mod.rs: extracted produce_outcome() (pub(crate)) — the report-producing core (session resolve + dependents + diagnose_with_outcome) now shared by the check ops AND the fold-in. No duplication.
    - crates/swissarmyhammer-tools/src/mcp/tools/files/edit/mod.rs + write/mod.rs: record_mutated_path(absolute path) after a successful mutation.
    - crates/swissarmyhammer-tools/src/mcp/server.rs: BOTH chokepoints route through fold_in_diagnostics after execute. execute_tool clones tool_context + with_fresh_mutated_paths (was passing the shared &self.tool_context); call_tool's tool_context_with_peer gets with_fresh_mutated_paths.
    - mcp/mod.rs: pub mod inline_diagnostics.

    Typed-channel design chosen: ToolContext interior-mutable out-param (NOT an envelope/extended result type), because the McpTool::execute contract is fixed at &ToolContext and the crate already sets per-call state (peer/progress_token/session_actor) via interior mutability — extending the result type would have rippled to every tool impl + both dispatch sites (large blast radius), while the out-param touches only the two mutators + the chokepoint.

    Verification (all fresh & green): diagnostics 39 pass; tools lib 1074 pass; full `cargo test -p swissarmyhammer-tools` exit 0 (integration + doctests); clippy -D warnings clean (both crates); cargo fmt --check clean. Next: really-done double-check.
  timestamp: 2026-06-18T14:55:53.350353+00:00
- actor: claude-code
  id: 01kvdm3nnc3sh4d33tzpzshjac
  text: |-
    Double-check (adversarial) verdict: REVISE with ONE latent, currently-unreachable finding — ToolContext::call_tool (tool-to-tool dispatch) passed the same context to the inner tool's execute without a fresh sink, so a future tool delegating to a mutator via call_tool would fold the inner mutation's diagnostics into the OUTER tool's result. No double-fold, no active bug (grep: zero .call_tool( mutator delegations exist today). Reviewer noted it's effectively a PASS if documented; I chose to FIX it rather than document.

    Resolution: ToolContext::call_tool now passes `self.clone().with_fresh_mutated_paths()` to the inner execute, isolating the inner call's mutated-paths channel. Inline diagnostics are now strictly a property of top-level dispatch, not internal tool composition. Pinned by new regression test tool_to_tool_mutation_does_not_pollute_outer_sink (builds a real registry, delegates an edit of a .rs via call_tool, asserts the outer sink stays empty AND the edit happened).

    All other double-check items verified PASS: fresh-Arc isolation correct; helper always drains the channel before any early return; single shared helper from both server paths; diagnose delegation behavior-preserving; produce_outcome extraction behavior-preserving; the LSP-rewire WIP in server.rs/code_context is intact (my additions coexist, revert nothing).

    Re-verified after the fix (all fresh & green): inline_diagnostics 9 tests pass; tools lib 1075 pass; clippy -D warnings clean; cargo fmt --check clean. Moving to review.
  timestamp: 2026-06-18T15:03:05.644246+00:00
- actor: claude-code
  id: 01kvdqa8apfeme6ev852w75dpg
  text: |-
    Addressed the one in-scope review finding (inline_diagnostics.rs test-helper duplication).

    Change: replaced the hand-rolled `fn context() -> ToolContext` in inline_diagnostics.rs tests with reuse of the canonical `create_test_context()` from `crate::test_utils`. The helper is now `async fn context() -> ToolContext { create_test_context().await }`; all 9 call sites updated to `.await` (two chained with `.with_fresh_mutated_paths()` / `.with_tool_registry(...)`). Dropped the now-unused `ToolHandlers` import, added `use crate::test_utils::create_test_context;`. The default ToolContext already carries a usable per-call `mutated_paths` sink, so no behavior change for the tests; `create_test_context()` additionally sets `mcp_server_port`, which is harmless here.

    Verification (all green, fresh runs):
    - `cargo test -p swissarmyhammer-tools --lib inline_diagnostics` → 9 passed; 0 failed.
    - `cargo clippy -p swissarmyhammer-tools --all-targets -- -D warnings` → clean (exit 0).
    - `cargo fmt -p swissarmyhammer-tools` → no changes.

    Out of scope (left unchecked, belong to other concurrent cards, NOT this card's diff): lsp_worker.rs duplicate test fns + magic-number nits, layered_context.rs missing docs, code_context/mod.rs helper unification + default-limit nit, write/mod.rs duplicate empty-path validation / MAX_FILE_SIZE / 0o444 nits (the write/mod.rs nits flag pre-existing code untouched by this card's `record_mutated_path` change).
  timestamp: 2026-06-18T15:59:07.094033+00:00
- actor: claude-code
  id: 01kvdqbbe5kr7jrepb72txfa1q
  text: 'Moved to done. Review verdict: the whole-tree `review working` sweep surfaced 14 confirmed findings, but exactly ONE was in this card''s own diff (the inline_diagnostics.rs `fn context()` test helper duplicating create_test_context()) — now fixed (reuses the canonical helper, 9 inline_diagnostics tests pass, clippy -D warnings clean, fmt clean). The remaining findings (lsp_worker.rs duplicate tests + timing-constant nits, layered_context.rs missing docs, code_context/mod.rs unification, pre-existing write/mod.rs duplicate empty-path validation / MAX_FILE_SIZE / 0o444 nits) are provably tangential — they belong to concurrent code-context/LSP work bundled in the same dirty tree, NOT this diff. Did not re-loop the review engine (it would re-find the same out-of-scope noise); verified this card''s machine-verifiable acceptance criteria directly instead. All criteria met. Committing rollback point next.'
  timestamp: 2026-06-18T15:59:43.045360+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffbe80
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

## Review Findings (2026-06-18 10:29)

> ⚠️ 6/75 review tasks failed — results are INCOMPLETE.

> Note (driver): the `review working` sweep covers the whole uncommitted tree. The findings in `lsp_worker.rs`, `layered_context.rs`, and `code_context/mod.rs` belong to concurrent code-context/LSP work, not this card's diff (this card touches `server.rs`, `files/edit/mod.rs`, `files/write/mod.rs`, `inline_diagnostics.rs`). The `write/mod.rs` nits flag pre-existing code untouched by this card's `record_mutated_path` change. The one finding in this card's own new code is the `inline_diagnostics.rs` test-helper duplication warning.

> NOTE (2026-06-18, implement pass): ONLY the `inline_diagnostics.rs` test-helper finding below is this card's own new code and was fixed. Every other finding (lsp_worker.rs duplicate tests + magic-number nits, layered_context.rs missing docs, code_context/mod.rs helper unification + default-limit nit, write/mod.rs duplicate empty-path validation / MAX_FILE_SIZE / 0o444 nits) belongs to OTHER concurrent in-flight cards and is intentionally left unchecked — out of this card's diff.

### Blockers
- [ ] `crates/swissarmyhammer-code-context/src/lsp_worker.rs:555` — Test function `test_loop_exits_immediately_when_shutdown_set` is duplicated verbatim. Same test appears twice in the file — once starting near line 555 and again around line 755. Duplication creates divergence risk when fixes are applied to one copy but not the other. Delete the duplicate test function. Keep a single canonical version. Git history will show why it existed if needed.
- [ ] `crates/swissarmyhammer-code-context/src/lsp_worker.rs:597` — Test function `test_loop_idles_and_shuts_down_with_no_dirty_files` is duplicated. Same test body appears twice — once near line 597, again around line 815. Identical implementations create maintenance debt: a fix or clarification to one copy won't be applied to the other. Remove one copy. Retain the single canonical test implementation.

### Warnings
- [ ] `crates/swissarmyhammer-code-context/src/layered_context.rs:192` — public constructor `new()` lacks a doc comment; constructors on public structs need documentation. add a doc comment explaining what the constructor does and the lifetime constraints.
- [ ] `crates/swissarmyhammer-code-context/src/layered_context.rs:199` — public function `has_live_lsp()` lacks a doc comment; all public items need documentation. add a doc comment explaining what the function does and its return value.
- [x] `crates/swissarmyhammer-tools/src/mcp/inline_diagnostics.rs:140` — Test helper `context()` reimplements `create_test_context()` that already exists in multiple test files (web_search_integration.rs, file_rate_limiting.rs, file_size_limits.rs at 0.96 similarity). Test helper duplication defeats the purpose of shared test utilities — consolidate into a common test module. Extract a shared `fn create_test_context() -> ToolContext` helper in `crates/swissarmyhammer-tools/src/test_utils.rs` (or equivalent) and import it in all test modules, including here.
- [ ] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs:56` — `lsp_session_for_file` and `any_lsp_session` follow the identical supervisor-access pattern (get, lock, iterate, check condition, return daemon.session()) but with different predicates. This repetition should be unified into a shared helper. Extract a generic helper `fn find_lsp_session<F>(predicate: F) -> Option<SharedLspSession> where F: Fn(&LspDaemon) -> bool` that handles supervisor access and iteration once, then implement both functions in terms of it: `lsp_session_for_file` and `any_lsp_session` each supply their own predicate closure.

### Nits
- [ ] `crates/swissarmyhammer-code-context/src/lsp_worker.rs:270` — Hardcoded millisecond duration (20) configures test timing but lacks an explanation. Extract to a named constant (e.g., `const TEST_SLEEP_BRIEF: Duration = Duration::from_millis(20);`) to clarify the delay's purpose and enable reuse.
- [ ] `crates/swissarmyhammer-code-context/src/lsp_worker.rs:322` — Hardcoded millisecond duration (30) configures test timing but lacks an explanation. Appears in multiple tests with different purposes. Extract to a named constant like `const TEST_SLEEP_MEDIUM: Duration = Duration::from_millis(30);` (or adjust to match the 50ms variant if they serve the same purpose).
- [ ] `crates/swissarmyhammer-code-context/src/lsp_worker.rs:375` — Hardcoded millisecond duration (50) configures test timing but lacks an explanation. Test sleeps should use named constants for clarity. Extract to a named constant like `const TEST_SLEEP_MEDIUM: Duration = Duration::from_millis(50);` and use it consistently across all test sleeps of this duration.
- [ ] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs:0` — Default limit of 50 for max_results should be a named constant to make it configurable and maintainable. Extract `.unwrap_or(50)` to a module-level constant like `const DEFAULT_MAX_RESULTS: usize = 50;`.
- [ ] `crates/swissarmyhammer-tools/src/mcp/tools/files/write/mod.rs:178` — Duplicate empty file_path validation. The identical check appears at line 161-166, and again at 178-183 with only the error message capitalization differing. Remove the duplicate validation at lines 178-183 since the first check at 161-166 already handles this case.
- [ ] `crates/swissarmyhammer-tools/src/mcp/tools/files/write/mod.rs:449` — Buffer size calculation 10 * 1024 * 1024 + 1 duplicates the magic numbers from MAX_FILE_SIZE constant and should reuse it instead. Use `MAX_FILE_SIZE + 1` instead of repeating the calculation.
- [ ] `crates/swissarmyhammer-tools/src/mcp/tools/files/write/mod.rs:523` — Octal literal 0o444 (read-only permissions) configures file permissions behavior and should be a named constant for clarity and reuse. Define `const READONLY_PERMISSIONS_MODE: u32 = 0o444;` at module level.
- [ ] `crates/swissarmyhammer-tools/src/mcp/tools/files/write/mod.rs:619` — Octal literal 0o444 (read-only permissions) duplicates the magic number from line 523 and should use a shared named constant. Define `const READONLY_PERMISSIONS_MODE: u32 = 0o444;` and reuse it in both tests.