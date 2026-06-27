---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw372m9g1by0205zajb6jc5z
  text: 'Picked up. Researched: OUTPUT_CONTRACT lives in crates/swissarmyhammer-validators/src/review/fleet.rs (shared by every fan-out prompt via render_validator_suffix and the monolithic render_fleet_prompt). Synthesis dedup is dedup_exact() in review/synthesize.rs, keyed on (file,line,validator,rule,claim) — so multiple lines for one rule already survive (different line => different key) and synthesize() renders each as a flat `- [ ] file:line` item. Plan: (1) RED test that OUTPUT_CONTRACT demands reporting every occurrence/no bail-fast; add the language. (2) Engine/guard test in synthesize.rs: one rule firing on multiple lines yields multiple rendered items, not collapsed.'
  timestamp: 2026-06-27T00:18:37.488298+00:00
- actor: claude-code
  id: 01kw37cwp0bqf8m8scg5eqbdq9
  text: |-
    Done (left in doing for review). TDD:

    (1) fleet.rs — added a paragraph to the private const OUTPUT_CONTRACT (shared verbatim by every fan-out prompt): "Report every occurrence of every rule that fires, in this single pass: when a rule matches on several lines, emit a separate finding for each match — one finding per `file:line`. Do not stop at the first match and do not collapse repeated matches into one finding; list them all..." RED test `output_contract_demands_every_occurrence_with_no_bail_fast` failed first (assertion on "every occurrence of every rule"), then GREEN after the edit.

    (2) synthesize.rs — added engine guard test `one_rule_matching_multiple_lines_renders_every_instance`: one rule (dead-code/no-unused) firing at lines 12/34/56 of one file yields 3 rendered `- [ ] file:line` items, counts.findings==3, not collapsed. dedup_exact keys on (file,line,validator,rule,claim) so different lines survive; the test guards against a future collapse-by-rule regression. (Honest note: this is a regression guard for already-correct behavior, so it passed on first run rather than failing-first.)

    Test output (real):
    - `cargo nextest run -p swissarmyhammer-validators` => 316 tests run: 316 passed, 0 skipped.
    - `cargo clippy -p swissarmyhammer-validators --all-targets -- -D warnings` => clean, Finished.
    - `cargo fmt -p swissarmyhammer-validators` => only my two files touched.

    No public surface changed (OUTPUT_CONTRACT is a private const; no signatures changed) so rdeps run not required; confirmed no downstream crate snapshots the contract text. Did NOT touch the unrelated parallel changes (Cargo.toml/Cargo.lock, crates/heb/* deletions, swissarmyhammer-kanban files). Did NOT re-add Finding.severity. Adversarial double-check verdict: PASS.
  timestamp: 2026-06-27T00:24:13.760647+00:00
- actor: claude-code
  id: 01kw3ctmghr6707jsrrtn2qyah
  text: 'Implemented (orchestrator finished after the implementer hit the session limit mid-run, leaving complete-but-unverified edits). OUTPUT_CONTRACT in fleet.rs now demands reporting every occurrence of every rule that fires, one finding per file:line, with explicit "do not stop at the first match / do not collapse repeated matches." Two tests added: fleet.rs `output_contract_demands_every_occurrence_with_no_bail_fast` (contract text), and synthesize.rs proving 3 same-rule instances at different lines all render and counts.findings==3 (not collapsed). Verified: cargo nextest -p swissarmyhammer-validators 316 passed/0 failed (+2); cargo fmt clean; cargo clippy --all-targets -D warnings clean. Built on z3ax4xt (eccd288dc); no severity reintroduced.'
  timestamp: 2026-06-27T01:59:07.025881+00:00
depends_on:
- 01KW2XZS1ZK47Q888HPZ3AX4XT
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffe580
project: local-review
title: 'Review: contract reports every rule instance, no bail-fast'
---
Like a test report listing every failing assertion: when a rule fires N times in a file, emit N findings (one per `file:line`) in a single pass. Never stop at the first instance.

## Why
Bail-fast = find-one → fix → re-review → find-next, round after round (the re-review token storm). Reporting all instances at once lets the implementer fix the root cause across the whole file in one go.

## Work
- `crates/swissarmyhammer-validators/src/review/fleet.rs` fan-out prompt: state explicitly "report EVERY occurrence of every rule that fires; do not stop at the first." (The model already emits a findings array and synthesis dedups by `file:line`, not by rule — so multiple instances already survive; this makes it intentional and prompt-guaranteed.)
- Add an engine test: a fixture where one rule matches multiple lines yields multiple findings, all rendered.

Depends on the severity-removal card (shared contract/prompt edits).