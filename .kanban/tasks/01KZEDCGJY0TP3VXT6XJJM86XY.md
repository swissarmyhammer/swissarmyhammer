---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzes1bjxanngak0yz46e6hnh
  text: |-
    Picked up; research done.

    Engine map:
    - `crates/swissarmyhammer-validators/src/review/fleet.rs` funnels every reply through `parse_task_response`, which calls `review/types.rs::parse_findings` and, on error, logs and returns `Err(())`. Three call sites: `handle_fork_success` (warm forked path), `collect_task` (monolithic path), and `sweep_until_dry` (follow-up loop).
    - `collect_fan_out` turns any `Err(())` into `failed += 1`, and `synthesize` renders the INCOMPLETE banner from that tally. That behaviour stays.
    - The `Err(())` today conflates "no reply delivered" (pool error, dropped channel) with "reply did not parse". A re-ask only makes sense for the second, so the collectors need to name which failure happened.

    Repair rung: `types.rs::extract_json_value` already strips prose before `[` and after `]` when nothing balances the first `[` (its step 4). What it cannot do is skip a bracketed aside in the prose: step 3 takes the FIRST `[` and the delimiter that balances it, so a reply like "I reviewed [src/lib.rs]:\n[{...}]" hands the parser `[src/lib.rs]`. The repair therefore walks the later `[` candidates too, each to the last `]`.

    Re-ask rung: the forked path can re-ask cheaply by forking the session that produced the bad reply. The monolithic path has no session id on `PromptResult`, so its re-ask re-renders and re-submits the same prompt on a fresh session.

    Test harness: the drive acceptance tests run with `ForkMode::Unsupported`, so they exercise the monolithic path. `ScriptedReply::Sequence` shares one queue across the agent, which scripts first-malformed / second-valid exactly.
  timestamp: 2026-08-07T18:51:14.397181+00:00
- actor: claude-code
  id: 01kzesx3az46jeywvfyb2cntrz
  text: |-
    Implementation landed. The ladder is repair → re-ask → fail, in that order.

    Rung 1 — repair (`review/types.rs`): `parse_findings_repaired` runs the strict `parse_findings` first and, only on error, strips the text before an `[` and after the last `]` and deserializes what is left. It walks the later `[` candidates too (capped by `MAX_REPAIR_ARRAY_STARTS`), because `extract_json_value` balances the FIRST `[`, which a bracketed aside in the prose captures. A repaired parse is a plain success and spends no re-ask. The repair reports the strict parse's own error when no candidate span reads.

    Rung 2 — re-ask (`review/fleet.rs`): the collectors now name their failure (`TaskFailure::NotDelivered` vs `Unparseable`), so a re-ask is spent only on a reply that arrived and could not be read. A pool error or a dropped channel fails at once, as before.
    - Forked path: `reask_forked_task` forks the session that produced the bad reply and sends `REASK_PROMPT` ("reply again with the SAME findings as a single JSON array"). The warm rung — the findings are already in context. The sweep then drives whichever session answered.
    - Monolithic path: `PromptResult` carries no session id, so `collect_monolithic_task` re-renders and re-submits the same prompt on a fresh session. This also covers the fork-failed fallback.

    Rung 3 — fail: only a second unreadable reply marks the task failed, so the INCOMPLETE banner behaviour is untouched.

    Supporting refactor, needed to carry the re-render material into the collectors: a `TaskContext` struct replaces the change_purpose/validator/ruleset/files parameter quartet (`handle_fork_failed` was already at 7 arguments), and the two duplicated failure log messages became `TASK_ERRORED` / `TASK_DROPPED`.

    Not changed: the follow-up sweep still ends its loop on an unreadable reply rather than re-asking — a sweep reply that cannot be read loses nothing already gathered, so there is no task to save.

    Verification: RED first for all five behaviour tests. The two drive tests failed on "asked once, not twice"; the repair test failed with the report reading `1/1 review tasks failed — results are INCOMPLETE`. The two fleet tests were written after the code, so their RED was proved by removing the re-ask call and watching both drop to 0 re-asks. `cargo test -p swissarmyhammer-validators` 515 passed / 0 failed, `cargo clippy --all-targets -D warnings` clean, `cargo test -p swissarmyhammer-tools --lib -- mcp::tools::review` 69 passed. No new rustdoc warning.

    ARCHITECTURE.md needs no update: the change adds no module, crate, or dependency edge, and stays inside `swissarmyhammer-validators::review`.
  timestamp: 2026-08-07T19:06:23.455298+00:00
- actor: claude-code
  id: 01kzesxam8r19wv50xakm3102w
  text: |-
    ### implement — changed
    - evidence: 5 files — crates/swissarmyhammer-validators/src/review/types.rs, review/fleet.rs, review/fleet/tests.rs, review/drive.rs, review/test_support.rs. `cargo test -p swissarmyhammer-validators`: 515 passed, 0 failed. `cargo clippy -p swissarmyhammer-validators --all-targets -- -D warnings`: clean.
    - next: /review
  timestamp: 2026-08-07T19:06:30.920665+00:00
- actor: claude-code
  id: 01kzew2pz652ex4gegyaht4sdh
  text: |
    ### review — findings
    - scope: `review sha 814d6f8ea..773fe1d4c` (HEAD~1..HEAD), driven over a fresh `sah serve` stdio process
    - engine counts: 4 findings / 4 confirmed / 14 refuted / 26 attempted / 0 failed / 1 skipped file
    - recorded: 1 finding — crates/swissarmyhammer-validators/src/review/types.rs:221
    - dropped as out of scope (git blame shows the commit did not introduce the code, and the skill's existing-test exception covers it):
      - drive.rs:1152 and drive.rs:1231 — magic literal `6` in `#[cfg(test)] mod tests` (module starts line 423); blame 945a7583ff (2026-06-09) and fcf1674b0a (2026-08-01). The commit's only drive.rs hunks are old line 443 and an insert at new line 622-842.
      - fleet/tests.rs:1 — over prompt cap, remedy is to split an existing test file. The file was 118353 bytes before the commit and 122169 after; the rendered overage is 25802 bytes, far larger than the commit's 3816-byte addition, so the cap was already exceeded at the parent commit.
    - next: extract each extraction strategy in `extract_json_value` into a private helper to bring cognitive complexity under 15
  timestamp: 2026-08-07T19:44:24.550678+00:00
- actor: claude-code
  id: 01kzew6rqct7hb8q0p8dngqk9n
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 5 files; a three-rung ladder on an unreadable reply (cheap repair, one re-ask, then fail), a named TaskFailure, five behaviour tests all RED first
    - test: green — cargo nextest run --workspace 13773 passed, doc tests 0 failed, fmt clean, clippy clean
    - commit: 773fe1d4c (parent 814d6f8ea)
    - review: findings — crates/swissarmyhammer-validators/src/review/types.rs:221 (extract_json_value cognitive complexity 20 against the limit of 15)
    - Coverage gap reported by the reviewer: crates/swissarmyhammer-validators/src/review/fleet/tests.rs is over the rendered prompt cap, so the duplication validator did not review that one file. The overage predates this commit — the file was already 118353 bytes at the parent, and the overage is about seven times this commit's addition.
  timestamp: 2026-08-07T19:46:37.420385+00:00
- actor: claude-code
  id: 01kzewp4tw17rrykm8z1hep7ze
  text: |-
    Finding worked: `extract_json_value` cognitive complexity 20 against the gate of 15.

    Measured with the real scorer, never by eye. A throwaway example (`crates/swissarmyhammer-sem/examples/complexity_report.rs`) called `swissarmyhammer_sem::parser::plugins::code::cognitive_complexity` on the file and printed `cognitive_score` / `max_nesting_depth` for every function against `COGNITIVE_COMPLEXITY_THRESHOLD` (15) and `NESTING_DEPTH_THRESHOLD` (4). The example was deleted after the last measurement.

    Whole-file sweep, not only the cited line. Before: `extract_json_value` score 20 / nesting 3 was the ONLY function over either gate; the next highest was `matching_delimiter` at score 10 / nesting 3, under both. So the file needed exactly the one fix.

    The fix, as the finding stated: each of the four extraction strategies is now a private helper, and `extract_json_value` is a flat `or_else` ladder over them.
    - `json_fenced_value` — a ```json fenced block.
    - `bare_fenced_value` — any bare ``` fenced block.
    - `balanced_value` — the first `open` to the `close` that balances it.
    - `spanning_value` — the first `open` to the last `close`, the last resort.
    - `delimited_value` — the acceptance test both fenced rungs share (the fence body must be exactly one `open`…`close` value). Extracted because the check repeated verbatim in the two fence rungs, which is duplication.

    Two named constants, `CODE_FENCE` and `JSON_CODE_FENCE`, replace the repeated fence literals and remove the magic `start + 3` the bare-fence rung carried.

    After: `extract_json_value` score 0 / nesting 0; every helper 0 or 1; the file's maximum is still `matching_delimiter` at score 10 / nesting 3. No function in the file is over either gate.

    Two characterization tests were added FIRST, and confirmed green on the unrefactored code, because the two lowest rungs had no test to hold them through the move:
    - `extract_json_value_spans_first_open_to_last_close_when_nothing_balances` — locks rung 4, which only fires when the first `open` never balances.
    - `extract_json_value_falls_back_to_the_trimmed_response` — locks the trimmed-input fallback.
    Rungs 1 to 3 were already covered by the `parse_findings_reads_*` tests. This is a behaviour-preserving refactor, so the harness is the existing green suite plus these two, not a new RED.

    Verification: `cargo test -p swissarmyhammer-validators` 517 passed / 0 failed (515 before, plus the two new tests), doc tests 2 passed. `cargo clippy -p swissarmyhammer-validators --all-targets -- -D warnings` clean. `cargo fmt --all -- --check` clean.

    `extract_json_value` keeps its exact `pub(crate) fn (&str, char, char) -> &str` signature, so no caller changed. The new helpers are private and single-call-site — the reuse rule states outright that a helper extracted to keep a function under the complexity limit is warranted with one caller and must never be flagged toward inlining.

    ARCHITECTURE.md needs no update: no module, crate, or dependency edge changed.
  timestamp: 2026-08-07T19:55:01.340164+00:00
- actor: claude-code
  id: 01kzewpb7zwsgh56xt1hny17bb
  text: |-
    ### implement — changed
    - evidence: 1 file — crates/swissarmyhammer-validators/src/review/types.rs. `extract_json_value` cognitive complexity 20 -> 0 by the real `swissarmyhammer_sem` scorer; no function in the file is over the complexity gate (15) or the nesting gate (4). `cargo test -p swissarmyhammer-validators`: 517 passed, 0 failed, doc tests 2 passed. `cargo clippy -p swissarmyhammer-validators --all-targets -- -D warnings`: clean. `cargo fmt --all -- --check`: clean.
    - next: /review
  timestamp: 2026-08-07T19:55:07.903078+00:00
- actor: claude-code
  id: 01kzexx51m4brxk0jwj0ddftre
  text: |
    ### review — findings
    - scope: `review sha 773fe1d4c..0e2946d2d` (HEAD~1..HEAD), driven through a fresh `sah serve` process because the session MCP image has an old probe catalog
    - prior finding verified closed: `extract_json_value` is now a flat four-rung `or_else` ladder (`json_fenced_value`, `bare_fenced_value`, `balanced_value`, `spanning_value`); the complexity cause is removed from the whole function
    - counts: 1 finding, 1 confirmed, 0 refuted, 9 attempted, 0 failed, 0 skipped
    - evidence: crates/swissarmyhammer-validators/src/review/types.rs:250
    - scope check: `git blame` puts line 250 in commit 0e2946d2d, so the finding is on new code; it asks for a NEW test, which the existing-test exception does not cover
    - next: add the uppercase language identifier test, then re-run review
  timestamp: 2026-08-07T20:16:19.508535+00:00
- actor: claude-code
  id: 01kzexydfkvp5y16x2hz4t051s
  text: |-
    ### finish iteration 2 — findings
    - implement: changed — crates/swissarmyhammer-validators/src/review/types.rs; the prior finding is closed and verified. extract_json_value is a flat ladder over five helpers, complexity 20 to 0 by the real scorer.
    - test: green — cargo nextest run --workspace 13775 passed, doc tests 0 failed, fmt clean, clippy clean
    - commit: 0e2946d2d (parent 773fe1d4c)
    - review: findings — crates/swissarmyhammer-validators/src/review/types.rs:250 (no test covers an uppercase ```JSON language identifier falling through to the bare-fence rung). git blame puts the line in this commit, so the existing-test exception does not release it.
    - guardrail: no repeat. Round 1 named the complexity, round 2 names a missing test on the new code.
  timestamp: 2026-08-07T20:17:00.915382+00:00
- actor: claude-code
  id: 01kzexz6gm4q9pe3dgv4y3ebn0
  text: |-
    ### finish iteration 3 — stuck (session limit, not a code problem)
    The session reached its subagent limit (200 of 200), so the finish loop cannot run another implement step. The card stays in `review` with 1 open finding:
    - crates/swissarmyhammer-validators/src/review/types.rs:250 — no test covers an uppercase ```JSON language identifier falling through to the bare-fence rung.

    The work itself is small and unblocked. A new session can continue with `/finish ^jjm86xy`. The last good commit is 0e2946d2d.
  timestamp: 2026-08-07T20:17:26.548609+00:00
position_column: review
position_ordinal: '8480'
title: 'review: retry a fleet reply that does not parse before failing the pair'
---
One malformed LLM reply invalidates a whole review. Seen 2026-08-07 in ../swissarmyhammer-main (`.sah/mcp.7341.log`, 15:16:31): the duplication validator's reply failed JSON parse ("expected `,` or `}` at line 7 column 361"), the engine yielded zero findings, marked the task failed, and the 6-minute review ended INCOMPLETE. The only recovery is a full re-run.

Requirements:
- When a fleet task reply does not parse into findings, re-ask that one task once before it is declared failed. Log the retry.
- Before the re-ask, try a cheap repair parse first (strip text before `[` and after `]`; a reply is a JSON array by contract). A repaired parse counts as success and needs no retry.
- Only a second parse failure marks the task failed. The INCOMPLETE banner behavior stays — honesty is correct.
- Test: a fleet task whose first reply is malformed and whose second reply is valid produces findings and a complete report; a task with two malformed replies fails the pair as today.

#tool-validators

## Review Findings (2026-08-07 14:28)

Scope: `814d6f8ea..773fe1d4c` (HEAD~1..HEAD).

- [x] `crates/swissarmyhammer-validators/src/review/types.rs:221` — Function `extract_json_value` exceeds cognitive complexity gate: complexity 20 is significantly over the limit of 15, making it difficult to verify and maintain. Extract each extraction strategy (```json fence, bare ``` fence, delimiter-matching, first-to-last fallback) into a separate private helper function that `extract_json_value` calls in sequence, reducing branching at the top level.

## Review Findings (2026-08-07 15:11)

Scope: `773fe1d4c..0e2946d2d` (HEAD~1..HEAD).

- [x] `crates/swissarmyhammer-validators/src/review/types.rs:250` — The new `json_fenced_value` function performs case-sensitive matching for the Markdown language identifier, searching for the exact string `"```json"`, but Markdown language identifiers are case-insensitive in practice and per CommonMark conventions. An LLM response using `"```JSON"` or `"```Json"` will fail to match this check and fall through to the generic fence handler, yet there is no test covering this non-canonical case to verify the fallback behavior. Add one test feeding an uppercase language identifier (e.g., `"```JSON\n[...]\n```"` or `"```Json\n[...]\n```"`) through `parse_findings` to verify the fallback to `bare_fenced_value` correctly handles case-insensitive identifiers.
