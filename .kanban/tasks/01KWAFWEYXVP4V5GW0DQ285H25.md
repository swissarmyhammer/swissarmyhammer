---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kwahfj3samvvxmybth2m2qz8
  text: |-
    Implemented. Replaced the one-shot `rescan_for_completeness` (fleet.rs) with `sweep_until_dry`: a bounded loop that drives the SAME accumulating session forward — each sweep `submit_forked`s the session that delivered the PRIOR sweep's answer (`session = turn.session_id` at the end of each iteration), never re-forking the first pass. Termination: empty follow-up turn (logged "went dry") OR the named cap `MAX_FOLLOWUP_SWEEPS = 4` (logged "hit the runaway cap"). Fork/parse failure ends the loop and keeps gathered findings; empty first pass spends zero sweeps. Renamed `RESCAN_PROMPT` -> `FOLLOWUP_PROMPT` and reworded its body to "report any ADDITIONAL violations ... you have NOT already named / not-already-listed findings"; kept the `## Completeness re-scan` header marker (logs + tests key on it) and did NOT touch OUTPUT_CONTRACT/whole-file scope or the verify stage. All-rules nudge (start-simple).

    Test-support: added `ScriptedReply::Sequence` (yields successive deltas per match of one needle, sticks on last when drained — the only way to script convergence when the loop sends a constant prompt) + a `session_history` accessor.

    Four new deterministic unit tests in review::fleet replace `completeness_rescan_fires_once...`:
    (a) `followup_sweep_continues_while_findings_arrive_and_stops_when_dry`
    (b) `followup_sweep_stops_at_the_cap_when_never_dry` (asserts exactly MAX_FOLLOWUP_SWEEPS sweeps)
    (c) `followup_sweep_drives_the_session_forward_not_reforking_the_first_pass` — load-bearing: asserts the 2nd sweep's accumulated context holds the nudge TWICE (forward chain), not once (re-fork). RED-verified: reverting `session = turn.session_id` to re-fork-parent made this fail `left: 1, right: 2`.
    (d) `empty_first_pass_spends_no_followup_sweeps`

    Verification: `cargo nextest run -p swissarmyhammer-validators` = 293 passed, 0 failed. `cargo clippy -p swissarmyhammer-validators --all-targets` = clean (exit 0, no warnings). swissarmyhammer-tools test build (test-support consumer) compiles.

    Decision on the "Ideally" real-model e2e: NOT added. `review_real_model_e2e.rs` runs a 0.6B qwen that is DESIGNED non-deterministic and asserts structure only (it documents that the model "frequently answers with unparseable output"); a multi-instance content-count assertion there would be flaky. The structural loop is fully and deterministically proven by the four unit tests (incl. the RED-verified forward-drive proof), so I declined to regress that test's reliability.
  timestamp: 2026-06-29T20:35:08.025068+00:00
- actor: claude-code
  id: 01kwahp7wpq21ez54m9hmmsw9p
  text: 'Orchestrator: iteration-1 implement landed green in doing. fleet.rs: replaced one-shot rescan_for_completeness with `sweep_until_dry` — forward-driving loop (session = turn.session_id each iter, never re-forks first pass), bounded 1..=MAX_FOLLOWUP_SWEEPS (=4), stops on [] (dry) or cap, degrades-not-loses on fork/parse failure, zero turns on empty first pass. RESCAN_PROMPT→FOLLOWUP_PROMPT reworded "additional, not already listed" (all-rules nudge). test_support.rs: added ScriptedReply::Sequence + session_history. 4 new tests incl. RED-verified convergence test (forward-driving ≠ re-fork). nextest -p swissarmyhammer-validators 293 passed/0 failed, clippy clean, double-check PASS. Real-model e2e deliberately skipped (0.6B qwen non-deterministic → content-count assert would be flaky; structural loop proven by deterministic unit tests). Proceeding to checkpoint commit, then review.'
  timestamp: 2026-06-29T20:38:46.934969+00:00
position_column: doing
position_ordinal: '8280'
project: local-review
title: 'Review fan-out: drive the validator session forward with follow-up "any more?" prompts until dry (replace the one-shot completeness re-scan)'
---
## Problem

Per-pass **recall** is low: a single review pass under-reports instances of a rule, so the full set of findings only emerges across many `/finish` implement→test→review rounds. Real example (task ^5yk6jmm): a 33 KB / 808-line file (`write/mod.rs`) took **6 review rounds** to converge — findings came out 8 → 5 → 7 → 8 → 1 → 0. Every finding was legitimate and almost all were pre-existing; nothing was a regression. The cost was purely that each round surfaced a *subset*, and each outer round is a ~45-minute compile/test cycle.

Root cause is named in the code itself (`fleet.rs` RESCAN_PROMPT comment): *"Small models under-report pre-existing instances of a rule on the first pass even with the whole-file contract — they anchor on the salient match."* The review model is local (qwen). The prompt language is already as strong as language gets — `OUTPUT_CONTRACT` (fleet.rs:1043) explicitly says "report every occurrence… list them ALL… one finding per file:line… including pre-existing instances." Six rounds proved that more words don't beat anchoring. The fix is **structural**, not more admonishment text.

## What exists today

`crates/swissarmyhammer-validators/src/review/fleet.rs`:
- `handle_fork_success` (fleet.rs:648) parses a validator's first-pass findings, then calls
- `rescan_for_completeness` (fleet.rs:746) — which does the right KIND of thing but is **capped at exactly one extra fork turn**: it `submit_forked(parent_session, RESCAN_PROMPT)` once, parses, merges, done. Comment: *"Capped at exactly one extra pass — this never recurses on its own result."*
- `RESCAN_PROMPT` (fleet.rs:1067) is the "scan the SAME files again for any further instance you missed" nudge.

One sweep recovers *some* misses, not all → residual instances spill into the next outer round.

## Decision / reframe

Stop thinking of this as a separate one-shot "completeness re-scan." Model it as **one validator review session driven forward**: the file is decoded once; the model produces first-pass findings; then we **tack on a follow-up prompt** — "you've listed these; report any ADDITIONAL violations of the same rules you have NOT already named" — and **repeat that nudge on the same accumulating session until a turn returns `[]` (dry) or we hit a hard cap N** (~3–4). It is conversational continuation with a repeated "anything else?", terminating when the model itself says none. Reword/rename away from the `rescan_for_completeness` one-shot metaphor toward a "follow-up-until-dry" loop (e.g. `sweep_until_dry` / `drive_findings_to_exhaustion` — namer's choice).

## Correctness detail (this is the load-bearing part)

**Each follow-up turn must continue the session that already contains the PRIOR follow-up answers — not re-fork the first-pass session every time.**

Today's call forks `parent_session` (the first pass). If the loop re-forks `parent_session` each iteration, every nudge only sees the first-pass findings, so iteration 3 will re-report what iteration 2 already found → it never converges / oscillates. To make "additional, not already listed" well-defined and make the loop actually go dry, drive the session forward: iteration K+1 continues the session returned by iteration K (`turn.session_id`), so the model's own accumulated answers are in context and "additional" means additional-to-everything-said-so-far.

## Stop condition + safety

- **Terminate** when a follow-up turn parses to an empty array (the model is the authority on "found them all"), OR at a hard cap N forks (runaway backstop). Log which.
- A follow-up that fork-fails / errors / doesn't parse ends the loop and keeps whatever was gathered (never loses the first-pass findings) — same degrade-not-fail discipline as today.
- Downstream `dedup_exact` (synthesize) already collapses exact repeats, so a model that re-lists something is harmless, not a convergence breaker.
- Empty first pass → no follow-ups at all (unchanged; don't spend turns on a clean validator).
- Cost is bounded and cheap: same warm session, file decoded once, each turn generates only a short delta.

## Open choice (decide during impl; start simple)

The follow-up nudge can be **all-rules** ("any rule you under-reported, more?") or **per-rule** ("for rule X specifically, enumerate every remaining file:line"). Per-rule walks each rule to exhaustion (higher recall, more turns); all-rules is cheaper. **Start all-rules**; only escalate to per-rule if recall still lags in the real-model e2e. Optionally also add a first-pass self-count checkpoint ("for each rule that matched, state how many occurrences exist and confirm you listed that many") — cheap anti-anchoring forcing function.

## Scope / non-goals

- Do NOT weaken or rewrite the existing strong `OUTPUT_CONTRACT` / whole-file-scope language — keep it; this change is the structural loop underneath it.
- Do NOT touch the verify/refutation stage (that's precision, not recall — and it was not the bottleneck here; confirmed findings were all real).
- Backend-agnostic: must work on both the warm-KV (qwen) fork path and the claude prompt-cache path, and degrade correctly when forking is unavailable (monolithic fallback already has no re-scan; keep that, or apply the loop there too if cheap — namer's call, but don't regress the fallback).

## Acceptance criteria

- `rescan_for_completeness` is replaced by a bounded forward-driving loop that continues the accumulating session (drives `turn.session_id` forward), not a single re-fork of the first pass.
- The loop terminates on an empty follow-up turn or a configurable hard cap N; both paths logged; cap has a named constant.
- First-pass-empty still spends zero follow-up turns; fork/parse failure ends the loop without losing gathered findings.
- A unit test (using the existing `ScriptedAgent` in `review::test_support`, which already scripts re-scan turns keyed on the `## Completeness re-scan` needle) proves: (a) the loop keeps going while turns return findings and stops on `[]`; (b) it stops at the cap if never dry; (c) later turns see earlier follow-up answers (no re-report / it converges); (d) empty first pass → zero follow-ups.
- `cargo nextest run -p swissarmyhammer-validators` green; `cargo clippy` zero warnings.
- Ideally: extend the real-model e2e (`crates/swissarmyhammer-agent/tests/review_real_model_e2e.rs` or `review_e2e.rs`) to assert a single pass now reports multiple instances of one rule that previously needed several rounds.

## Evidence / reference

The 6-round convergence trace lives on task ^5yk6jmm's comment thread (file-edit-tools project) — concrete data on which classes spilled across which rounds, useful for a regression fixture.