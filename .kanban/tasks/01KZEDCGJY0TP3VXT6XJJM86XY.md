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
position_column: doing
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