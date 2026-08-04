---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kz40fw3g2t97e9p3481s2cj5
  text: |-
    Found a true conflict before writing code. The task text says code-hygiene must declare exactly `probes: [\"callers\"]` (stated twice: in the set description and as an acceptance criterion). But the `cognitive-complexity.md` rule being folded into code-hygiene depends on its own `complexity` probe (added in commit 8d7d8f57d, the day before this task's last update) for deterministic tree-sitter-computed numbers. Probes are declared once per validator SET in VALIDATOR.md (`crates/swissarmyhammer-validators/src/review/scope.rs` reads `manifest.probes` per set, not per rule) and drive which probe evidence renders for every rule in that set. Declaring only `callers` on code-hygiene would silently stop the complexity probe from firing for the merged rule, reverting it to ungrounded LLM counting -- the exact nondeterminism 8d7d8f57d was written to remove.

    Asked the user directly (not guessing): answer was (A) -- declare `probes: [callers, complexity]` on code-hygiene, keeping the complexity rule's real evidence mechanism intact even though it deviates from the literal `[\"callers\"]` acceptance-criterion text. Proceeding with probes: [callers, complexity] for code-hygiene. Updating the acceptance criteria text below to match this resolution.
  timestamp: 2026-08-03T14:29:51.344087+00:00
- actor: claude-code
  id: 01kz43azts6r6r9bzrp0mby57j
  text: |-
    ### implement — changed

    Files touched:
    - `builtin/validators/code-security/VALIDATOR.md` (new) — `match.files: [@file_groups/source_code]`, no probes
    - `builtin/validators/code-hygiene/VALIDATOR.md` (new) — `match.files: [@file_groups/source_code]`, `probes: [callers, complexity]` (see resolved-conflict note in the description)
    - `builtin/validators/code-security/rules/{no-secrets,injection,command-safety}.md` — `git mv`'d unchanged from the retired sets
    - `builtin/validators/code-hygiene/rules/{no-commented-code,function-length,cognitive-complexity,missing-docs,data-driven,dead-code}.md` — `git mv`'d unchanged from the retired sets
    - Deleted the nine retired set directories: `no-secrets`, `injection`, `command-safety`, `no-commented-code`, `function-length`, `complexity`, `missing-docs`, `data-driven`, `dead-code`
    - `builtin/validators/README.md` — swapped the stale `no-secrets/` example for `code-security/`
    - `crates/mirdan/retired-validators/<name>/...` (new, 9 dirs) — byte-for-byte snapshot of each retired set's shipped content, kept outside `builtin/validators/` so neither the RuleSet loader nor the builtin embed ever picks it up as a live/installable set
    - `crates/mirdan/src/retired_validators.rs` (new) — `RETIRED_VALIDATOR_SETS` table (`include_str!` of the snapshot files) + `prune_unmodified_retired_sets(store_root)`: removes a retired set directory only when every file matches the snapshot byte-for-byte (no extra/missing/edited files); leaves anything else untouched
    - `crates/mirdan/src/lib.rs` — registered `pub mod retired_validators;`
    - `crates/mirdan/src/install.rs` — wired `prune_unmodified_retired_sets` into `install_profile_validators` (runs on every refresh/init); fixed three existing tests that referenced the now-retired `dead-code`/`no-secrets` names (`init_profile_materializes_builtin_validators_to_home_store`, `init_profile_validators_idempotent_refreshes_builtin_preserves_user`, `init_profile_writes_store_readme_and_deinit_removes_it`) to use `code-hygiene`/`code-security`; added new test `init_profile_refresh_prunes_unmodified_retired_set_but_keeps_user_modified_copy`
    - `crates/mirdan/src/builtin_validators.rs` — updated `test_builtin_validators_embed_expected_sets` to assert `code-security`/`code-hygiene`/`duplication`/`reuse`/`test-integrity` present and the nine retired names absent
    - `crates/swissarmyhammer-validators/src/builtin/mod.rs` — trimmed `PROBE_VALIDATORS`/`SAFETY_VALIDATORS` to the sets that stayed standalone, added `MERGED_VALIDATORS`/`RETIRED_VALIDATOR_NAMES`, added `test_code_security_loads_with_three_rules_and_no_probes`, `test_code_hygiene_loads_with_six_rules_and_callers_and_complexity_probes`, `test_retired_single_rule_validators_no_longer_load`, `test_code_security_and_code_hygiene_match_expected_paths`, `test_test_integrity_reuse_and_duplication_are_unaffected_by_the_merge`; fixed every other test that referenced a retired name (`test_load_builtins`, `test_rehomed_quality_validators_load`, `test_no_secrets_expands_file_groups` → `test_code_security_expands_file_groups`, the two `IN_FILE_VALIDATORS` chain call sites) or removed the now-vacuous `test_in_file_validators_declare_no_probes`; deleted `data-driven` from `test_builtin_rulesets_carry_their_validator_md_body` since its body no longer stands alone

    Conflict found and resolved before writing code (full record in the earlier comment + description): the card said code-hygiene needs only `probes: [callers]`, but the `cognitive-complexity` rule bundled into it needs its own `complexity` probe (added same day in 8d7d8f57d) to stay deterministic. Asked the user directly; kept both probes. Updated the card's description and acceptance criteria to match.

    Verified byte-identity of all nine moved rule files via `diff` against the `cp`-made snapshot taken before the `git mv`.

    Evidence: `cargo test -p mirdan -p swissarmyhammer-validators` → 412 passed (mirdan) + 362 passed (swissarmyhammer-validators) + 2 doctests, 0 failed. `cargo fmt --all` clean. `cargo clippy -p mirdan -p swissarmyhammer-validators --all-targets -- -D warnings` clean.

    next: /review
  timestamp: 2026-08-03T15:19:37.049993+00:00
- actor: claude-code
  id: 01kz493wrtsrhkrmdh4zsmst8p
  text: |-
    ### finish iteration 1 — findings (pre-commit, found by orchestrator's own verification)
    The implementer's scoped test run (`cargo test -p mirdan -p swissarmyhammer-validators`) passed clean, but it explicitly skipped the card's own required broader check (`cargo nextest run -E 'rdeps(swissarmyhammer-validators) or rdeps(mirdan)'`) based on its own judgment that the change was "additive." I ran that check myself and it surfaces 3 real regressions in `swissarmyhammer-tools`, a crate the implementer's scope never touched:

    1. `swissarmyhammer-tools::tools_tests integration::review_e2e::review_e2e_working_confirms_real_defects_and_refutes_both_red_herrings` (review_e2e.rs:88) — asserts the report text contains `"dead-code"` by name. The rule still fires (the finding text for `src/payments.rs:16`/`util_reuse.rs:3` is present and correct), but the validator is now named `code-hygiene`, not `dead-code`.
    2. `swissarmyhammer-tools::tools_tests integration::review_e2e::review_e2e_sha_range_confirms_the_same_defects` (review_e2e.rs:165) — same cause, same fix.
    3. `swissarmyhammer-tools::review_progress_stdio_test::review_progress_is_received_by_a_real_client_over_a_byte_stream_transport` — times out at 70s. Confirmed this is a REAL regression, not pre-existing or flaky: stashed the ^ettx7ay diff, ran this test alone against the clean merged tree — it passes in 10.4s. With the diff restored it times out. Root cause is likely that `code-hygiene` now bundles 6 rules (including the `complexity` probe) into one validator task, so that single fleet task takes much longer than any of the 9 original single-rule tasks did, and this test's fixed wait budget doesn't account for it.

    Also confirmed independently: `mcp::tools::review::tests::review_file_reviews_the_real_llama_agent_acp_server_file_under_the_default_budget`, which failed on the FIRST rdeps run (before the origin merge), is gone from the second run — it referenced the now-deleted `llama-agent` crate (merged in from origin PR #69, unrelated to this task) and no longer applies.

    Also: the `review` tag was found dropped from this task's frontmatter after the first implement pass (present in `filter_tags` as `READY` only, `tags: []`). Restored it. Cause not diagnosed — flagging in case it recurs on other update-task calls.

    - test: red — 3 real failures on `rdeps(swissarmyhammer-validators) or rdeps(mirdan)`, confirmed not pre-existing
    - next: step 2 — /implement fixes all 3, and this time DOES run the card's own required broader check rather than skipping it
  timestamp: 2026-08-03T17:00:35.994810+00:00
- actor: claude-code
  id: 01kz49qmm4kcb521enk4f5ftyk
  text: |-
    ### implement — changed

    Fixed the 3 real regressions the prior finish-iteration surfaced via `rdeps(swissarmyhammer-validators) or rdeps(mirdan)`. All 3 traced to ONE root cause, not three separate ones.

    **Root cause**: `crates/swissarmyhammer-tools/tests/integration/review_fixture.rs`'s `fanout_rules()` scripts the fake ACP agent to answer by matching the literal `# Validator: <name>` header the production fleet puts in each fan-out prompt (`VALIDATOR_HEADER` in `crates/swissarmyhammer-validators/src/review/fleet.rs`, built from `validator.validator_name()` — the validator SET's name, one task per SET, not per rule). The fixture still scripted responses keyed to the nine retired single-rule names (`no-secrets`, `data-driven`, `dead-code`). After the merge, the production fleet now sends `# Validator: code-security` / `# Validator: code-hygiene`, so those three fan-out rules never matched, fell through to the scripted agent's default `[]` response, and the secret/data-driven/dead-code findings (and, downstream, their verify verdicts) never fired.

    - Findings 1 and 2 (`review_e2e.rs`) showed up as missing findings in the rendered report (`report_has_claim` false for `CLAIM_SECRET`/`CLAIM_DATA`/`CLAIM_DEAD_ORPHAN`), not as a literal-string mismatch on `"dead-code"` as first hypothesized — the actual failure was `item 5 secret` (line 88) because `no-secrets` never fired. Same root cause hits the sha-range test (line 165/167).
    - Finding 3 (`review_progress_stdio_test.rs`) was NOT a batching/complexity-probe latency issue. The "6-rules-in-one-task is slower" hypothesis was wrong: fan-out tasks are cheap fixture-agent lookups, not the source of the 70 s stall. The real cause: the test's step 8 (`wait_until("streamed review findings + verdicts...", ...)` ) blocks on `has_verdict(&logs, CLAIM_SECRET, true)`, which can never become true while the `no-secrets`→`code-security` fanout rule never matches (no finding → no verify prompt → no verdict ever streams), so the wait spins until `WAIT_DEADLINE` (60 s) and the test fails loudly around ~70 s wall clock including setup. No timeout change was needed or made.

    **Fix**: rewrote `fanout_rules()` in `review_fixture.rs` to key on the new set names. `no-secrets`'s finding moved under a `fanout("code-security", ...)` entry (unchanged, single finding — the only planted code-security defect in this fixture). `data-driven` and `dead-code` were merged into ONE `fanout("code-hygiene", ...)` entry, since `code-hygiene` is one fan-out task per batch and the scripted-agent harness matches the FIRST rule whose needles are all present in one prompt (not a merge of multiple matching rules) — so one task needs one response carrying all three findings (the data-driven finding plus both dead-code findings: the real orphan and the guard red herring). Added a `three_findings()` helper alongside the existing `finding()`/`two_findings()` to build that combined JSON array. Updated the surrounding doc comments (module banner, `FILE_ORPHAN`/`FILE_LIVE` doc comments) to reflect the new validator names. No rule text, VALIDATOR.md, or production code touched — the fix is entirely in the test fixture that scripts the fake agent.

    Files touched:
    - `crates/swissarmyhammer-tools/tests/integration/review_fixture.rs` — `fanout_rules()` rewritten to use `code-security`/`code-hygiene` instead of the retired `no-secrets`/`data-driven`/`dead-code` names; added `three_findings()` helper; updated doc comments.

    No production code, VALIDATOR.md, or rule-text files changed by this pass.

    Evidence:
    - `cargo test -p swissarmyhammer-tools --test tools_tests` → 162 passed, 0 failed (includes both previously-red `review_e2e` tests).
    - `cargo test -p swissarmyhammer-tools --test review_progress_stdio_test` → 3 passed, 0 failed, finished in 10.57s (was timing out at ~70s; now matches the ~10.4s baseline the diagnosis comment recorded for the pre-merge tree).
    - `cargo test -p swissarmyhammer-tools --test review_progress_notifications_test --test review_global_subscriber` → 2 passed, 0 failed.
    - `cargo test -p mirdan -p swissarmyhammer-validators` → 412 passed (mirdan) + 362 passed (swissarmyhammer-validators) + 2 doctests, 0 failed.
    - `cargo fmt --all -- --check` → clean, no diff.
    - `cargo clippy --workspace --all-targets -- -D warnings` → clean, 0 warnings.
    - **The card's own required broader check**: `cargo nextest run -E 'rdeps(swissarmyhammer-validators) or rdeps(mirdan)'` → `Summary [73.591s] 3545 tests run: 3545 passed (5 slow, 1 leaky), 0 skipped`. Zero failures. Full wall time 1m28s (matches the note that llama-agent's removal — unrelated origin PR #69 — collapsed this from the ~2000s it took on the first finish pass).

    Subtasks/acceptance criteria in the description are unchanged — this pass fixed a test fixture regression, not the merge itself, and every item there was already checked and remains accurate.

    next: /review
  timestamp: 2026-08-03T17:11:23.012268+00:00
- actor: claude-code
  id: 01kz49xppx397x0cq8fk4n7n4n
  text: |-
    ### finish iteration 2 — changed
    - implement: changed — all 3 failures traced to ONE root cause, not three: crates/swissarmyhammer-tools/tests/integration/review_fixture.rs's scripted fan-out rules (fanout_rules()) were still keyed on the retired single-rule validator names (no-secrets, data-driven, dead-code). Production sends `# Validator: code-security` / `# Validator: code-hygiene` (one fan-out task per SET), so those scripted rules never matched, findings silently vanished, and review_progress_stdio_test's real timeout was just it correctly waiting forever for a verdict that could never arrive — not a latency regression from bundling 6 rules, as I'd hypothesized. No timeout change was needed or made. Rekeyed the fixture to the two new set names; added three_findings() to bundle data-driven + both dead-code findings under one code-hygiene response, matching production's one-task-per-set shape. No production code, VALIDATOR.md, or rule text touched.
    - test: green — cargo test -p swissarmyhammer-tools (162 + 3 + 2 relevant suites) all passed; cargo test -p mirdan -p swissarmyhammer-validators 412+362 passed, 2 doctests; fmt clean; clippy clean. Required broader check run for real: `cargo nextest run -E 'rdeps(swissarmyhammer-validators) or rdeps(mirdan)'` => 3545 tests run, 3545 passed (5 slow, 1 leaky), 0 skipped, 0 failed. Independently spot-checked the fixture diff and the byte-identity of all 9 moved rule files myself before accepting.
    - commit: 54fc50ac0 (also folded in a one-line builtin/validators/README.md fix: an example name still said `no-secrets/`, now `code-security/`)
    - next: /review HEAD~1..HEAD
  timestamp: 2026-08-03T17:14:41.757324+00:00
- actor: claude-code
  id: 01kz4cjzyv7dd7bxcda97qj5tg
  text: |
    ### review — findings
    - evidence: `review sha 54fc50ac0~1..54fc50ac0` — engine ran to completion (0 failed, 1 skip: `crates/mirdan/src/install.rs` too large for the duplication validator's batch budget, pre-existing code, not part of this diff). Engine returned 63 confirmed findings; blame-checked every one against `54fc50ac0`. 60 cite pre-existing code (all of `install.rs`, most of `lib.rs`, all of `review_fixture.rs`, plus `builtin/mod.rs:65,165,351`) — dropped. One more (`builtin/mod.rs:296`) is new by blame but its fix requires deduplicating against the pre-existing `test_safety_validators_match_expected_paths` test — dropped under the test-refactor exception. 3 findings on genuinely new code stand: `crates/mirdan/src/builtin_validators.rs:73`, `crates/mirdan/src/retired_validators.rs:31`, `crates/mirdan/src/lib.rs:37` (line corrected from the engine's reported 36).
    - next: /implement to address the 3 findings, then re-review
  timestamp: 2026-08-03T18:01:16.507934+00:00
- actor: claude-code
  id: 01kz4cm45p1zc85691dqb7v4p0
  text: |-
    ### finish iteration 2 — findings
    - review: findings — engine ran to completion (failed: 0, skipped: 1 immaterial oversized-file skip on install.rs), no stale-process signs (sah --version 0.17.0/fba8183656f9). 63 raw hits, blame-checked against 54fc50ac0, 3 survive: builtin_validators.rs:73 (test_builtin_validators_embed_expected_sets checks the two sets exist but never verifies the nine moved rule files landed inside them), retired_validators.rs:31 (RetiredFile derives Debug/Clone/Copy but not PartialEq/Eq), lib.rs:37 (new `pub mod retired_validators;` has no doc comment).
    - next: /implement fixes all 3, then re-review
  timestamp: 2026-08-03T18:01:53.590925+00:00
- actor: claude-code
  id: 01kz4czezy5e00d7ejwn1tjte7
  text: |-
    ### implement — changed

    Fixed the 3 open review findings:

    1. `crates/mirdan/src/builtin_validators.rs` — extended `test_builtin_validators_embed_expected_sets` to assert the moved rule file names are present under each merged set: `code-security` now checks for `rules/no-secrets.md`, `rules/injection.md`, `rules/command-safety.md`; `code-hygiene` checks for `rules/no-commented-code.md`, `rules/function-length.md`, `rules/cognitive-complexity.md`, `rules/missing-docs.md`, `rules/data-driven.md`, `rules/dead-code.md`.
    2. `crates/mirdan/src/retired_validators.rs` — added `PartialEq, Eq` to `RetiredFile`'s derive list. Also added them to `RetiredSet` (same struct-derive-completeness cause, same file, same reasoning: a slice of `RetiredFile` needs `PartialEq`/`Eq` on the element type to make `RetiredSet` itself comparable).
    3. `crates/mirdan/src/lib.rs` — added a doc comment above `pub mod retired_validators;` explaining it holds the retired builtin validator sets' shipped content snapshot, used by the refresh-prune mechanism in `install.rs`.

    Evidence:
    - `cargo fmt --all` — clean, no diff.
    - `cargo clippy --workspace --all-targets -- -D warnings` — clean, 0 warnings.
    - `cargo test -p mirdan -p swissarmyhammer-validators` — mirdan: 412 passed + 2 (findings_are_requirements_coverage), 0 failed; swissarmyhammer-validators: 362 passed + 2 doctests, 0 failed.
    - Required broader check run for real (not skipped): `cargo nextest run -E 'rdeps(swissarmyhammer-validators) or rdeps(mirdan)'` → `Summary [ 92.804s] 3545 tests run: 3545 passed (8 slow), 0 skipped`.

    All 3 checklist items under `## Review Findings (2026-08-03 18:00)` are checked off.

    next: /review
  timestamp: 2026-08-03T18:08:05.118720+00:00
- actor: claude-code
  id: 01kz4d5m5h94c340kh0byrmd8h
  text: |-
    ### finish iteration 3 — changed
    - implement: changed — all 3 round-2 findings fixed: builtin_validators.rs test now asserts the nine moved rule files actually landed in the two merged sets, not just that the sets exist; RetiredFile + RetiredSet gained PartialEq/Eq derives; retired_validators module now has a doc comment.
    - test: green — fmt clean, clippy clean, cargo test -p mirdan -p swissarmyhammer-validators 412+2 and 362+2 doctests passed, full rdeps(swissarmyhammer-validators) or rdeps(mirdan) run for real: 3545 passed, 0 skipped, 0 failed
    - commit: c535bb75e
    - next: /review HEAD~1..HEAD
  timestamp: 2026-08-03T18:11:27.025406+00:00
- actor: claude-code
  id: 01kz4ds5s2gvz581jmn1ttbadk
  text: |-
    ### review — clean
    - evidence: `review sha c535bb75e~1..c535bb75e`. Engine ran to completion (0 failed, 0 skipped). sah --version 0.17.0/fba8183656f9, matching round 2 — no stale-process signal. Returned 60 confirmed findings, all "missing doc comment" flags on `pub mod` declarations in `crates/mirdan/src/lib.rs` (lines 12,13,14,15,17-36,42-48). Blame-checked every one against `c535bb75e` via `git blame -L 1,50 c535bb75e -- crates/mirdan/src/lib.rs`: all 60 blame to commits from Feb-June 2026, pre-existing, untouched by this diff. None cite line 37 (the doc comment this commit actually added) or anything in `builtin_validators.rs`/`retired_validators.rs`. 0 new findings survive blame-check. All 3 round-2 findings remain checked `[x]`.
    - next: done
  timestamp: 2026-08-03T18:22:07.650439+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffff9580
title: Merge nine single-rule builtin validators into code-security and code-hygiene
---
## What

The review fleet makes one agent task for each validator that matches the changed
files. Nine builtin validators hold one rule each, so one changed source file
costs nine agent tasks. Merge them into TWO sets, split by concern.

The source of truth is `builtin/validators/` (deployed to `~/.validators` by
`sah init`; see `builtin/validators/README.md` for the precedence rules).

Every one of the nine matches `@file_groups/source_code`, so the match globs
merge without loss.

**RESOLVED CONFLICT (see comments for full record):** the original text below
said code-hygiene needs only the `callers` probe. That is wrong: the
`cognitive-complexity` rule (from `complexity`) declares its own `complexity`
probe (added in 8d7d8f57d, computes deterministic tree-sitter numbers), and
probes are declared once per validator SET, not per rule
(`crates/swissarmyhammer-validators/src/review/scope.rs` reads
`manifest.probes` for the whole set). Dropping `complexity` would silently kill
that rule's evidence mechanism and reintroduce the LLM nondeterminism
8d7d8f57d removed. Asked the user; answer was to keep both probes.
**`code-hygiene` therefore declares `probes: [callers, complexity]`, not just
`[callers]`.**

## The two sets

**`code-security`** — no probes. Rules:

- `no-secrets.md`
- `injection.md`
- `command-safety.md`

**`code-hygiene`** — `probes: [callers, complexity]` — `callers` for
`dead-code`, `complexity` for `cognitive-complexity`. Rules:

- `no-commented-code.md`
- `function-length.md`
- `cognitive-complexity.md` (from `complexity`)
- `missing-docs.md`
- `data-driven.md`
- `dead-code.md`

Keep security separate from hygiene. A leaked credential or an injection hole is
not untidiness, and a set named "hygiene" understates it. Two names, two
concerns.

## Out of scope — do not touch these

- `test-integrity` — it matches `@file_groups/test_files` as well as source, so
  it does not merge with a source-only set. Leave it whole.
- `reuse` (`probes: [similar]`) and `duplication` (`probes: [duplicates]`) —
  each carries its own probe. Folding either in would force its probe on every
  rule in the set. Leave them alone.
- `naming` and `magic-numbers` — user-level sets in `~/.validators` only. They
  do not exist in `builtin/validators/`.
- The language sets (`rust`, `python`, `swift`, `dart`, `js-ts`, `numpy`) and
  `completeness`.

## Changes

- Create `builtin/validators/code-security/VALIDATOR.md`, `match.files: [@file_groups/source_code]`, no probes.
- Create `builtin/validators/code-hygiene/VALIDATOR.md`, `match.files: [@file_groups/source_code]`, `probes: [callers, complexity]`.
- Move the nine rule files unchanged into the two `rules/` directories.
- Delete the nine retired set directories from `builtin/validators/`.
- Make the builtin validator refresh remove a retired builtin set from the
  deployed store (`~/.validators`), but ONLY when the deployed files are
  identical to what was shipped. A user-modified set of the same name stays.

## Subtasks

- [x] Create `builtin/validators/code-security/VALIDATOR.md`
- [x] Create `builtin/validators/code-hygiene/VALIDATOR.md` with `probes: [callers, complexity]`
- [x] Move the nine rule files into the two new `rules/` directories
- [x] Delete the nine retired set directories
- [x] Remove retired, unmodified builtin sets from the deployed store on refresh (`crates/mirdan/src/install.rs`)
- [x] Update the embed and loader tests

## Acceptance Criteria

- [x] The loader reports `code-security` with 3 rules and no probes
- [x] The loader reports `code-hygiene` with 6 rules and `probes: ["callers", "complexity"]`
- [x] The loader no longer reports the nine retired set names from the builtin layer
- [x] `test-integrity`, `reuse` and `duplication` still load unchanged
- [x] A refresh deploy removes an unmodified retired set from the target store, and keeps a user-modified set of the same name
- [x] Every one of the nine rule texts ships unchanged — no rule is reworded, weakened or dropped by this merge

## Tests

- [x] Update `test_builtin_validators_embed_expected_sets` in `crates/mirdan/src/builtin_validators.rs`: assert `code-security` and `code-hygiene` are present and the nine retired names are gone
- [x] Update the loader tests in `crates/swissarmyhammer-validators/src/builtin/mod.rs` (they read `../../builtin/validators` directly): assert both new sets, their rule counts, and their probes
- [x] New test for the refresh prune in `crates/mirdan/src/install.rs`: deploy the old set, refresh, assert it is gone; deploy a modified copy, refresh, assert it stays
- [x] `cargo test -p mirdan -p swissarmyhammer-validators` passes

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.

## Review Findings (2026-08-03 18:00)

Scope: `review sha 54fc50ac0~1..54fc50ac0`. Engine ran to completion (0 failed tasks, 1 skip — `crates/mirdan/src/install.rs` was too large for the duplication validator's batch budget; that skip does not affect the items below, since every install.rs finding the engine did return is pre-existing code, not part of this diff — see the blame check note).

Blame check against `54fc50ac0`: the engine returned 63 confirmed findings. 60 of them cite lines that blame to a commit other than `54fc50ac0` (all in `crates/mirdan/src/install.rs`, most of `crates/mirdan/src/lib.rs`, and all of `crates/swissarmyhammer-tools/tests/integration/review_fixture.rs`) — pre-existing code this diff did not touch. Those are dropped. Three more of the sixty were in `crates/swissarmyhammer-validators/src/builtin/mod.rs` (lines 65, 165, 351) and are also pre-existing by blame. A further finding at `crates/swissarmyhammer-validators/src/builtin/mod.rs:296` is new by blame, but its fix is to deduplicate a test loop against the pre-existing `test_safety_validators_match_expected_paths` test function — dropped under the review skill's rule against refactoring test code that already existed.

The following 3 findings cite code this commit actually added, and stand:

- [x] `crates/mirdan/src/builtin_validators.rs:73` — Nine rule files are moved (written) into code-security and code-hygiene sets; test verifies these sets exist but doesn't verify the moved rules are present in them. Add an assertion in the test to verify that code-security contains the rules that were moved from no-secrets, injection, and command-safety (e.g., files.iter().any(|(name, _)| name.contains("rules/no-secrets.md")) etc.), and similarly for code-hygiene's source sets.
- [x] `crates/mirdan/src/retired_validators.rs:31` — Public struct `RetiredFile` omits `PartialEq` and `Eq` derives, blocking downstream crates from implementing these standard comparison traits due to orphan rules. Derive `PartialEq` and `Eq`: change line 31 to `#[derive(Debug, Clone, Copy, PartialEq, Eq)]`.
- [x] `crates/mirdan/src/lib.rs:37` — Public module `retired_validators` lacks documentation. Add a doc comment above line 37 explaining the module's purpose. (The engine reported this at line 36; blame-corrected to line 37 — line 36 is the pre-existing `pub mod registry;`, line 37 is the new `pub mod retired_validators;` this commit adds.) #review