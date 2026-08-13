---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzmcczzat3sgwwk1r7rcr5we
  text: |-
    Research done. The card's file layout is stale; here is the current state and the plan.

    Where the work lands:
    - `crates/swissarmyhammer-validators/src/doctor.rs` — `check_tool_rule` does presence -> version -> fixtures. Only `check_fixtures` is slow (it runs the rule's own `run` script twice).
    - `crates/swissarmyhammer-validators/src/review/tool_rules.rs` — `plan_tool_rules` -> `plan_rule_by_health` calls `check_tool_rule` once per matched rule.
    - `crates/swissarmyhammer-validators/src/review/synthesize.rs` `run_review` — stage 2 plans AND executes the tool runs before the batch loop. That is the serial half the card names.

    Cache design:
    - New module `review/tool_health.rs` with `ToolHealthCache`, stored at `<workspace>/.sah/tmp/review-tool-health.json` (the `tmp` subdir is already gitignored and auto-created by `ManagedDirectory<SwissarmyhammerConfig>`).
    - Key = (tool version string from `doctor.check_version_command`, SHA-256 of the rule name + the serialized `tool` block + every file in the set's `fixtures/` directory).
    - A rule with no version command is never cached — a tool upgrade would be undetectable, so it re-verifies every run. Every shipped rule declares a version command.
    - One entry point `tool_rule_health(cache, proof, ruleset, rule, spec)` with `HealthProof::{Stored, Fresh}`. The planner asks for `Stored`; `check_review_engine_with` asks for `Fresh`, which always runs the fixtures and replaces the stored verdict, so doctor stays the ground truth.

    Overlap design:
    - `start_tool_runs(runs, repo_root, progress) -> ToolRunsInFlight` puts `execute_tool_runs` on `spawn_blocking` (the scripts are blocking processes).
    - `run_review` keeps the plan step (the suppression map is what the fan-out needs), starts the runs, then awaits `ToolRunsInFlight::finish()` after the batch loop and before synthesis.
    - A task that panics is reported as one `ToolRunError` per run it carried, so no run is silently lost.
    - `Planned { total_pairs }` is SUMMED by the MCP progress consumer, so interleaved tool and fleet plan events stay correct.
  timestamp: 2026-08-09T23:05:50.826758+00:00
- actor: claude-code
  id: 01kzme410thbbam45n9p0ejyt0
  text: |-
    Implementation landed. Both halves of the card are in.

    1. Stored fixture verdict. New module `crates/swissarmyhammer-validators/src/review/tool_health.rs`:
    - `ToolHealthCache::open(workspace_root)` reads `<workspace>/.sah/tmp/review-tool-health.json`; `save()` writes it back. The `tmp` subdirectory is created and git-ignored by `ManagedDirectory<SwissarmyhammerConfig>`, and the repository `.gitignore` already covers `**/.sah/`.
    - `tool_rule_health(cache, proof, ruleset, rule, spec)` is the one entry point. `HealthProof::Stored` reads a stored verdict; `HealthProof::Fresh` runs the fixtures and replaces what is stored.
    - Keys: the tool version `doctor.check_version_command` reports, plus a SHA-256 of the rule's whole `tool` block and every file in the set's `fixtures/` directory. A rule that reports no version is never stored, because a tool upgrade would be undetectable.
    - `doctor.rs` gained `check_tool_rule_with`, which reads presence and version fresh and takes the fixture half as a parameter. `check_tool_rule` is that function with the plain fixture run, so the engine and doctor describe a tool rule the same way.
    - `check_review_engine_with` and `check_review_engine` take the cache and ask for `Fresh`.

    2. Overlap. `start_tool_runs(runs, repo_root, progress) -> ToolRunsInFlight` puts `execute_tool_runs` on `spawn_blocking`. `run_review` plans, saves the verdicts, starts the runs, runs the whole batch loop, then awaits `ToolRunsInFlight::finish()` before synthesis. A task that does not finish reports one `ToolRunError` per run it carried, so no run is lost in silence.

    What did not work, so the next agent does not repeat it:
    - The first `VerdictKeys` version held the fixture-directory lock across a `match` scrutinee and its arm. That is a self-deadlock on a `std::sync::Mutex`: seven tests timed out at 300 s each. Take the guard in its own statement.
    - The same version remembered the fixture digest per directory for the life of the cache. It made an edited fixture invisible within one run, so `an_edited_fixture_proves_the_rule_again` still failed. The memo is gone: a set ships a handful of small fixtures, and reading them costs far less than one fixture run.
    - Two drive tests share a helper now in `test_support` (`counting_tool_script`, `write_counted_tool_rule_fixtures`, `fixture_runs`, `FIXTURE_RUN_MARKER`). The marker works because the doctor copies the WHOLE fixtures directory into a scratch directory, so a file written beside the fixture pair reaches every fixture run and no other run.

    `ARCHITECTURE.md` gained a paragraph under the fact-producer section, because the review engine now owns an on-disk artifact it did not own before.
  timestamp: 2026-08-09T23:35:54.138289+00:00
- actor: claude-code
  id: 01kzme4c89mbmrg0xsd01evrn2
  text: |-
    ### implement — changed
    - evidence: 15 files. New: `crates/swissarmyhammer-validators/src/review/tool_health.rs`, `.../review/tool_health/tests.rs`. Changed: `.../src/doctor.rs`, `.../review/tool_rules.rs`, `.../review/synthesize.rs`, `.../review/mod.rs`, `.../review/drive.rs`, `.../review/test_support.rs`, `.../review/tool_rules/tests/plan.rs`, `.../review/tool_rules/tests/shipped.rs`, `crates/swissarmyhammer-validators/Cargo.toml`, `Cargo.lock`, `ARCHITECTURE.md`, `apps/swissarmyhammer-cli/tests/commented_code_tool_rule.rs`, `apps/swissarmyhammer-cli/tests/duplication_tool_rule.rs`.
    - RED, acceptance 1 (`a_second_review_of_an_unchanged_tool_rule_runs_no_fixture_script`): "assertion `left == right` failed: the first review proves the rule with the fail fixture and the pass fixture; the second must read that verdict back instead of proving it again / left: 4 / right: 2".
    - RED, acceptance 2 (`the_tool_run_overlaps_the_fleet_instead_of_delaying_it`): "the review file pipeline must not hang: Elapsed(())" after 37.3 s — the serial tool run held the runtime thread, so the fleet never started.
    - RED, invalidation (`a_changed_tool_version_proves_the_rule_again`, `an_edited_run_script_proves_the_rule_again`, `an_edited_fixture_proves_the_rule_again`): "left: 2 / right: 4" with "a verdict proved against the old tool version / run script / fixtures must not stand for the new one".
    - RED, doctor (`doctor_proves_the_rule_again_and_replaces_the_stored_verdict`): "error[E0061]: this function takes 2 arguments but 3 arguments were supplied" at `check_review_engine_with`.
    - gates: `cargo nextest run --workspace` 14002 tests run, 14002 passed, 0 skipped. `cargo fmt --all --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - next: ready for /review.
  timestamp: 2026-08-09T23:36:05.641709+00:00
- actor: claude-code
  id: 01kzmfq5qm03cs7g0r8z1698bt
  text: |
    ### review — findings
    - scope: `review sha c1e6cca1c^..c1e6cca1c` (explicit range; HEAD is moved by a parallel session and is not trustworthy here)
    - engine counts: 4 findings recorded — 2 confirmed by the engine (45 attempted, 6 refuted, 0 failed, 0 skipped), 2 confirmed by in-turn verification of the cache and concurrency claims
    - evidence: crates/swissarmyhammer-validators/src/review/drive.rs:2001, crates/swissarmyhammer-validators/src/review/test_support.rs:136, crates/swissarmyhammer-validators/src/review/tool_health.rs:119, crates/swissarmyhammer-validators/src/review/tool_health/tests.rs:137
    - verified sound, no finding raised:
      - a stale verdict is not reachable. Version and presence are read fresh on every check (doctor.rs:313-330); a stored verdict replaces only the fixture half. An absent, failing, or empty `check_version_command` yields `None` (doctor.rs:377-390), so `VerdictKeys::of` returns `None` and the rule is proved without being stored. The content digest covers the whole serialized `ToolSpec` (`scope`, `run`, `doctor`, `install`, no `serde(skip)`), so an edited run script invalidates. The fixture digest hashes sorted path plus file content, so an edit, an addition, and a deletion all invalidate.
      - the non-recursive `read_dir` in `fixture_digest` (tool_health.rs:282-288) mirrors the doctor's own non-recursive scratch materialization (doctor.rs:549-557), which skips non-files too. The digest scope and the run scope agree, so a subdirectory is invisible to both and no stale path opens.
      - no fixture-digest memo survives. `fixture_digest` reads from disk on every call with no `OnceCell`, `HashMap`, or `lazy_static`. `an_edited_fixture_proves_the_rule_again` (tool_health/tests.rs:66) edits a fixture between two probes on ONE cache in ONE process and asserts the fixtures ran twice.
      - the self-deadlock in the health path is gone. `ToolHealthCache::stored` returns an owned `Option<FixtureOutcome>` and releases the guard before returning, so the `if let` at tool_health.rs:226 does not hold the lock when `cache.prove` re-acquires it at tool_health.rs:236.
      - `HealthProof::Fresh` bypasses the stored read (the `proof == HealthProof::Stored` guard at tool_health.rs:225) and falls through to `prove`, which runs the fixtures and overwrites. `doctor_proves_the_rule_again_and_replaces_the_stored_verdict` (tool_health/tests.rs:87) proves doctor sees a break the stored verdict hides and that the engine then follows doctor's replaced verdict.
      - no tool finding can be lost or misattributed. The batch loop between `start_tool_runs` (synthesize.rs:666) and `finish()` (synthesize.rs:744) is straight-line `await`s with no `?` and no early return. A `JoinError` maps to one `ToolRunError` per carried identity with validator and rule preserved (tool_rules.rs:692-712). Tool findings join the verified stream after the fleet drains and keep their own provenance; they skip the adversarial verify pass by documented design because tool output is deterministic.
      - the suppression plan does not race the fan-out. `plan_tool_rules` resolves the health verdict synchronously and `into_parts` yields `suppression` before `start_tool_runs` is called, so `run_fleet` always reads a settled map.
    - tests: `cargo test -p swissarmyhammer-validators tool_health` — 5 passed, 0 failed, 0 ignored
    - next: implementer clears the four unchecked items, then re-review the same range
  timestamp: 2026-08-10T00:03:50.132176+00:00
- actor: claude-code
  id: 01kzmfrbm6paggnknt74t4z3w3
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 15 files; new `review/tool_health.rs` + tests; the stored fixture verdict and the overlapped tool run
    - test: green — `cargo nextest run --workspace`, 14002 passed, 0 skipped; fmt clean; clippy clean
    - commit: c1e6cca1c
    - review: findings — crates/swissarmyhammer-validators/src/review/drive.rs:2001, crates/swissarmyhammer-validators/src/review/test_support.rs:136, crates/swissarmyhammer-validators/src/review/tool_health.rs:119, crates/swissarmyhammer-validators/src/review/tool_health/tests.rs:137
    - note: the three named correctness concerns held. No stale verdict is reachable, no tool finding can be lost or misattributed, and both defects the implementer named are dead. The guard-across-match shape survives in `save()` only.
  timestamp: 2026-08-10T00:04:28.934828+00:00
- actor: claude-code
  id: 01kzmfz2k1w1pqyz1cx2n7y5rx
  text: |-
    Iteration 2 picked up. Research done on the four findings.

    1. `drive.rs` worker threads. 14 of the 15 `#[tokio::test(flavor = "multi_thread", ...)]` attributes in the file say `worker_threads = 2`; only `the_tool_run_overlaps_the_fleet_instead_of_delaying_it` says 4. The tokio macro reads an integer literal, so a named `const` cannot go in the attribute. The fix is therefore to make the count agree with the suite, and to prove 2 is enough by running the test.

    2. `test_support.rs` shell injection. `counting_tool_script` writes `echo run >> "{counter}"`. Double quotes do not stop a backtick, a `$(...)`, or an embedded quote in the path. The `which "$@"` shape from ^t0dgame does not fit here: the engine already passes the changed files as the script's positional parameters, so `$@` is taken. This crate already answers the same question once — `tool_install.rs` tests carry a private `shell_quote` that wraps a path in single quotes and writes `'` as `'\''`. That helper moves to `test_support.rs`, `counting_tool_script` uses it, and the private copy goes.

    3. `tool_health.rs` `save()`. The guard from `self.verdicts()` is a named binding, so it lives to the end of the function — across the `match` scrutinee, both arms, and the blocking `std::fs::write`. `stored()` and `prove()` do not share the defect: each takes the guard in a statement of its own, so the guard drops at the semicolon.

    4. `tool_health/tests.rs`. All five tests build the probe rule through `ProbeDirs::ruleset`, which always sets `check_version_command`. The invariant "a rule that reports no version is never stored" is stated in the module docs and implemented by the `version?` in `VerdictKeys::of`, and nothing tests it.
  timestamp: 2026-08-10T00:08:09.057748+00:00
- actor: claude-code
  id: 01kzmg2q4m4h2y7nrqnmqdknrk
  text: |
    ### review — findings (adversarial verification pass folded in)

    The adversarial verifier returned REVISE after the first section was written. Its findings are recorded as the second dated section. This comment supersedes the count in the previous comment.

    - scope: `review sha c1e6cca1c^..c1e6cca1c`
    - total: 12 open findings — 2 from the engine, 2 from my own in-turn verification, 8 from the adversarial verifier (one of its eight restated a finding already recorded, so it was not duplicated; its distinct failing-version-command branch was added instead)
    - verified myself before recording, rather than accepting on report:
      - the digest collision is real. `find_fixture` (`doctor.rs:456`) matches on `starts_with("{rule}.{kind}.")`, so `probe-tool.pass.rsX` is a live fixture name and the unframed path+content concatenation in `fixture_digest` collides with `probe-tool.pass.rs`.
      - the repository side effect is real. `ManagedDirectory::new` calls `write_gitignore_if_needed` unconditionally (`directory.rs:113`), so opening the cache creates `.sah/`, `.sah/.gitignore`, and `.sah/tmp/` in the repo under review. This repo ignores `**/.sah/` at `.gitignore:166`, so the untracked-file pollution does not appear here; the unconditional write at review time still stands.
      - the sticky negative verdict is real. `prove` (`tool_health.rs:163`) inserts whatever `check_fixtures` returned and `stored` (`tool_health.rs:151`) returns the stored variant unchecked, so a `Failed` from a transient `cargo clippy` failure is replayed as a prompt fallback on every later review.
    - the earlier comment's "verified sound" list stands unchanged. The verifier reached the same conclusion independently on all of it: no stale verdict from a version, run-script, or fixture edit; no surviving fixture-digest memo; no lost or misattributed tool finding; no suppression race; the health-path self-deadlock gone.
    - the one disagreement: the verifier read `save()`'s guard-across-the-match as contention only and raised no finding. I keep that finding recorded — the change states the pattern was removed, and it survives at `tool_health.rs:119`.
    - tests: `cargo test -p swissarmyhammer-validators tool_health` — 5 passed, 0 failed, 0 ignored. Verifier additionally ran `cargo build` and `cargo clippy -p swissarmyhammer-validators --lib --all-features` clean.
    - not verified by either pass: the real-tool timing payoff is unmeasured (the overlap test uses a scripted agent and a sleeping script); `save()` uses a non-atomic `std::fs::write`, and concurrent writers were not tested, though `read_verdicts` degrades safely; the two tool-rule integration tests were not executed.
    - next: implementer clears the twelve unchecked items, then re-review `c1e6cca1c^..c1e6cca1c`
  timestamp: 2026-08-10T00:10:08.404725+00:00
- actor: claude-code
  id: 01kzmgg4rkjea7as1w3naweva4
  text: |-
    The four findings of the 18:37 section are closed, and all three gates were green on them (14004 tests run, 14004 passed, 0 skipped; fmt clean; clippy clean).

    1. `drive.rs` — the test now says `worker_threads = 2`, which is what the other 14 multi-thread tests in the file say. Note on the finding's suggested fix: the tokio macro reads an integer literal for `worker_threads`, so a named `const` cannot go in the attribute; the count itself had to change. To prove 2 is not merely enough to pass but enough to still CATCH a serial tool run, `synthesize.rs` was temporarily changed to await `ToolRunsInFlight::finish()` right after `start_tool_runs`, before the batch loop. The test then failed at 2 workers with "a fleet pair must finish while the tool script is still running; the tool run delayed the fan-out instead of overlapping it" after 10.1 s. `synthesize.rs` was put back, and `git diff` reports it unchanged.

    2. `test_support.rs` — `shell_quote` is now a shared fixture there, and `counting_tool_script` uses it. `tool_install.rs` had a private copy of the same helper; that copy is gone and its tests import the shared one, so the crate holds one answer to shell quoting. `tool_health/tests.rs` interpolated two more paths the same way (the version command and the break file); both now use `shell_quote`. A new test, `shell_quote_keeps_a_command_substitution_in_a_path_from_running`, runs a real `sh` against a path holding `$(touch pwned)'x` and asserts the shell read it as one literal word and never ran the substitution.

    3. `tool_health.rs` `save()` — the map is snapshotted in a statement of its own, so the guard drops at that semicolon. Neither the encode nor the blocking write holds the lock now, whatever either one calls.

    4. `tool_health/tests.rs` — new test `a_rule_that_reports_no_version_is_never_stored`. RED was proved by putting the regression the finding names into the production code (`version?` became `version.unwrap_or_default()`): "assertion `left == right` failed: a rule that reports no version must run its fixtures on every probe / left: 2 / right: 4", while the other five tests all passed — which is the finding, measured. The production line is back to `version?`.
  timestamp: 2026-08-10T00:17:28.339725+00:00
- actor: claude-code
  id: 01kzmhdkeyb5cv5vw6qzaedvsc
  text: |-
    The eight findings of the 19:12 section are closed. Every one has a test, and each test was seen RED against the regression the finding names before it went green.

    Production changes in `review/tool_health.rs`:

    1. Sticky negative verdict. `prove` now stores an entry only when the fixtures PASSED, and drops whatever stood under the key when they did not. `StoredVerdict` lost its `fixtures` field, so the shape itself cannot carry a failure: an entry standing under a key IS the statement that the rule passed under those keys. `stored` became `passed -> bool`, and the replay path returns `FixtureOutcome::Passed` rather than a stored variant.
       - Consequence, stated because it changed a test: doctor now DROPS a verdict its own run did not earn instead of replacing it with a failure. `doctor_proves_the_rule_again_and_replaces_the_stored_verdict` is renamed to `..._and_drops_the_stored_verdict`, and its last assertion changes from "the engine reads doctor's verdict without proving the rule" to "with no verdict standing, the engine proves the rule for itself". Doctor stays the ground truth either way: the engine still follows doctor and reports the rule unusable.
       - `FixtureOutcome` lost its `serde::Serialize`/`Deserialize` derives and the doc paragraph that explained them. Commit c1e6cca1c added both only for the stored shape, and nothing else in the workspace serializes the enum, so they were left unreachable by this fix.

    2. Module doc. The digest bullet no longer claims the rule name. A new paragraph states that the rule name is the storage key `<set>/<rule>`, and that two rules of one set with identical `tool` blocks share a digest and still hold their own verdicts.

    3. Unframed digest. `fixture_digest` now feeds every name and every content blob through `update_framed`, which writes the length first. RED, with the framing removed: "assertion `left != right` failed: a name that grows by the byte its content loses must not digest the same; the doctor reads both names as this rule's pass fixture".

    4. Unreadable fixture. The read error is no longer dropped. A file that reads is tagged `FIXTURE_READ` and framed; one that does not is tagged `FIXTURE_UNREADABLE` and logged. RED, with the error dropped: "assertion `left != right` failed: a fixture the digest cannot read must not stand for an empty one".

    5. Write on open. `cache_path` now derives the path and creates nothing. `ToolHealthCache` holds the workspace root, and `save()` returns early when it has no verdict, then calls `writable_cache_path()`, which is the one place the engine writes into the tree it reviews. RED, with a `from_custom_root` call put back into `open`: "opening a cache must leave the reviewed tree as it found it" and "a review that stored no verdict must write nothing into the reviewed tree".

    Tests added, with the RED each one was proved against:
    - `a_broken_fixture_run_is_never_stored` — RED with `prove` storing every outcome: "assertion failed: !probe_health(&cache, HealthProof::Stored, &ruleset).usable()".
    - `an_added_fixture_neighbour_proves_the_rule_again` and `a_deleted_fixture_neighbour_proves_the_rule_again` — RED with the digest narrowed to the `.pass.`/`.fail.` names: "left: 2 / right: 4" under "a fixture run reads the whole directory, so an added (deleted) file must not stand under the old verdict".
    - `a_version_command_that_fails_leaves_the_rule_unstored` and `a_version_command_that_reports_nothing_leaves_the_rule_unstored` — RED with `version?` relaxed to `version.unwrap_or_default()`: "left: 2 / right: 4" under "a version command that exits nonzero (prints nothing): the fixtures must run on every probe". The three "unstored" tests share `assert_never_stored`, so the shape is written once.
    - `opening_a_cache_creates_nothing_in_the_workspace`, `a_cache_with_no_verdict_creates_nothing_in_the_workspace`, and `a_stored_verdict_survives_a_reopened_cache`. The last one covers the save-and-reopen round trip, which nothing exercised before: every earlier test used one cache in one process.
    - `two_fixture_sets_that_share_one_byte_stream_digest_differently` and `an_unreadable_fixture_does_not_digest_as_an_empty_one` (unix only, because it needs a file of mode 0).
    - `a_tool_run_task_that_panics_reports_every_run_it_carried`, in `tool_rules/tests/execute.rs`. It builds `ToolRunsInFlight` over two real runs with `RunIdentity::of`, and a real `spawn_blocking` task that panics. RED with the `Err` arm returning a default outcome: "assertion `left == right` failed: every run the task carried must be reported under its own names / left: [] / right: [(\"docs\", \"docs-tool\"), (\"todo\", \"todo-tool\")]".

    `ARCHITECTURE.md` was corrected: the paragraph said the engine stores "that verdict" and that doctor "replaces what is stored". It now says a PASS is stored, that a rule which does not pass is proved again every run, and that nothing is created until a verdict is saved.
  timestamp: 2026-08-10T00:33:33.662508+00:00
- actor: claude-code
  id: 01kzmhdtvbagt5d2ae8k2pf6jz
  text: |-
    ### implement — changed
    - evidence: 8 files — crates/swissarmyhammer-validators/src/review/tool_health.rs, .../review/tool_health/tests.rs, .../review/test_support.rs, .../review/tool_install.rs, .../review/drive.rs, .../review/tool_rules/tests/execute.rs, .../src/doctor.rs, ARCHITECTURE.md
    - findings: 12 of 12 closed, both dated sections checked
    - gates: `cargo nextest run --workspace` 14015 tests run, 14015 passed, 0 skipped. `cargo fmt --all --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - next: ready for /review of the same range.
  timestamp: 2026-08-10T00:33:41.227046+00:00
- actor: claude-code
  id: 01kzmjh3127xs1ebqteh0mk4sg
  text: |
    ### review — findings
    - scope: `review sha 6258c7d47^..6258c7d47` (one commit, 8 files)
    - engine counts: 1 finding, 1 confirmed, 2 refuted, 27 attempted, 0 failed
    - verified in this turn, not delegated: all 12 prior findings are fixed in the tree
      - only a PASS is stored: `StoredVerdict` has no outcome field (`tool_health.rs:85`); `prove` inserts on `Passed` and removes otherwise (`tool_health.rs:217`)
      - digest framing: `update_framed` writes a u64 length before each path and each blob (`tool_health.rs:385`); the `probe-tool.pass.rs`/`XY` versus `probe-tool.pass.rsX`/`Y` collision breaks on the differing path length prefix
      - unreadable is not empty: `FIXTURE_READ` 0 versus `FIXTURE_UNREADABLE` 1 (`tool_health.rs:322`)
      - `open` creates nothing: `cache_path` is a pure join (`tool_health.rs:395`); `writable_cache_path` runs only inside `save` (`tool_health.rs:151`)
      - `shell_quote` is shared (`test_support.rs:142`); the private duplicate left `tool_install.rs`
      - `save` snapshots the map in its own statement (`tool_health.rs:135`)
      - the module doc no longer puts the rule name in the digest (`tool_health.rs:17`)
      - the five named tests exist, plus a deleted-neighbour test and a version-reports-nothing test
    - beyond the finding text, checked and correct: `FixtureOutcome` keeps no serde use anywhere; `ARCHITECTURE.md:563` matches the new behaviour; `drive.rs:2001` `worker_threads` 4 to 2 now matches all 15 other tests in the file; `synthesize.rs` is not in the commit and is clean in the tree
    - new findings: 3 — the doctor drop does not reach the disk when the map empties, its test never crosses `save`, and `shell_quote` duplicates `shell_escape_path`
    - next: fix the three findings, then re-review the same range
  timestamp: 2026-08-10T00:52:56.482963+00:00
- actor: claude-code
  id: 01kzmjhx3wt9x2rnn9b4q7dz2d
  text: |-
    ### finish iteration 2 — findings
    - implement: changed — 8 files; all 12 findings across both dated sections fixed, each proved RED first
    - test: green — `cargo nextest run --workspace`, 14015 passed, 0 skipped; fmt clean; clippy clean
    - commit: 6258c7d47
    - review: findings — crates/swissarmyhammer-validators/src/review/test_support.rs:142, crates/swissarmyhammer-validators/src/review/tool_health.rs:136, crates/swissarmyhammer-validators/src/review/tool_health/tests.rs:390
    - note: the 12 prior findings are all genuinely fixed, verified against the code, not accepted on report. The new defect is a consequence of the fix for "create the directory only when there is a verdict": the empty-map early return in `save` leaves a stale PASS on disk when doctor drops the last entry.
    - guardrail: no finding repeats from iteration 1. The loop continues.
  timestamp: 2026-08-10T00:53:23.196452+00:00
- actor: claude-code
  id: 01kzmkfx524a9kqbx3t29e6zhk
  text: |-
    The three findings of the 19:35 section are closed. Each one was proved RED first.

    1. `tool_health.rs` — the drop now reaches the disk. `save` no longer returns on an empty map: it calls the new `remove_stored_verdicts`, which deletes the stored file. Delete is taken over writing an empty map because a delete creates nothing, so `a_cache_with_no_verdict_creates_nothing_in_the_workspace` still holds — a workspace with no stored file has nothing to delete. A `NotFound` error is the normal case and is dropped; any other error is logged and the stored pass then stands one more review.
       - RED, from a test that saves, reopens, and reads (a live cache alone does not reproduce it): "a review that follows doctor must not replay the pass doctor dropped", at `tool_health/tests.rs:468`. The old `doctor_proves_the_rule_again_and_drops_the_stored_verdict` PASSED in the same run, which is finding 2 measured.

    2. `tool_health/tests.rs` — new test `a_saved_verdict_that_doctor_drops_does_not_survive_a_reopened_cache`. It crosses the process boundary the defect lives on: each of the three steps (review, doctor, next review) opens its OWN `ToolHealthCache` from the same workspace and saves it, exactly as the three processes do.

    3. `shell_quote` versus `shell_escape_path` — the function moved to a shared location, and every call site now uses the one copy. `swissarmyhammer_common::test_utils::shell_escape_path` is that copy: both crates already depend on `swissarmyhammer-common`, and `test_utils` is an ordinary public module there, so neither crate needed a new edge. The name is `shell_escape_path`, the name the finding gives. Deleted: `shell_quote` in `review/test_support.rs` and the private `shell_escape_path` inside the `#[cfg(test)] mod tests` of `agent-client-protocol-extras/src/hook_config.rs`. Routed: `counting_tool_script`, three sites in `review/tool_install.rs`, two sites in `review/tool_health/tests.rs`, two sites in `hook_config.rs`. The behaviour test moved with the function and is now `shell_escape_path_keeps_a_command_substitution_in_a_path_from_running` in `swissarmyhammer-common`.
       - RED: "error[E0425]: cannot find function `shell_escape_path` in this scope" at `test_utils.rs:1376` and `:1377`, from the moved test before the function existed there.

    Docs corrected along with the code, because each one stated the contract the finding names:
    - the `tool_health` module doc gained a paragraph: the drop reaches the DISK, because doctor and the review are two processes and the file is all they share.
    - `doctor.rs` said doctor "REPLACES what is stored". It now says doctor stores the pass and drops what a rule no longer earns, and that the caller's save is what carries the drop to the next process.
    - `cache_path` says `save` is the one writer: it creates the directory when it has a verdict, and deletes the file when none stands.
    - `ARCHITECTURE.md` gained the same statement in its fact-producer paragraph.

    `synthesize.rs` was not touched; `git diff` reports it unchanged.
  timestamp: 2026-08-10T01:09:46.274801+00:00
- actor: claude-code
  id: 01kzmkgf4jx1k7ahvbs4cq7vdg
  text: |-
    ### implement — changed
    - evidence: 6 files — crates/swissarmyhammer-validators/src/review/tool_health.rs, .../review/tool_health/tests.rs, .../review/test_support.rs, .../review/tool_install.rs, .../src/doctor.rs, crates/swissarmyhammer-common/src/test_utils.rs, crates/agent-client-protocol-extras/src/hook_config.rs, ARCHITECTURE.md
    - findings: 3 of 3 closed in the 2026-08-09 19:35 section; all three dated sections now checked
    - RED, durability: "a review that follows doctor must not replay the pass doctor dropped" (tool_health/tests.rs:468), from a test that saves, reopens, and reads. `doctor_proves_the_rule_again_and_drops_the_stored_verdict` passed in the same run.
    - RED, shared quoting: "error[E0425]: cannot find function `shell_escape_path` in this scope" (test_utils.rs:1376 and :1377).
    - gates: `cargo nextest run --workspace` 14016 tests run, 14016 passed, 0 skipped. `cargo fmt --all --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - next: ready for /review of the same range.
  timestamp: 2026-08-10T01:10:04.690097+00:00
- actor: claude-code
  id: 01kzmmna4tjewwtqsqw4bc5kmg
  text: |-
    ### review — findings
    - evidence: 75 findings recorded, scope `1c8765c91^..1c8765c91` (engine counts: 76 findings, 78 confirmed, 6 refuted, 1 file skipped). All 75 are in `crates/agent-client-protocol-extras/src/hook_config.rs` — 1 prompt-cap finding at :1, and 74 missing-docs findings on struct fields between :88 and :1357. All are production code; `#[cfg(test)]` in that file starts at :1984.
    - dropped under the skill exception: `crates/swissarmyhammer-validators/src/review/test_support.rs:414` (extract `end_line` 10 to a named constant). `seed_chunk` dates to commit 83223a9b0 on 2026-06-11, so the finding asks to restyle test code that already existed.
    - prior findings: all 3 findings of the 2026-08-09 19:35 pass verified fixed, each by reading the code, not by the report.
      1. `tool_health.rs:149` — `save` now calls `remove_stored_verdicts()` on an empty map. `remove_stored_verdicts` (`tool_health.rs:186`) resolves through `cache_path` (`tool_health.rs:38`), which is three `join` calls and creates nothing; `writable_cache_path` is the only creator and the empty branch never reaches it. `std::fs::remove_file` is an unlink, so a concurrent reader holds the old inode or gets NotFound — never a truncated file. NotFound is dropped, every other error is a `tracing::warn!`.
      2. `tool_health/tests.rs` — `a_saved_verdict_that_doctor_drops_does_not_survive_a_reopened_cache` would FAIL against 6258c7d47. Confirmed: the old `save` returned at `if verdicts.is_empty() { return; }` before any write, and `ProbeDirs::break_file` (`tool_health/tests.rs:113`) sits at `base()/break-the-tool`, beside `fixtures/` and not inside it, so breaking the tool changes neither the version nor the content digest. Under the old code step 3 reopens a file that still holds the matching PASS, `passed()` returns true, and the assert `!probe_health(&next, HealthProof::Stored, &ruleset).usable()` fails.
      3. `shell_escape_path` — one definition only, at `crates/swissarmyhammer-common/src/test_utils.rs:811`. `shell_quote` is gone from the whole tree. Nine call sites: `tool_health/tests.rs:128,148`, `tool_install.rs:819,831,865,866`, `test_support.rs:140`, `hook_config.rs:2331,2388`.
    - introduced by the fix: `ToolHealthCache::open` (`tool_health.rs:120`) reads with `read_verdicts(...).unwrap_or_default()`, so a stored file that cannot be parsed or read now opens empty and the following `save()` deletes it, where before it was left alone. The delete only ever names `<workspace_root>/.sah/tmp/review-tool-health.json`; an empty or wrong workspace root moves that one engine-owned filename, and `remove_file` cannot remove a directory.
    - tests: `cargo test -p swissarmyhammer-validators --lib tool_health` — 17 passed, 0 failed.
    - `crates/swissarmyhammer-validators/src/review/synthesize.rs` is not in this commit and is clean in the working tree.
    - next: document the 74 struct fields in `hook_config.rs` and split the file under the 262144-byte review prompt cap.
  timestamp: 2026-08-10T01:30:11.994837+00:00
- actor: claude-code
  id: 01kzmmpw91ahhetpxpdsjep7e9
  text: |-
    ### finish iteration 3 — findings
    - implement: changed — 8 files; all 3 findings fixed, each proved RED first
    - test: green — `cargo nextest run --workspace`, 14016 passed, 0 skipped; fmt clean; clippy clean
    - commit: 1c8765c91
    - review: findings — 75, all in `crates/agent-client-protocol-extras/src/hook_config.rs`: one prompt-cap finding at :1 (366736 rendered bytes against the 262144-byte cap, so `duplication` could not read the file at all), and 74 missing struct-field docs between :88 and :1357
    - note: the 3 targeted findings are verified fixed by reading the code. The 75 new findings are pre-existing debt in a file this commit touched by ONE line — the private `shell_escape_path` copy was deleted from its test module, which put the whole file in scope. `#[cfg(test)]` starts at :1984, so every flagged field is production code and the test-code exception does not apply.
    - guardrail: no finding repeats across the three ledger entries. The loop continues.
    - This is the known pattern: one line pulls a whole file into scope and its debt with it. `tool_rules.rs` was split for the same prompt-cap reason earlier in this work.
  timestamp: 2026-08-10T01:31:03.329055+00:00
- actor: claude-code
  id: 01kzmp4xn3a6cd9h80ycdb6rsa
  text: |-
    Iteration 4. The 75 findings of the 2026-08-09 20:11 section are closed. All 75 were in `crates/agent-client-protocol-extras/src/hook_config.rs`.

    1. The prompt-cap finding at `:1`. The file is split, on the shape of the `tool_rules.rs` precedent: the parent file keeps only the module doc, the `mod` list, and the `pub use` list, and each subject gets its own file. Tests move to a `tests.rs` beside the code they test.

       New tree under `crates/agent-client-protocol-extras/src/hook_config/`:
       - `event.rs` — `HookCommandContext`, `SessionSource`, `HookEvent`, `HookEventKind`, and every `json_*` builder.
       - `decision.rs` (+ `decision/tests.rs`) — `HookDecision`, the `HookHandler` and `HookEvaluator` traits, `Matcher`, `HookRegistration`.
       - `config.rs` (+ `config/tests.rs`) — `HookConfig`, `MatcherGroup`, `HookEventKindConfig`, `UnsupportedEventKind`, `HookHandlerConfig`, `HookConfigError`, and the factory that makes registrations.
       - `output.rs` (+ `output/tests.rs`) — `HookDecisionValue`, `HookOutput`, `HookOutputBuilder`, `HookSpecificOutput`, `PromptHookResponse`, and the rules that turn one into a `HookDecision`.
       - `handlers.rs` (+ `handlers/tests.rs`) — `CommandHandler` and `EvaluatorHandler`.

       Measured rendered size of each new file, in file bytes plus 22 bytes for each line, against the 262144-byte cap. The largest is 51291 bytes, which is 19.6 percent of the cap; the file before the split was 190403 bytes by the same measure, and 366736 bytes as the engine measured it with the `duplicates` probe evidence added.
       - `hook_config.rs` 4127
       - `hook_config/event.rs` 51291
       - `hook_config/decision.rs` 12642
       - `hook_config/config.rs` 22616
       - `hook_config/output.rs` 27484
       - `hook_config/handlers.rs` 16998
       - `hook_config/decision/tests.rs` 5854
       - `hook_config/config/tests.rs` 36278
       - `hook_config/output/tests.rs` 23751
       - `hook_config/handlers/tests.rs` 7092

       The split also cuts the probe evidence, which is what put the file 176333 bytes above its own text: the `duplicates` probe binds to one file, so each part now carries only the duplicate rows of its own definitions.

    2. The 74 missing struct-field docs. Every field has a real doc comment that says what the field means and, for an `Option` field, what an absent value does to the hook JSON. The `HookEvent` and `HookSpecificOutput` fields are wire fields of the Claude Code hook protocol, so each doc names the JSON key and its meaning, not the Rust field name.

    Visibility changes the split forced, all as narrow as possible:
    - `output.rs` — `is_blockable`, `feeds_stderr_to_agent`, `interpret_output`, and `interpret_prompt_response` are `pub(super)`, because `handlers.rs` calls them.
    - `handlers.rs` — `CommandHandler` and `EvaluatorHandler`, with their fields, are `pub(super)`, because the factory in `config.rs` builds them.
    Nothing became `pub` or `pub(crate)`. The public surface of the module is the same: the parent file re-exports all 20 public types, so `hook_config::<Type>` still resolves and `lib.rs` needs no change.

    Two facts checked, not assumed:
    - No test was lost or renamed. The 66 test functions of the old `mod tests` are the same 66 the new tree lists; `comm` of the old names against `cargo nextest list` shows no name only on one side.
    - The `Stop` variant kept its field order. The first draft put `stop_hook_active` before `stop_reason` and gave each the doc of the other. That was corrected before the build.
  timestamp: 2026-08-10T01:56:12.067743+00:00
- actor: claude-code
  id: 01kzmp573s336qfh8kfc7mspa8
  text: |-
    ### implement — changed
    - evidence: 11 files — `crates/agent-client-protocol-extras/src/hook_config.rs` (now the module root), and new: `.../hook_config/event.rs`, `.../hook_config/decision.rs`, `.../hook_config/decision/tests.rs`, `.../hook_config/config.rs`, `.../hook_config/config/tests.rs`, `.../hook_config/output.rs`, `.../hook_config/output/tests.rs`, `.../hook_config/handlers.rs`, `.../hook_config/handlers/tests.rs`
    - findings: 75 of 75 closed in the 2026-08-09 20:11 section; all four dated sections are now checked (90 of 90 items)
    - rendered size after the split, in file bytes plus 22 for each line, against the 262144-byte cap: 4127, 51291, 12642, 22616, 27484, 16998, 5854, 36278, 23751, 7092. The largest is 51291, which is 19.6 percent of the cap.
    - gates: `cargo nextest run --workspace` 14016 tests run, 14016 passed, 0 skipped. `cargo fmt --all --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - `crates/swissarmyhammer-validators/src/review/synthesize.rs` is not touched; `git diff` reports it unchanged.
    - next: ready for /review of the same range.
  timestamp: 2026-08-10T01:56:21.753470+00:00
- actor: claude-code
  id: 01kzmrjgjkkxw3b804gyfe6885
  text: |
    ### review — findings
    - scope: `review sha f6b70d9ab^..f6b70d9ab` (the hook_config.rs split)
    - engine counts: 7 findings, 7 confirmed, 1 refuted, 18 attempted, 0 failed, **0 skipped, skipped_files empty**
    - recorded: 6 findings — handlers.rs:188 (x2), output.rs:264 (x2), output.rs:276, output.rs:436. All are duplication of production code between `handlers.rs` and `output.rs` after the split: the event-kind decision tree appears twice, and the `EVENT_PROPERTIES` lookup appears twice.
    - dropped 1 finding under the review skill's written blanket exception for test code that already existed: `hook_config/handlers/tests.rs:38` asked to replace the hardcoded `exit 2` with a named constant. That string existed verbatim before this commit at `hook_config.rs:2772`, so the finding's subject is changing pre-existing test code.
    - prior-pass findings verified fixed by my own measurement, not by the engine alone:
      - prompt cap: every part measured as file bytes + 22/line. Largest is `hook_config/event.rs` at 51291 (32063 bytes, 874 lines) against the 262144 cap. Module root `hook_config.rs` is 4127. Engine confirmed no file was skipped.
      - module root is only a module doc, 5 `mod` lines, and 20 re-exports (73 lines).
      - public surface unchanged: 20 public types re-exported; `lib.rs` and `hookable_agent.rs` are not in the commit. 6 items went private -> `pub(super)` (structs `CommandHandler`, `EvaluatorHandler`; fns `is_blockable`, `feeds_stderr_to_agent`, `interpret_output`, `interpret_prompt_response`). Zero `pub(crate)`. Nothing became `pub`.
      - tests: 66 test functions before, 66 after, name sets identical (comm shows no difference in either direction).
      - field docs accurate, not filler. `stop_reason` doc says the JSON sends the debug text of the value, which matches `obj["stop_reason"] = String(format!("{:?}", stop_reason))`; `stop_hook_active` doc says the JSON sends the bool and a hook reads it to prevent an endless loop, which matches `obj["stop_hook_active"] = Bool(...)`. The two are not swapped. Every `HookSpecificOutput` doc names the same key as its `#[serde(rename = ...)]`. Every JSON key named in an `event.rs` field doc is genuinely emitted — `cwd` and `hook_event_name` by `base_event_json`, and `branch_name`/`teammate_id`/`task_id`/`task_title` through `build_session_event_with_optional_string`.
    - `crates/swissarmyhammer-validators/src/review/synthesize.rs` is not in this commit and is not modified in the working tree.
    - next: remove the duplicated decision tree and the duplicated `EVENT_PROPERTIES` lookup, then re-review.
  timestamp: 2026-08-10T02:38:34.579391+00:00
- actor: claude-code
  id: 01kzmrknkymemc3sybqzzrsxqw
  text: |-
    ### finish iteration 4 — findings
    - implement: changed — 11 files; `hook_config.rs` split into `hook_config/` with 5 subject files and 4 test files; 74 struct-field docs written
    - test: green — `cargo nextest run --workspace`, 14016 passed, 0 skipped; fmt clean; clippy clean
    - commit: f6b70d9ab
    - review: findings — 6, two distinct causes: the event-kind decision tree written twice (`hook_config/handlers.rs:188` and `hook_config/output.rs:436`), and the `EVENT_PROPERTIES` table lookup written twice (`hook_config/output.rs:264` and `:276`)
    - note: all 75 prior findings are fixed and verified. The cap is measured, not assumed — largest part 51291 of 262144, and the engine reported `skipped: 0` with an empty `skipped_files` list. 66 test functions before, 66 after, identical name sets. The public surface did not move: `lib.rs` and `hookable_agent.rs` are not in the commit, six items went private to `pub(super)`, nothing became `pub` or `pub(crate)`.
    - The new findings are the split working as designed. `duplication` could not read this file before, so it never reported the duplication that was already there.
    - guardrail: no finding repeats across the four ledger entries. The loop continues.
  timestamp: 2026-08-10T02:39:12.510182+00:00
- actor: claude-code
  id: 01kzmsdwgbxxaagt2ne9x06fbr
  text: |-
    Iteration 5. The six findings of the 2026-08-09 20:59 section are closed. They name two causes, and each cause is removed one time.

    1. The event-kind decision tree, written twice (`hook_config/handlers.rs` in `interpret_exit_2_stderr`, and `hook_config/output.rs` in `interpret_prompt_response`).

       `output.rs` gained `decide_by_event_kind(event_kind, reason) -> HookDecision`, which holds the four rules and nothing else. Both call sites now call it, and the tree exists one time.

       The logging stays at the call site, because only the call site knows what to name. `interpret_exit_2_stderr` logs when the result is `HookDecision::Allow`, which is exactly the `else` branch the log sat in before: `Allow` is the only decision the tree can give from that branch, so the condition selects the same case. The other three decisions keep the message of the hook, so they need no log.

       Why this name: two of the three decision-tree findings name `decide_by_event_kind` with the argument order `(event_kind, reason)`, so the code takes the name and the order the findings give.

    2. The `EVENT_PROPERTIES` table lookup, written twice (`is_blockable` and `feeds_stderr_to_agent`).

       The table rows are now `(HookEventKind, EventProperties)`, where `EventProperties` is a small `Copy` struct with the two named fields. One function, `event_properties(kind)`, searches the table and gives `EventProperties::NONE` for a kind the table does not list. `is_blockable` and `feeds_stderr_to_agent` are each one line over that lookup, and they keep the doc that says why each property holds.

       The engine asked for a closure selector, and one of its two forms asked for an index parameter that means "which tuple field". Both were rejected for a named struct field: a reader of `properties.blockable` needs no key to a position, and `unwrap_or(false)` becomes `EventProperties::NONE`, which states the default one time instead of twice. Neither field moved, and the default did not change, so the lookup answers exactly what it answered before.

    Visibility: `is_blockable` and `feeds_stderr_to_agent` are now private to `output.rs`, because `handlers.rs` was their only outside caller and it now calls `decide_by_event_kind`. `decide_by_event_kind` is `pub(super)`, the same visibility the two functions had. Nothing became `pub` or `pub(crate)`.

    No behavior changed. Which tests hold each path:

    - `interpret_prompt_response`, all four branches: `test_prompt_response_ok_false_blocks` (Block, PreToolUse), `test_prompt_response_ok_false_stop_is_should_continue` (ShouldContinue), `test_prompt_response_ok_false_post_tool_feeds_context` and `test_prompt_response_ok_false_post_tool_failure_feeds_context` (AllowWithContext), and the new `test_prompt_response_ok_false_on_a_silent_event_allows` (Allow, Notification). `test_prompt_response_ok_true` holds the `ok: true` short path, which does not reach the helper.
    - `interpret_exit_2_stderr`, all four branches: `test_exit_2_on_blockable_event_blocks` (Block, PreToolUse and UserPromptSubmit), `test_exit_2_on_silent_events_allows` (Allow, Notification and SessionStart), and the new `exit_2_on_a_stop_event_asks_the_agent_to_continue` (ShouldContinue) and `exit_2_on_a_post_tool_event_gives_the_stderr_to_the_agent` (AllowWithContext, both tool kinds). The two new tests assert the reason text as well, so the reason still reaches the decision it belongs to.
    - The log condition: `exit_2_on_a_silent_event_logs_the_fall_back_to_allow` asserts the warning IS written for `Allow`, and `exit_2_on_a_blockable_event_logs_no_fall_back` asserts it is NOT written for `Block`.
    - End to end, through the whole hook path, unchanged and green: `e2e_hooks::exit2_tests` and `e2e_hooks::json_output_tests` (317 tests in the crate, all pass).

    Note: the four branches of `interpret_exit_2_stderr` had no test for Stop and no test for the tool kinds before this change, and neither call site had a test for its own `Allow` branch. The union of the two suites covered the four rules; neither suite alone did. Both suites now cover all four, so a later edit of one call site cannot pass on the coverage of the other.

    RED, measured, not assumed: the log condition was inverted to `!matches!(decision, HookDecision::Allow)`. `exit_2_on_a_silent_event_logs_the_fall_back_to_allow` failed with "a refusal that becomes Allow must log the fall back", `exit_2_on_a_blockable_event_logs_no_fall_back` failed with "a block keeps the message of the hook, so it must not log the fall back", and the other nine tests of the module passed. The condition is back.

    Test-file duplication avoided, not added: the four new handler tests need a hook command that exits 2 with a message on stderr. That block was written out twice in the file already, so it is now the helper `refusing_hook_output(message)`, and the two tests that held the copies call it. The helper writes `exit {EXIT_CODE_BLOCK}` with the named constant the module already declares, in place of a literal 2.

    Rendered size of every part, file bytes plus 22 for each line, against the 262144-byte cap: `hook_config.rs` 4127, `event.rs` 51291, `config/tests.rs` 36278, `output.rs` 31766, `output/tests.rs` 24492, `config.rs` 22616, `handlers.rs` 17035, `decision.rs` 12642, `handlers/tests.rs` 11280, `decision/tests.rs` 5854. The largest is 51291, which is 19.6 percent of the cap. `output.rs` grew by 4282 bytes and `handlers.rs` by 37; both stay far under.

    `crates/swissarmyhammer-validators/src/review/synthesize.rs` is not touched; `git diff --stat` on it reports nothing.
  timestamp: 2026-08-10T02:53:31.531825+00:00
- actor: claude-code
  id: 01kzmse797m2va8vd75ryw3t9e
  text: |-
    ### implement — changed
    - evidence: 4 files — crates/agent-client-protocol-extras/src/hook_config/output.rs, .../hook_config/output/tests.rs, .../hook_config/handlers.rs, .../hook_config/handlers/tests.rs
    - findings: 6 of 6 closed in the 2026-08-09 20:59 section; all five dated sections now checked (96 of 96 items)
    - tests for each path: `interpret_prompt_response` — test_prompt_response_ok_false_blocks, ..._stop_is_should_continue, ..._post_tool_feeds_context, ..._post_tool_failure_feeds_context, and the new ..._on_a_silent_event_allows. `interpret_exit_2_stderr` — test_exit_2_on_blockable_event_blocks, test_exit_2_on_silent_events_allows, and the new exit_2_on_a_stop_event_asks_the_agent_to_continue and exit_2_on_a_post_tool_event_gives_the_stderr_to_the_agent. The log condition — the new exit_2_on_a_silent_event_logs_the_fall_back_to_allow and exit_2_on_a_blockable_event_logs_no_fall_back.
    - RED: the log condition inverted; the two log tests failed and the other nine of the module passed. The condition is back.
    - rendered size, file bytes plus 22 for each line, against the 262144-byte cap: 4127, 51291, 36278, 31766, 24492, 22616, 17035, 12642, 11280, 5854. The largest is 51291, which is 19.6 percent of the cap.
    - gates: `cargo nextest run --workspace` 14021 tests run, 14021 passed, 0 skipped. `cargo fmt --all --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - `crates/swissarmyhammer-validators/src/review/synthesize.rs` is not touched.
    - next: ready for /review of the same range.
  timestamp: 2026-08-10T02:53:42.567305+00:00
- actor: claude-code
  id: 01kzmt4e5nt7efj6b5wch90p7r
  text: |
    ### review — findings
    - scope: `review sha 6e5c8a288^..6e5c8a288`
    - evidence: 1 finding — crates/agent-client-protocol-extras/src/hook_config/handlers/tests.rs:16 (findings 1, confirmed 1, refuted 3, attempted 9, failed 0, skipped 0, skipped_files [])
    - verified: the six duplication findings of the prior pass are fixed; the log condition of `interpret_exit_2_stderr` is exactly equivalent to the old `else` arm; `EventProperties::NONE` keeps the old `unwrap_or(false)`; the five new tests assert the branch each names; behavior does not change; every file is under the 262144-byte rendered cap (max 31766); `synthesize.rs` is not in the commit and has no uncommitted change
    - next: escape the `message` argument of `refusing_hook_output` before it reaches the shell
  timestamp: 2026-08-10T03:05:50.517634+00:00
- actor: claude-code
  id: 01kzmt5mavzgc02xt998ag6kbk
  text: |-
    ### finish iteration 5 — findings
    - implement: changed — 4 files; both duplications removed at the cause, 5 new tests close branches neither call site covered
    - test: green — `cargo nextest run --workspace`, 14021 passed, 0 skipped; fmt clean; clippy clean
    - commit: 6e5c8a288
    - review: findings — 1, `crates/agent-client-protocol-extras/src/hook_config/handlers/tests.rs:16`, the new `refusing_hook_output` helper interpolates a message into a shell command string
    - note: all 6 prior findings verified fixed. The log condition was proved equivalent from the variant set, not the input set: the old `else` arm returned the unit variant `Allow`, and the other three arms return `Block`, `ShouldContinue`, and `AllowWithContext`, so `matches!(decision, HookDecision::Allow)` is true for exactly the old `else` inputs — and stays true for event kinds added later. `EventProperties::NONE` keeps the old `unwrap_or(false)`.
    - guardrail: this is the THIRD instance of the shell-interpolation class on this card (`binary_present` on ^t0dgame, `test_support.rs:136`, now this helper), but each is a different file and a different line, so the guardrail does not apply. It is a new instance the last iteration introduced, and the fix already exists in the workspace.
  timestamp: 2026-08-10T03:06:29.595181+00:00
- actor: claude-code
  id: 01kzmtwqha3h0gn4fx7sf621ab
  text: |-
    Finding of the 2026-08-09 21:55 pass, `handlers/tests.rs:16`, is fixed.

    Mechanism chosen: the positional parameter, NOT a new escape helper. Reasons:
    - `shell_escape_path` (`crates/swissarmyhammer-common/src/test_utils.rs`) takes a `&Path`. The call site has a `&str` message, which is not a path. To pass it as `Path::new(message)` makes the doc of that helper false.
    - The doc of `shell_escape_path` states the rule: "A script that takes its value as a positional parameter needs no quoting at all, and is the better shape where it fits." It fits here.
    - This is the shape `^t0dgame` used for `binary_present` (`which "$@"`) and the shape the tool rules use for changed files. The workspace keeps ONE escape helper, and it did not grow a fourth copy.

    The change:
    - `refusing_hook_output` builds `echo "$1" >&2; exit {EXIT_CODE_BLOCK}` and gives the message as `$1`. The script holds only the block-code constant, which comes from code.
    - New private helper `shell_output(script, value)` calls `swissarmyhammer_common::command::shell_command(Shell::Platform, script)` — the one place in the workspace that decides the interpreter and the stream wiring — then adds `$0` and the value.
    - `malformed_hook_specific_output_allows_with_an_explicit_asserted_log` had the same cause (`echo '{stdout}'`). It now calls `shell_output` too, so the cause is gone from the whole file.

    RED first. New test `a_hostile_message_reaches_stderr_as_data` with the message `don't run $(echo INJECTED) or `echo INJECTED`` failed before the fix:

        assertion `left == right` failed
          left: "sh: -c: line 0: unexpected EOF while looking for matching `''\nsh: -c: line 1: syntax error: unexpected end of file"
         right: "don't run $(echo INJECTED) or `echo INJECTED`"

    The shell read the message as code, so the reason the handler reported was the shell's own error text, not the message. GREEN after the fix.
  timestamp: 2026-08-10T03:19:06.538369+00:00
- actor: claude-code
  id: 01kzmtx1bvrfsbwz2g2vcrnj97
  text: |-
    ### implement — changed
    - evidence: 1 file — crates/agent-client-protocol-extras/src/hook_config/handlers/tests.rs. Gates: `cargo nextest run --workspace` 14022 tests run, 14022 passed, 0 failed, 0 skipped; `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean. `crates/swissarmyhammer-validators/src/review/synthesize.rs` has no change. HEAD is 6e5c8a288, no commit made.
    - next: /review
  timestamp: 2026-08-10T03:19:16.603520+00:00
- actor: claude-code
  id: 01kzmv935dk4ah1zfwab8s0d1a
  text: |
    ### review — clean

    - scope: `review sha 7b7c3f23a^..7b7c3f23a` (one commit, one file: `crates/agent-client-protocol-extras/src/hook_config/handlers/tests.rs`)
    - counts: 0 findings, 0 confirmed, 0 refuted, 9 attempted, 0 failed, skipped 0, skipped_files empty
    - cap: the file renders at 8951 bytes against the 262144-byte cap. Every file of the commit was reviewed.
    - prior findings: every item of all six earlier `## Review Findings` sections is checked.

    Verification of the claims of the commit:

    - The script takes the message as a positional parameter. `refusing_hook_output` builds `echo "$1" >&2; exit {EXIT_CODE_BLOCK}` and passes the message as an argument. `shell_output` gives `shell_command` the script, then `SHELL_ARGV0` for `$0`, then the value for `$1`. The value never joins the command string, so no value can be read as code.
    - The helper goes through `swissarmyhammer_common::command::shell_command` (`crates/swissarmyhammer-common/src/command.rs:47`), which picks the interpreter and sets stdin null, stdout piped, and stderr piped. The explicit `.stdout(piped())` / `.stderr(piped())` the old code spelled are now set in that one place, so capture is kept.
    - No new escaping code and no new dependency. The commit touches one file; no `Cargo.toml` and no `Cargo.lock` changed. A workspace grep finds exactly one escaping helper, `shell_escape_path` (`crates/swissarmyhammer-common/src/test_utils.rs:811`), and no `shell_escape` crate in any manifest. The engine's earlier remedy asked for that crate; reusing `shell_command` meets the same requirement with no fourth helper.
    - The same cause is gone from both places. The malformed-output test now calls `shell_output(r#"echo "$1""#, stdout)`. The file holds no `Command::new("sh")`, no `arg("-c")`, and no message or stdout interpolated into a command string.
    - The new test proves the message arrives as data. `a_hostile_message_reaches_stderr_as_data` sends `HOSTILE_MESSAGE`, which holds `$(echo INJECTED)`, a backtick substitution, and a single quote. It asserts the exit code stays `EXIT_CODE_BLOCK` and that the reason `interpret_exit_2_stderr` reads from stderr equals the message byte for byte.
    - `crates/swissarmyhammer-validators/src/review/synthesize.rs` is not in the commit and has no uncommitted change.
    - tests: `cargo nextest run --package agent-client-protocol-extras -E 'test(hook_config::handlers::tests)'` — 12 passed, 0 failed.

    - next: none. Task moved to done.
  timestamp: 2026-08-10T03:25:51.661076+00:00
- actor: claude-code
  id: 01kzmvaa5v72hx4vwg2e4stare
  text: |-
    ### finish iteration 6 — clean
    - implement: changed — 1 file; the test script takes the message as a positional parameter (`"$1"`), through `swissarmyhammer_common::command::shell_command`. No new escaping code, no new dependency. The same cause was in the file a second time and is gone from both.
    - test: green — `cargo nextest run --workspace`, 14022 passed, 0 skipped; fmt clean; clippy clean
    - commit: 7b7c3f23a
    - review: clean — 0 findings, 9 attempted, 0 failed, `skipped: 0`, `skipped_files: []`. All 97 findings across five dated sections are checked.
    - result: the card is in `done` after six iterations.
    - note: the engine's own remedy was to install a `shell_escape` crate. That was refused, and the refusal was then verified: no `Cargo.toml` or `Cargo.lock` is in the commit, and the workspace still holds exactly one escaping helper.
  timestamp: 2026-08-10T03:26:31.611049+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffd480
title: 'tool rules: cache the doctor/fixture verdict and overlap the tool run with the fan-out'
---
Every review re-proves tool health from scratch, serially. The 2026-08-07 dogfood run (../swissarmyhammer-main/.sah/mcp.7341.log): ~50s of doctor fixture verification (the fixtures run real cargo clippy twice, fail + pass) plus 42s of workspace clippy for the actual judgment — about 100 of 368 seconds, all before the LLM fan-out starts. The health verdict cannot change until the tool version changes, and the tax grows with every tool rule (six missing-docs rules today; the complexity rules will add more cargo runs).

Two changes:
1. Cache the doctor + fixture verdict, keyed on (tool version string, rule content hash). A hit skips the fixture runs. A miss, a version change, or a rule edit re-verifies. Store beside the review engine's other state; `sah doctor` always re-verifies and refreshes the cache — doctor stays the ground truth.
2. Run the tool `run` scripts concurrently with the LLM fan-out. The suppression plan needs only the HEALTH verdict up front (it decides which prompt rules are skipped); the tool FINDINGS are only needed at synthesis. Keep the plan step; move the execution to overlap the fleet.

Acceptance: a second `review working` on an unchanged toolchain shows no fixture clippy runs in the log, and the tool run no longer delays the first fleet task.

#tool-validators

## Review Findings (2026-08-09 18:37)

- [x] `crates/swissarmyhammer-validators/src/review/drive.rs:2001` — Hardcoded worker thread count (4) differs from the test suite pattern (all other tests use 2) and lacks explanation for why this test needs higher concurrency. Extract to a named constant (e.g., `const TEST_POOL_WORKERS_OVERLAP: usize = 4;`) and add a comment explaining why higher concurrency is required for this specific test.
- [x] `crates/swissarmyhammer-validators/src/review/test_support.rs:136` — Command injection via unescaped path interpolation in shell script. Path is interpolated with only double quotes, insufficient to escape special characters like backticks, $(), or embedded quotes. Use shell-safe quoting for the path. Replace `"{counter}"` with `'{}' shell-quoted version using printf %q or similar escaping that handles all special characters.
- [x] `crates/swissarmyhammer-validators/src/review/tool_health.rs:119` — `ToolHealthCache::save` holds the `MutexGuard` returned by `self.verdicts()` live across the `match serde_json::to_vec_pretty(&*verdicts)` scrutinee AND both of its arms, including the blocking `std::fs::write` call. This is the same guard-across-a-match-scrutinee-and-its-arm shape this change states it removed, so the pattern survives in the new module. It does not deadlock today only because no arm re-enters the lock, which makes the safety incidental rather than structural, and it holds the lock across filesystem I/O. Serialize into an owned value and drop the guard before the `match`.
- [x] `crates/swissarmyhammer-validators/src/review/tool_health/tests.rs:137` — No test proves that a rule reporting no tool version is never stored. The module states this invariant at `tool_health.rs:20` and `VerdictKeys::of` implements it by returning `None` at the `version?` in `tool_health.rs:192`, but all five tests build a probe rule that always sets `check_version_command` (`tool_health/tests.rs:115`), so a regression that stored a version-less verdict would pass the suite. Add a test whose rule declares no `check_version_command`, asserting the fixtures run on every probe and that no verdict is written under that key.

## Review Findings (2026-08-09 19:12)

- [x] `crates/swissarmyhammer-validators/src/review/tool_health.rs:162` — A negative fixture verdict is sticky, so one transient failure disables a tool rule until the tool version or the rule content changes. `prove` stores whatever `check_fixtures` returned, including `FixtureOutcome::Failed` and `MissingFixtures`, and `stored` (`tool_health.rs:147`) replays the stored variant with no check of which variant it is. `verify_fixture_contract` maps every `ScriptFailure::Start` (io error) and `ScriptFailure::Exit` (nonzero) to `Failed`, which for a `workspace`-scope Rust rule includes a `cargo clippy` that lost the build lock, ran out of disk, or hit a network failure. That `Failed` is written to `.sah/tmp/review-tool-health.json` and replayed by `plan_rule_by_health` (`tool_rules.rs:624`) as `!status.usable()` -> prompt fallback on every later review. The module's stated justification, that the verdict cannot change while the tool and the rule stay the same, does not hold for a failure whose cause is environmental. Store only `FixtureOutcome::Passed` and prove any non-passing rule again every run, and add a test that a rule which failed its fixtures once is proved again on the next check.
- [x] `crates/swissarmyhammer-validators/src/review/tool_health.rs:14` — The module doc states the digest covers "the rule name the fixture files are named for, the rule's whole `tool` block, and every file in the set's `fixtures/` directory". `VerdictKeys::of` (`tool_health.rs:189`) does not take `rule` at all, and `content_digest` (`tool_health.rs:251`) hashes only the `ToolSpec` and the fixture digest. The rule name reaches the store through `verdict_key` (`tool_health.rs:241`), which is the storage key and a different mechanism. A reader who relies on the doc would conclude that two rules in one set with identical `tool` blocks get different digests, which is false. Correct the doc to say the rule name is part of the storage key, not the digest.
- [x] `crates/swissarmyhammer-validators/src/review/tool_health.rs:291` — `fixture_digest` concatenates each path and its content into one unframed byte stream (`hasher.update(path...); hasher.update(bytes);`), so two different fixture sets can hash the same. Renaming `probe-tool.pass.rs` with content `XY` to `probe-tool.pass.rsX` with content `Y` produces an identical digest, and both names remain live fixtures because `find_fixture` (`doctor.rs:456`) matches on the `"{rule}.{kind}."` prefix with `starts_with`. The stored verdict then stands for a fixture set it never proved. Frame each entry — hash the path length and the content length as fixed-width bytes before each blob, or hash a per-file digest and fold the results.
- [x] `crates/swissarmyhammer-validators/src/review/tool_health.rs:294` — A fixture file that fails to read hashes identically to an empty file. `if let Ok(bytes) = std::fs::read(&path) { hasher.update(bytes); }` drops the error, so a permission or I/O failure on one fixture produces the same digest as that fixture being empty, and a verdict proved against an empty fixture is replayed for an unreadable one. This contradicts the doc at `tool_health.rs:266`, which states the digest covers every file by name and by content. Fold the error into the digest with a distinct sentinel, or return a digest that cannot match so the rule is proved again.
- [x] `crates/swissarmyhammer-validators/src/review/tool_health.rs:303` — Opening the cache writes into the repository under review. `cache_path` calls `ManagedDirectory::from_custom_root(workspace_root)`, whose `new` (`crates/swissarmyhammer-directory/src/directory.rs:91`) unconditionally creates `<repo>/.sah/`, writes `<repo>/.sah/.gitignore` (`directory.rs:113`), and creates `<repo>/.sah/tmp/`. `synthesize.rs:655` calls this on every review, so it is a new side effect of running a review on a repo. The verdict file is covered by that `.gitignore` (`tmp/`), but `.sah/.gitignore` itself is a new untracked file in the reviewed tree, and `Scope::Working` includes untracked files, so a review of a repo that does not already ignore `.sah/` pollutes its own next scope. (This repo ignores `**/.sah/` at `.gitignore:166`, so the pollution does not appear here.) Resolve the path without creating anything, and create the directory inside `save()` only when there is a verdict to write.
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules.rs:691` — The `Err` arm of `ToolRunsInFlight::finish()` has no test. The change states that a task which does not finish reports one `ToolRunError` per run it carried, so no run is lost in silence; the claim that the error count equals the run count and that the validator and rule names survive rests on code reading alone. Add a test that plans two runs, forces the blocking task to panic, calls `finish()`, and asserts two `ToolRunError`s carrying the two distinct (validator, rule) pairs.
- [x] `crates/swissarmyhammer-validators/src/review/tool_health/tests.rs:115` — No test covers a rule whose `check_version_command` is present but reports nothing usable. `check_version` (`doctor.rs:377`) returns `None` on a nonzero exit and on empty stdout as well as on an absent command, and each of those must leave the rule unstored. `ProbeDirs::ruleset` always sets a command that succeeds, so none of these branches is driven. Add a test whose rule sets `check_version_command` to a failing command, proved twice, asserting the fixtures run on both probes.
- [x] `crates/swissarmyhammer-validators/src/review/tool_health/tests.rs:192` — Fixture addition and deletion are untested. `an_edited_fixture_proves_the_rule_again` only overwrites an existing file. The doc at `tool_health.rs:268` argues the whole directory counts because `materialize_fixtures` copies the fixture's neighbours into the scratch directory, which is exactly the addition and deletion case, so a digest narrowed to only the two named fixtures would pass the current suite. Add one test that writes a new neighbour file into `fixtures/` and one that deletes it, each asserting the fixtures ran again.

## Review Findings (2026-08-09 19:35)

- [x] `crates/swissarmyhammer-validators/src/review/test_support.rs:142` — Function `shell_quote` reimplements shell path escaping instead of reusing the existing `shell_escape_path` function that already exists in the codebase. Import and reuse `shell_escape_path` from `hook_config.rs` (or move it to a shared location if access is restricted), rather than duplicating this shell-escaping logic. If the existing function's contract differs slightly, extend it to cover both use cases instead of implementing a parallel version.
- [x] `crates/swissarmyhammer-validators/src/review/tool_health.rs:136` — A verdict that `sah doctor` drops does not reach the disk when it was the last one, so doctor cannot invalidate a stored pass. `save` returns at `if verdicts.is_empty() { return; }` before it writes, and `tool_health.rs` holds no `remove_file`, so an empty map leaves the old file exactly as it was. `prove` (`tool_health.rs:227`) drops a non-passing rule with `self.verdicts().remove(&key)`, and `check_review_engine` (`doctor.rs:230`) opens the cache, proves every rule under `HealthProof::Fresh` (`doctor.rs:274`), then calls `health.save()` (`doctor.rs:233`) in a process of its own. A workspace whose stored file holds one entry, or whose whole toolchain broke, therefore keeps the stale PASS on disk after doctor proved the rule broken, and the next `review working` reads that PASS under `HealthProof::Stored` and skips the fixtures. This is the case the change is written for — the module doc (`tool_health.rs:40`) states "a pass replaces the stored verdict, and anything else drops it. Doctor therefore stays the ground truth, and a review that follows it never replays a pass the tool no longer earns", and `ARCHITECTURE.md:563` states the same. A drop of a subset persists only because the surviving entries rewrite the whole file. Make the drop durable: write the empty map, or delete the file, when a verdict was dropped and none remains.
- [x] `crates/swissarmyhammer-validators/src/review/tool_health/tests.rs:390` — `doctor_proves_the_rule_again_and_drops_the_stored_verdict` proves the drop only inside one live `ToolHealthCache`. It never calls `save()` and never reopens the cache, so it does not cover the shape doctor actually runs in: `sah doctor` is a separate process from the review, and the drop reaches the next review only through the file. `a_stored_verdict_survives_a_reopened_cache` (`tool_health/tests.rs:459`) covers the reopen for a stored pass and not for a drop. Add a test that stores a pass, saves it, breaks the tool, runs doctor, saves again, and asserts that a cache reopened from the same workspace proves the rule rather than replaying the pass.

## Review Findings (2026-08-09 20:11)

Scope: `1c8765c91^..1c8765c91`

> WARNING: 1 file(s) not reviewed — the rendered prompt would exceed the agent's prompt cap:
> - `crates/agent-client-protocol-extras/src/hook_config.rs` — 366736 rendered bytes, over the 262144-byte per-file cap; not reviewed by: duplication (split the file)

- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:1` — This file exceeds the review prompt cap — 366736 rendered bytes against the 262144-byte per-file cap — so these validators could not review it: duplication. Split the file into smaller modules that fit the review prompt cap.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:88` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:89` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:90` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:94` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:95` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:96` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:100` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:101` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:102` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:103` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:104` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:108` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:109` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:110` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:111` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:112` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:113` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:117` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:118` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:119` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:120` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:121` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:122` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:126` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:127` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:128` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:129` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:133` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:134` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:138` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:139` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:140` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:141` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:142` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:143` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:147` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:148` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:149` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:150` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:151` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:152` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:156` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:157` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:158` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:162` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:163` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:164` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:168` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:169` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:170` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:173` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:175` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:178` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:179` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:180` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:184` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:185` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:186` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:187` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:782` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:784` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:786` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:789` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:793` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:1321` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:1323` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:1325` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:1327` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:1332` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:1337` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:1342` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:1347` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:1352` — missing documentation for a struct field.
- [x] `crates/agent-client-protocol-extras/src/hook_config.rs:1357` — missing documentation for a struct field.

## Review Findings (2026-08-09 20:59)

Scope: `f6b70d9ab^..f6b70d9ab`

The prior prompt-cap finding and the 74 missing-field-doc findings are fixed and verified: the engine reported `skipped: 0` with an empty `skipped_files` list, so every file of the commit was reviewed. The largest part renders at 51291 bytes against the 262144-byte cap.

- [x] `crates/agent-client-protocol-extras/src/hook_config/handlers.rs:188` — The decision-making logic in `interpret_exit_2_stderr` (lines 188–201) is nearly identical to the logic in `interpret_prompt_response` in output.rs (lines 436–444). Both follow the same pattern: check `is_blockable()` → `Block`, check `Stop` event → `ShouldContinue`, check `feeds_stderr_to_agent()` → `AllowWithContext`, else → `Allow`. The duplication means maintenance burden if this logic ever needs to change — both implementations must be kept in sync. Extract a shared helper function `fn interpret_deny_as_decision(reason: String, event_kind: HookEventKind) -> HookDecision` containing the core decision logic (lines 188–201 without the logging). Call it from both `interpret_exit_2_stderr` and `interpret_prompt_response`. In `interpret_exit_2_stderr`, log the warning before calling the helper, or conditionally log if the result is `Allow`.
- [x] `crates/agent-client-protocol-extras/src/hook_config/handlers.rs:188` — The event-kind-based decision logic (lines 188-200) is duplicated in output.rs:436-444. This conditional tree—checking is_blockable, Stop, and feeds_stderr_to_agent in sequence—should be extracted into a shared helper to keep one canonical implementation and prevent divergence. Extract the decision logic into a shared helper function: `fn decide_by_event_kind(event_kind: HookEventKind, reason: String) -> HookDecision` and call it from both interpret_exit_2_stderr() and interpret_prompt_response().
- [x] `crates/agent-client-protocol-extras/src/hook_config/output.rs:264` — Functions `is_blockable` (line 264) and `feeds_stderr_to_agent` (line 276) contain nearly identical logic that differs only in which tuple element they extract from EVENT_PROPERTIES. Both iterate over the same table, find a matching HookEventKind, extract a bool field, and return it with an `unwrap_or(false)` default. Duplicated logic can drift out of sync if either function's lookup algorithm changes. Extract a shared helper function parameterized by the field selector: `fn get_event_property<F>(kind: HookEventKind, selector: F) -> bool where F: Fn(&(HookEventKind, bool, bool)) -> bool { EVENT_PROPERTIES.iter().find(|(k, _, _)| *k == kind).map(selector).unwrap_or(false) }`, then rewrite `is_blockable` as `get_event_property(kind, |(_, b, _)| b)` and `feeds_stderr_to_agent` as `get_event_property(kind, |(_, _, f)| f)`.
- [x] `crates/agent-client-protocol-extras/src/hook_config/output.rs:264` — The EVENT_PROPERTIES lookup pattern (lines 265-270) is duplicated in feeds_stderr_to_agent() at line 276. Both functions iterate through EVENT_PROPERTIES, find the matching kind, extract a different field, and return with default false. This should be extracted into a shared helper. Extract a shared helper: `fn get_event_property_field(kind: HookEventKind, index: usize) -> bool { EVENT_PROPERTIES.iter().find(|(k, _, _)| *k == kind).map(|(_, b, f)| if index == 1 { b } else { f }).unwrap_or(false) }` or use a closure-based generic to eliminate the duplicate lookup code.
- [x] `crates/agent-client-protocol-extras/src/hook_config/output.rs:276` — The EVENT_PROPERTIES lookup pattern (lines 277-282) is duplicated in is_blockable() at line 264. Both functions iterate through EVENT_PROPERTIES, find the matching kind, extract a field, and return with default false. Extract the common lookup into a helper function to eliminate the duplicated iteration and error handling code.
- [x] `crates/agent-client-protocol-extras/src/hook_config/output.rs:436` — The event-kind-based decision logic (lines 436-444) is duplicated in handlers.rs:188-200. This pattern should be extracted into a shared helper to avoid maintenance burden and ensure both code paths make identical decisions as the logic evolves. Extract into a shared helper: `fn decide_by_event_kind(event_kind: HookEventKind, reason: String) -> HookDecision` and use it from both interpret_exit_2_stderr() and interpret_prompt_response() to eliminate the duplication.

## Review Findings (2026-08-09 21:55)

Scope: `6e5c8a288^..6e5c8a288`

The six duplication findings of the last pass are fixed. Verification of the claims of the commit:

- `decide_by_event_kind` (`output.rs:352`) holds the four rules in the same order the two call sites held them, and both call sites now call it. The log condition is exactly equivalent: the old `else` arm returned the unit variant `HookDecision::Allow`, and the three other arms return `Block`, `ShouldContinue`, and `AllowWithContext`, which are distinct variants. `matches!(decision, HookDecision::Allow)` is therefore true for exactly the inputs that took the old `else` arm. No input reaches the warning that did not before, and none misses it.
- `EventProperties::NONE` sets `blockable: false` and `feeds_stderr_to_agent: false`, which is the old `unwrap_or(false)` of both readers. The four table rows keep their old values. `is_blockable` and `feeds_stderr_to_agent` are private, and a workspace grep shows their only callers are inside `output.rs`.
- The five new tests assert the branch each one names: `test_prompt_response_ok_false_on_a_silent_event_allows` (Notification, asserts `Allow`), `exit_2_on_a_stop_event_asks_the_agent_to_continue` (asserts `ShouldContinue` and the reason), `exit_2_on_a_post_tool_event_gives_the_stderr_to_the_agent` (asserts `AllowWithContext` and the context for both post-tool kinds), `exit_2_on_a_silent_event_logs_the_fall_back_to_allow` (asserts `Allow` and `logs_contain`), and `exit_2_on_a_blockable_event_logs_no_fall_back` (asserts `Block` and `!logs_contain`).
- Behavior does not change.

Every file of the commit is under the 262144-byte rendered cap: `handlers.rs` 17035, `handlers/tests.rs` 11280, `output.rs` 31766, `output/tests.rs` 24492. The engine reported `skipped: 0` with an empty `skipped_files` list. `crates/swissarmyhammer-validators/src/review/synthesize.rs` is not in the commit and has no uncommitted change.

One new finding:

- [x] `crates/agent-client-protocol-extras/src/hook_config/handlers/tests.rs:16` — Command injection vulnerability: the message parameter is interpolated into a shell command without escaping. A message containing single quotes could break out of the string and execute arbitrary commands. Use proper shell escaping. Install the shell_escape crate and write: format!("echo {} >&2; exit {EXIT_CODE_BLOCK}", shell_escape::unix::escape(message.into())).