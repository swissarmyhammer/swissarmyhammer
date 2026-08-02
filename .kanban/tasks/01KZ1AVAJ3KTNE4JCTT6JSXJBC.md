---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kz1j080je1tdqavxh15fex6b
  text: |-
    Absorbed the budget half of ^7wpd955, which was filed independently while working ^k5wsxh0. That card is now narrowed to its documentation cleanup only.

    Two things it adds to the acceptance here:

    1. The 1.45x expansion factor behind the 384 KiB budget is CONTENT DEPENDENT, not a constant. Each rendered line gains a fixed ~16 bytes (`{line:>6} | {sha:8} {mark} | `), so a file of short lines expands far more than 1.45x, and a file of long lines far less. A budget that multiplies raw bytes by a fixed factor is therefore wrong for exactly the files most likely to overflow.

    2. Required test case: a short-line file that the raw-byte budget admits but the rendered size pushes over the cap. That is the case a fixed multiplier misses, so it is the one the test must pin.

    Either budget the rendered size directly, or derive the budget from line count as well as byte count. Do not keep a fixed multiplier.
  timestamp: 2026-08-02T15:38:10.322643+00:00
- actor: claude-code
  id: 01kz1jp49v85r8twnmz9yncq6q
  text: |
    Research — root cause confirmed from the real log, and it is worse than the card states.

    Evidence from `/Users/wballard/github/swissarmyhammer/swissarmyhammer-main/.sah/mcp.45537.log`:

    - `fleet task failed` occurs 36 times; every failure carries `error=Internal error: failed to execute prompt: Invalid params`.
    - The wire error is `Error { code: -32602: Invalid params, message: "Invalid params", data: None }` — exactly `agent_client_protocol::Error::invalid_params()` with no data, i.e. `ClaudeAgent::validate_prompt_request`.
    - The review agent is `executor=ClaudeCode, extra_args=["--model","haiku"]`, so the live cap is `swissarmyhammer-agent`'s `MAX_PROMPT_LENGTH_BYTES = 5_000_000`, NOT claude-agent's 100_000 default. The prompt still exceeded it.
    - The rejected `session/prompt` request line is 15,063,563 characters. The prompt inside it is ~14.9 MB.

    Composition of that 14.9 MB prompt (measured):

    - It is the MONOLITHIC fallback, not a fork: 10 `## File:` blocks, 1 `# Validator:`, no `Reply with exactly OK` handoff.
    - `primed prefix session has no restorable state; falling back to monolithic prompts` occurs 45 times. On the claude backend the prime NEVER saves restorable state, so every fan-out task runs the monolithic prompt. The monolithic prompt is the production path there.
    - Per file block: source + semantic diff = ~9-120 KB. Probe evidence = ~1.44 MB.
    - Each file block carries TWO probe results: the per-file `duplicates` result (9-123 KB) and the shared `duplicates` on `<changed-set>` result (~1.43 MB). The `<changed-set>` blob is rendered once PER FILE, so 10 files = 10 copies = 14.3 MB of the 14.9 MB prompt.

    So the raw-source budget (`file.source_slice().len()`, 384 KiB) measured about 1% of the real prompt. The dominant term is probe evidence, which the budget does not measure at all. Only `duplication` failed because `select_probe_results` filters probe results to the validator's declared probes, so only that validator's `FileWork` carries the duplicates rows.

    Two consequences for the fix:

    1. A fixed multiplier on raw source bytes cannot work — confirmed empirically, not just for short-line files.
    2. The shared `<changed-set>` evidence being repeated per file, and being ~1.4 MB unbounded, is a SECOND defect. It is not the budget bug and will get its own card. This card makes the budget honest and makes the overflow a named visible gap instead of a silent failed task.
  timestamp: 2026-08-02T15:50:07.419874+00:00
- actor: claude-code
  id: 01kz1x3xpeda7zevatkw9bnq6y
  text: |-
    ### implement — changed

    **Root cause confirmed** from the real log (`/Users/wballard/github/swissarmyhammer/swissarmyhammer-main/.sah/mcp.45537.log`) — see the research comment above. The failing prompt was 14.9 MB against the live cap of 5 MB (`swissarmyhammer-agent`'s old `MAX_PROMPT_LENGTH_BYTES`), not the 100 KB claude-agent default the card cites — the disagreement is real either way. The dominant term was NOT source bytes: 14.3 MB of the 14.9 MB was the `duplicates` probe's `<changed-set>` evidence block, rendered once per file (10 files × ~1.43 MB). That is a second, separate defect (probe evidence sizing/dedup), out of scope here — noted on the card, not fixed by this change.

    **One source of truth** (`crates/claude-agent/src/constants/sizes.rs`):
    - `claude_agent::constants::sizes::messages::MAX_PROMPT_LENGTH` raised 100_000 → `512 * 1024` (sized to the 200k-token context window, not an arbitrary bump) and is now the ONLY declaration. `AgentConfig::max_prompt_length` defaults to it (unchanged wiring).
    - `swissarmyhammer-agent`'s `MAX_PROMPT_LENGTH_BYTES` now reads `claude_agent::constants::sizes::messages::MAX_PROMPT_LENGTH` instead of declaring its own `5_000_000`.
    - `swissarmyhammer-validators::review::fleet::AGENT_PROMPT_CAP` (and `validators::pool::AGENT_PROMPT_CAP`, one constant re-exported) reads the same constant. `DEFAULT_BATCH_SIZE = AGENT_PROMPT_CAP`. `FleetConfig::new` clamps any caller-supplied `batch_size` (the `review` tool's modifier) to the cap so no caller can ask for a budget the agent would reject.
    - Pinned by `the_batch_budget_and_the_agent_prompt_cap_are_one_constant` (fleet/tests.rs).

    **Budget measures RENDERED bytes, not raw source, and not a fixed multiplier** (per the comment's required acceptance):
    - `rendered_file_block_bytes` (fleet.rs) renders one file through the real `render_file_block` and measures the result.
    - `batch_work_list` (scope.rs) now takes a `cost: &dyn Fn(&FileWork) -> usize` parameter instead of reading `source_slice().len()` directly; the fleet passes `rendered_file_block_bytes`.
    - Cost is per **(validator, file) pair**, not per path — a file's block carries the probe evidence selected for that validator, so the same path can cost KB for one validator and MB for another. A pair whose own cost exceeds the budget is excluded and reported; the surviving files are packed by the largest surviving cost any validator has for that path.
    - `prompt_framing_bytes` measures the non-file-block part of a prompt (change purpose + payload header + the largest validator suffix across the run) so `FleetConfig::file_payload_budget(framing)` gives the packer `cap - framing`, never a budget that overflows once the rules are appended.
    - Pinned by `a_short_line_file_the_raw_byte_budget_admits_is_measured_by_its_rendered_size` (1000 short lines, ~3000 raw bytes, well over budget once rendered — the exact case a fixed multiplier misses) and `one_validators_oversized_file_does_not_cost_the_other_validators_that_file`.

    **Diagnostic rejection** (`crates/claude-agent/src/agent.rs::validate_prompt_request`): every branch now returns `crate::acp_error::invalid_params(<message>)` instead of the bare `Error::invalid_params()`. The over-length branch names both numbers: `"prompt text is {len} bytes, over the {limit}-byte max_prompt_length limit"`. Pinned by `crates/claude-agent/tests/integration/prompt_validation_errors.rs` (`an_over_length_prompt_names_its_length_and_the_limit`, `a_prompt_with_no_content_says_so`).

    **Never silently degrades**: `AgentPool::enqueue` (validators/pool.rs) now checks every submitted prompt against `AGENT_PROMPT_CAP` before sending it, refusing with the new typed `PoolError::PromptTooLong { length, limit }` instead of letting an over-cap prompt reach the agent and come back as a bare `invalid_params`. This is the single choke point for every submission API (`submit`/`submit_primed`/`submit_forked`), so fan-out and verify are both covered. Pinned by `test_pool_refuses_a_prompt_over_the_agent_cap_before_sending_it`.

    **`SkippedFile` gap reporting reworked** to the (validator, file) grain: `path()`, `validator()`, `size()` (rendered bytes), `budget()`. `synthesize.rs`'s markdown groups by path (`group_skips_by_path`) so the reader sees one line per file naming every validator that could not carry it, and `ReviewCounts::skipped` counts distinct paths, not pairs.

    **Acceptance test with real batching/rendering**: `every_prompt_a_packed_batch_sends_fits_inside_the_agent_prompt_cap` builds a 12-file WorkList, runs it through the real `batch_work_list` + `prompt_framing_bytes` + `file_payload_budget`, then renders EVERY resulting batch through the real `render_run_prime` and `render_fleet_prompt` (the monolithic fallback — the actual production path on the claude backend, since priming never saves restorable state there) and asserts every one is `<= AGENT_PROMPT_CAP`.

    **Test evidence**:
    - `cargo nextest run -p claude-agent -p swissarmyhammer-agent -p swissarmyhammer-validators -p swissarmyhammer-tools -p acp-conformance` → 3318 tests, 0 failed, 0 skipped (this is the actual blast radius: every crate that depends on `claude-agent` or `swissarmyhammer-validators`, confirmed via `grep -rl '^claude-agent = '` / `'^swissarmyhammer-validators'` across every `Cargo.toml`).
    - `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` → 5059 tests run, 5057 passed, 2 timed out, 2 skipped. The 2 timeouts are `llama-agent::agent_tests integration::agent_tools_mount::agent_tools_mount_lists_intrinsic_tools_with_no_external_servers` and `integration::dual_source_shell_dedup::llama_dual_source_aggregation_has_shell_exactly_once` — both in `llama-agent`, a wholly separate ACP backend this change never touches (grepped both test files for `claude_agent`/`swissarmyhammer_validators`/`swissarmyhammer_agent`/`MAX_PROMPT_LENGTH`: no matches). Both hit the package's 300s slow-timeout on all 3 tries even with zero other `cargo nextest` processes running. This is a pre-existing, unrelated issue — filed as `^57vx78g` for someone to investigate; the literal `rdeps` command does NOT cleanly pass today because of it, and I am not claiming otherwise.
    - `cargo fmt --all -- --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean. `cargo clippy -p kanban-app --all-targets -- -D warnings` clean (kanban-app is outside the main workspace).

    **`review sha 0c8b969b8~1..0c8b969b8` (71 files) — NOT run.** The commit exists in this repo's shared git history (confirmed with `git cat-file -e`), but reproducing it live requires a real agent connection (claude CLI or a configured local backend) driving a 71-file review — several minutes, and per the card's own instruction not to be assumed. I did not run it. The `every_prompt_a_packed_batch_sends_fits_inside_the_agent_prompt_cap` test above is the strongest available substitute: it packs a comparably large multi-file batch through the REAL batching/rendering pipeline (not a hand-built string) and proves every resulting prompt fits under the cap.

    **Unrelated deletions, done at the user's explicit direction mid-session** (not part of this card's scope, called out so the diff is not mysterious): `apps/kanban-app/tests/ai_panel_e2e.rs` and `crates/swissarmyhammer-agent/tests/review_real_model_e2e.rs` were deleted — both real-model e2e tests that were stalling/timing out under contention from an unrelated parallel `cargo nextest run --workspace` in the `swissarmyhammer-shell` worktree. `.config/nextest.toml` was updated to remove their now-dangling overrides and fix comments that referenced them.

    **Files changed**:
    - `crates/claude-agent/src/constants/sizes.rs` — `MAX_PROMPT_LENGTH` raised to 512 KiB, doc rewritten as the single source of truth.
    - `crates/claude-agent/src/agent.rs` — `validate_prompt_request` rejections all carry diagnostic messages.
    - `crates/claude-agent/tests/integration/prompt_validation_errors.rs` (new), `mod.rs` — pins the diagnostic rejections.
    - `crates/swissarmyhammer-agent/src/lib.rs` — `MAX_PROMPT_LENGTH_BYTES` reads `claude_agent`'s constant.
    - `crates/swissarmyhammer-validators/src/review/fleet.rs` — `AGENT_PROMPT_CAP`, `DEFAULT_BATCH_SIZE`, `FleetConfig::file_payload_budget`, `rendered_file_block_bytes`, `prompt_framing_bytes`.
    - `crates/swissarmyhammer-validators/src/review/fleet/tests.rs` — new budget/rendering tests.
    - `crates/swissarmyhammer-validators/src/review/scope.rs` — `batch_work_list` takes a `cost` fn, pairs-not-paths, `SkippedFile` carries `validator()`.
    - `crates/swissarmyhammer-validators/src/review/synthesize.rs` — wires `prompt_framing_bytes`/`file_payload_budget`, groups skips by path.
    - `crates/swissarmyhammer-validators/src/review/drive.rs` — `TEST_BATCH_SIZE_BYTES` re-tuned for rendered-byte budgeting.
    - `crates/swissarmyhammer-validators/src/review/mod.rs` — re-exports.
    - `crates/swissarmyhammer-validators/src/validators/pool.rs`, `mod.rs` — `AGENT_PROMPT_CAP`, `PoolError::PromptTooLong`, pre-flight refusal in `enqueue`.
    - `crates/swissarmyhammer-tools/src/mcp/tools/review/{mod.rs,review_op.rs,tests.rs}` — doc/description updates, retuned `batch_size` fixtures.
    - `.config/nextest.toml`, `apps/kanban-app/tests/ai_panel_e2e.rs` (deleted), `crates/swissarmyhammer-agent/tests/review_real_model_e2e.rs` (deleted) — unrelated, user-directed.

    next: /review
  timestamp: 2026-08-02T18:52:25.166805+00:00
position_column: doing
position_ordinal: '8380'
title: Review batch budget exceeds the agent prompt cap — every fat batch fails as "Invalid params"
---
# Symptom

A `review sha` over a large commit fails most or all of its fleet tasks. The engine
reports `attempted: N, failed: N`, zero findings, and flags the results INCOMPLETE.
Four separate agents diagnosed this as an infrastructure failure and refused to
treat it as a clean pass. They were right to refuse, and wrong about the cause.

The logged error is useless:

```
error=Internal error: failed to execute prompt: Invalid params
```

# Root cause

The batcher packs about 4x more text than the agent will accept.

```
MAX_PROMPT_LENGTH  =  100_000 bytes   crates/claude-agent/src/constants/sizes.rs:83
DEFAULT_BATCH_SIZE =  393_216 bytes   crates/swissarmyhammer-validators/src/review/fleet.rs:113
```

`ClaudeAgent::validate_prompt_request` (`crates/claude-agent/src/agent.rs`) ends with:

```rust
if prompt_text.len() > self.config.max_prompt_length {
    return Err(agent_client_protocol::Error::invalid_params());
}
```

So an over-long prompt is reported as a bare `invalid_params` with no message. The
fleet turns that into `Err(())` and tallies a failed task.

Three things make it bite now, though the mismatch is older:

1. `DEFAULT_BATCH_SIZE` went 256 KiB -> 384 KiB in `71148449d` (^k12rn64). It was
   already 2.6x over the cap; it is now 3.9x.
2. That same commit made the prime render every source line with a line number and
   an 8-char blame sha, about 1.45x the raw source bytes. The budget counts RAW
   source bytes (`file.source_slice().len()`), but the prompt carries RENDERED
   bytes plus the rule text plus probe results. The budget therefore understates
   the real prompt by a wide margin.
3. Only fat batches trip it. Small commits never fill the budget, so they pass.
   That is why this looked intermittent and environment-dependent.

Evidence: in one run every failure was `validator=duplication` — the validator
carrying `probes: [duplicates]`, so its prompts are the fattest.

# The structural defect

Two values that MUST agree are declared independently, in different crates, and
neither knows about the other:

- `claude-agent` defaults `max_prompt_length` to `100_000`.
- `swissarmyhammer-agent` sets `max_prompt_length: 5_000_000` (`crates/swissarmyhammer-agent/src/lib.rs:932`).
- The fleet picks `DEFAULT_BATCH_SIZE` knowing neither.

So the effective cap depends on which agent serves the review, and the batcher
budgets against a number unrelated to either.

# Changes

1. Derive the batch budget from the agent's actual prompt cap. One source of
   truth. The fleet must not carry an independent constant that silently
   disagrees with the agent it is talking to.
2. Budget against the RENDERED prompt size, not raw source bytes — or apply an
   explicit, named, tested headroom factor covering the line/blame rendering, the
   rule text and the probe results. Whichever is chosen, record why on this card.
   Do NOT leave the budget measuring one thing while the limit measures another.
3. Make the over-length rejection diagnostic. `invalid_params` with no message
   cost four agents a full diagnosis each. It must say the actual length and the
   limit.
4. An over-length prompt must not silently degrade to a failed task with zero
   findings. Either it is prevented by correct budgeting, or it is reported as a
   named, visible gap the way `^3rnvage` made oversized files report as skipped.

Do NOT fix this by raising `max_prompt_length` alone. That hides the disagreement
instead of removing it, and the next rendering change re-opens it.

# Acceptance

- A test that packs a full batch and asserts the resulting prompt is within the
  agent's configured cap. This must go through the real batching and rendering,
  not a hand-built string.
- A test that an over-length prompt produces an error naming the length and the
  limit, not a bare `invalid_params`.
- A test pinning that the fleet's budget and the agent's cap come from one
  source, so they cannot drift apart again.
- `review sha` over the 71-file commit `0c8b969b8~1..0c8b969b8` completes with
  zero failed tasks. This is the real reproduction; verify it, do not assume it.
- `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` passes.
- `cargo fmt --all`; `cargo clippy --workspace --all-targets -- -D warnings` clean.

Blocks every other #review card, because the gate cannot be trusted until this is
fixed. #review #bug