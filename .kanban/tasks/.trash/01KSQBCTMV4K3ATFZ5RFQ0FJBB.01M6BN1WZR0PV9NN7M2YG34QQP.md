---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffb480
project: llama-coverage
title: Measure baseline coverage for llama-agent + ACP, produce the gap map
---
## What

Establish the coverage baseline for `crates/llama-agent` (including the `acp/` submodule) so the rest of this epic targets real gaps instead of guesses. The streaming 0-token bug proved that high test *volume* (819 lib tests) coexisted with zero coverage of the production streaming path — so we need a line/region map, not a test count.

## Steps

1. Pick the instrumentation already used in this repo. Check the `coverage` skill / existing CI config for `cargo-llvm-cov` vs `cargo-tarpaulin`. Use whatever is already wired; do not introduce a second tool.
2. Run coverage scoped to `llama-agent` only (the workspace run is huge and slow):
   - `cargo llvm-cov --package llama-agent --html` (or tarpaulin equivalent), plus a `--lcov`/`--json` export for diffing.
3. Produce a **per-file gap report** ranked by uncovered regions, with special attention to the large/critical files:
   - `generation/mod.rs`, `generation/generator.rs`
   - `stopper/*`
   - `queue.rs`
   - `chat_template.rs` (8.3k lines)
   - `acp/server.rs` (5.2k), `acp/translation.rs` (3k), `session.rs` (2.2k), `agent.rs` (2.4k)
4. For each major uncovered region, classify it: **pure logic** (testable now, no model), **model-dependent** (needs the scripted-model harness — card `<harness-id>`), or **dead/unreachable** (candidate for deletion).
5. Write the gap map into this task's comments/description as a checklist the downstream cards can consume. Do NOT create the downstream cards here — they already exist in this project; instead annotate which files each should target.

## Acceptance Criteria

- [x] A coverage run for `llama-agent` completes and the tool + exact command are recorded in this task.
- [x] A per-file uncovered-region report exists (committed as an artifact or pasted into the task), ranked worst-first.
- [x] Each major gap is classified pure-logic / model-dependent / dead.
- [x] Baseline overall % for the crate is recorded so the final coverage-gate card can set a threshold above it.

## Tests

- [x] N/A — this is a measurement task, not a code change. The "test" is that `cargo llvm-cov --package llama-agent` runs to completion and emits a report. It ran to completion (exit 0, all tests passing).

## Workflow

- Use the `coverage` skill if it fits.
- This card unblocks the targeted-coverage cards; it does not itself add tests.

---

# BASELINE RESULTS (pre-epic, measured 2026-05-28)

## Tool + exact command

- **Tool**: `cargo-llvm-cov` 0.6.21 (already installed; it is the repo/coverage-skill default. `cargo-tarpaulin` is also present but a single tool is used per the card — llvm-cov, which the `coverage` skill names as preferred and which the card itself specifies). No CI coverage job exists yet, so there was no pre-wired command to match.
- **Clean step**: `cargo llvm-cov clean --package llama-agent`
- **Measurement command** (this is the recorded baseline command):

  ```
  cargo llvm-cov --package llama-agent --lcov --output-path llama-agent-baseline.lcov
  ```

- **Per-file summary table** (region/line/function, workspace-rendered, then scoped): `cargo llvm-cov report --package llama-agent`
- **Gap report generator**: `python3 scripts/llama_agent_gap_report.py llama-agent-baseline.lcov` → `llama-agent-gap-report.txt` (committed artifact; the 3.9 MB raw LCOV is transient and regenerable, not committed).
- **Model note**: the suite's real-model tests use the small `unsloth/Qwen3-0.6B-GGUF` (~600 MB, hardcoded in `src/test_models.rs`), already in the HF cache; no 27B / large model is pulled. Model tests are NOT `#[ignore]`-gated, so they run in this baseline.

## Baseline overall coverage (the number the final coverage-gate card sits above)

- **llama-agent crate, line coverage: 78.01%** — 18219 / 23356 covered lines, **5137 uncovered** (LCOV `DA:` scope = `crates/llama-agent/src/**`, 56 files).
- Cross-check from `cargo llvm-cov report` per-file "lines" column agrees within rounding (e.g. generator.rs 0.00%, chat_template.rs ~81%, session.rs ~90%).
- For the gate card: set the threshold floor at **78% line coverage** for the crate and ratchet up as targeted cards land. The most load-bearing single number: **`generation/generator.rs` is 0.00% covered (the entire production llama.cpp inference + streaming path) — this is the streaming-0-token blind spot.**

## Per-file gap report — ranked worst-first (by uncovered line count)

Full ranges in `llama-agent-gap-report.txt`. Classification legend: **[MODEL]** model-dependent (needs scripted-model / real-model harness), **[PURE]** pure logic (testable now, no model), **[MIXED]** both, **[DEAD]** unreachable/candidate for deletion.

| Rank | File | Cov% | Uncov lines | Class | Notes / what the downstream card should target |
|------|------|------|-------------|-------|-----------------------------------------------|
| 1 | `chat_template.rs` (8.3k) | 81.3% | 892 | **[PURE]** | Jinja/chat-template rendering, tool-call parsing, message formatting. Pure string→string logic. Largest absolute gap that is fully testable now without a model. Highest ROI for pure-logic card. |
| 2 | `acp/server.rs` (5.2k) | 68.3% | 863 | **[MIXED]** | Uncovered = ACP `Agent` impl: `prompt` (model turn → [MODEL]); `new_session`, `authenticate`, `cancel`, `set_session_mode`, `dispatch_client_request/notification`, `ext_method/notification`, `filesystem_error_to_protocol_error`, `agent_type`, `build_lines_transport` (protocol plumbing → [PURE], drivable via in-process ACP harness without inference). |
| 3 | `generation/generator.rs` | **0.0%** | 740 | **[MODEL]** | `LlamaCppGenerator` — the entire production inference path: `generate_text_with_context`, `generate_stream_with_context`, `process_prompt(_incremental)`, `process_new_tokens`, `create_sampler`, `token_to_str_lossy`, `handle_streaming_completion`. Every method holds a live `LlamaModel`/`LlamaContext`. **This is the streaming-0-token bug's home. Top priority for the model/scripted-model harness card.** |
| 4 | `agent.rs` (2.4k) | 49.6% | 430 | **[MODEL]** | Uncovered = `prompt`, `submit_streaming_request`, `execute_tool_with_retry`, `generate_session_title` / `title_via_model`, `maybe_auto_compact`, `create_summary_generator` — orchestration that drives generation + tool loops. Needs the model harness. (`example` fns are doc examples → ignore.) |
| 5 | `generation/mod.rs` | 38.3% | 371 | **[MODEL]** | `GenerationHelper::generate_text_with_borrowed_model`, `generate_stream_with_borrowed_model`, `generate_common` — borrowed-model batch/stream loops. Covered remainder = pure helpers (`should_stop`, validation). Model harness card. |
| 6 | `mcp.rs` | 46.6% | 198 | **[PURE]** | MCP client wiring / tool discovery / server config translation. No model; testable with an in-process MCP server (pattern already exists via `swissarmyhammer-tools` dev-dep). Pure-logic/integration card. |
| 7 | `model.rs` | 71.3% | 190 | **[MIXED]** | Model loading from HF/local, context creation, GGUF metadata. Load path is [MODEL] (real load), but config validation + source resolution + error mapping are [PURE]. |
| 8 | `queue.rs` | 86.8% | 175 | **[MIXED]** | Request queue. Most logic covered. Uncovered = worker generation hot path (calls into generator → [MODEL]) plus a few error/edge branches ([PURE]). |
| 9 | `session.rs` | 89.1% | 161 | **[PURE]** | Session state, message history, KV-cache bookkeeping accessors. Uncovered = mostly error branches and rarely-hit accessors. Pure-logic card. |
| 10 | `acp/error.rs` | 58.6% | 157 | **[PURE]** | `TerminalError` / ACP error → JSON-RPC + `to_protocol_error` match arms. Trivial pure-logic Display/conversion tests. High ROI. |
| 11 | `acp/translation.rs` (3k) | 91.7% | 133 | **[PURE]** | ACP↔internal type translation. Already high; uncovered = remaining enum/variant arms + error conversions. Pure-logic card. |
| 12 | `acp/terminal.rs` | 86.1% | 79 | **[MIXED]** | Terminal create/exec/output. Process-spawning paths [MIXED]; state/error arms [PURE]. |
| 13 | `types/errors.rs` | 45.0% | 72 | **[PURE]** | `AgentError`/`SessionError` `category()`, `error_code()`, `Display`, `From` impls. Pure match-arm tests. High ROI. |
| 14 | `mcp_client_handler.rs` | 70.7% | 67 | **[PURE]** | MCP client callback handler. In-process MCP server test. Pure/integration. |
| 15 | `storage.rs` | 72.2% | 60 | **[PURE]** | Session persistence to disk. tempfile-based pure-logic card. |
| 16 | `acp/test_utils.rs` | 18.8% | 52 | **[DEAD?]** | Test helper module — low coverage expected (helpers only run when a test uses them). Not a product gap; do NOT target. Candidate to ignore in the gate (exclude test_utils from the metric). |
| 17 | `acp/filesystem.rs` | 88.3% | 47 | **[PURE]** | ACP filesystem ops (read/write/permission). tempfile pure-logic card. |
| 18 | `stopper/max_tokens.rs` | 50.0% | 43 | **[MIXED]** | `max_tokens()`/`tokens_generated()`/`remaining()` accessors → [PURE], easy now. `should_stop(&mut self, ctx, batch)` ignores `_context` but its signature requires a `LlamaContext` to invoke → [MODEL] to drive (needs harness that can mint a context). |
| 19 | `dependency_analysis.rs` | 84.2% | 37 | **[PURE]** | Tool-call dependency/parallelism analysis. Pure logic; high ROI to finish off. |
| 20 | `acp/mcp_client_factory.rs` | **0.0%** | 35 | **[PURE]** | `create_mcp_client_from_acp` — builds an MCP client from ACP `McpServer` config. No model; needs an MCP server fixture. Integration/pure card. |
| — | `validation/errors.rs` (40%, 30) | | | **[PURE]** | Validation error Display/conversion. Pure. |
| — | `validation/agent_validator.rs` (74%, 28) | | | **[PURE]** | Config validation rules. Pure. |
| — | `acp/session_resume.rs` (81%, 26) | | | **[MIXED]** | Resume-from-record; record parsing [PURE], replay-into-context [MODEL]. |
| — | `echo.rs` (75%, 21) | | | **[PURE]** | Echo transport for examples/tests. Pure. |
| — | `acp/elicitation.rs` (88%, 21) | | | **[PURE]** | Elicitation request/response handling. Pure. |
| — | `types/errors.rs`, `types/ids.rs`, `types/mcp.rs`, `types/configs.rs` | | | **[PURE]** | Type/Display/serde tail gaps. Pure. |
| — | `stopper/eos.rs` (79%, 13) | | | **[MODEL]** | `should_stop` reads `context` (EOS token lookup) → needs a real context. |

### Files at or near 100% (no action)

`acp/acp_error.rs` 100%, `generation/config.rs` 100%, `generation/error.rs` 100%, `test_models.rs` 100%, `types/tools.rs` 100%, `validation/generation_request/mod.rs` 100%, `validation/generation_request/session_validator.rs` 100%, `acp/permissions.rs` 99.9%, `validation/mod.rs` 99.2%, `validation/queue_validator.rs` 98.3%, `validation/generation_request/composite_validator.rs` 98.6%, `types/sessions.rs` 98.1%, `validation/tool_call/argument_validator.rs` 98.3%.

## Gap map → downstream card routing (annotation only; cards already exist)

- **Model / scripted-model harness card** (the `<harness-id>` referenced in step 4): owns the [MODEL] files — `generation/generator.rs` (0%, the streaming-0-token path — do this first), `generation/mod.rs`, `agent.rs`, `stopper/eos.rs`, the `prompt` path in `acp/server.rs`, the worker hot path in `queue.rs`, and the load path in `model.rs`. These cannot be covered without a real/scripted model.
- **Pure-logic coverage card(s)**: owns the [PURE] files, ranked by ROI — `chat_template.rs` (892, biggest pure gap), `acp/error.rs` + `types/errors.rs` + `validation/errors.rs` (error/Display match arms, trivial), `acp/translation.rs`, `session.rs`, `dependency_analysis.rs`, `acp/filesystem.rs`, `storage.rs`, `acp/elicitation.rs`, type-tail files.
- **MCP/integration coverage card**: `mcp.rs`, `mcp_client_handler.rs`, `acp/mcp_client_factory.rs` (0%) — drivable with the existing in-process MCP server pattern (`swissarmyhammer-tools` dev-dep), no model.
- **ACP-protocol coverage card**: the [PURE] half of `acp/server.rs` (new_session/authenticate/cancel/set_session_mode/dispatch/ext/error-mapping) + `acp/terminal.rs` state/error arms — drivable via in-process ACP harness without inference.
- **Coverage-gate card**: set the crate floor at **78% line coverage**; recommend EXCLUDING `acp/test_utils.rs` (and any `*test_utils.rs` / `tests/` helper modules) from the gated metric so test scaffolding does not depress the product number.

### Dead-code note

No clearly dead/unreachable product code was found in the worst-first set; the one genuinely-low outlier (`acp/test_utils.rs` 18.8%) is test scaffolding (only exercised when a test calls it), not dead product code — exclude it from the gate rather than delete it.

## Artifacts (committed)

- `scripts/llama_agent_gap_report.py` — LCOV → per-file gap report parser (reusable for re-measuring after each card).
- `llama-agent-gap-report.txt` — full worst-first per-file report with exact uncovered line ranges.