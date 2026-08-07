---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzd035ym1fc2ky852yerr3zd
  text: |
    ### Research

    Root cause confirmed. `run_review` (synthesize.rs) computes one number and uses it for two different jobs:

    ```
    let framing = prompt_framing_bytes(&work, loader);
    let budget  = fleet_config.file_payload_budget(framing);   // = batch_size.min(AGENT_PROMPT_CAP - framing)
    let (batches, skipped) = batch_work_list(&work, budget, rendered_file_block_bytes);
    ```

    `batch_work_list` uses that ONE number both to decide where batch boundaries fall AND to decide the over-cap verdict (`size > budget` -> `SkippedFile`).

    `prompt_framing_bytes` (fleet/render.rs) grows with the diff:
    - `render_shared_probe_evidence` over the WHOLE changed set (the `<changed-set>` duplicates rows) — the dominant term, ~18 KB per changed file in the reported rounds;
    - `render_focus_files` inside `render_validator_suffix` — one line per matched file.

    So more files -> more framing -> smaller budget -> more files over cap. The card's loop.

    Numbers: `AGENT_PROMPT_CAP = 512 * 1024 = 524288` (claude_agent `MAX_PROMPT_LENGTH`). Round 1 budget 164176 -> framing 360112. Round 2 budget 54338 -> framing 469950.

    Plan: split the one number into two.
    - constant per-file cap decides the over-cap verdict — `min(batch_size, AGENT_PROMPT_CAP / 2)`, independent of the diff;
    - measured per-batch budget still decides batch boundaries, so prompts keep fitting.
  timestamp: 2026-08-07T02:16:05.332233+00:00
- actor: claude-code
  id: 01kzd0xfc9ngtzvm6hhefp8h5g
  text: |
    ### Implementation

    Split the one number into two.

    `crates/swissarmyhammer-validators/src/review/scope/batch.rs` now takes a `BatchBudget { file_cap, batch_bytes }` built from the `FileCapBytes` and `BatchBytes` newtypes (the `TasksAttempted`/`TasksFailed` pattern, so the two byte counts cannot be transposed):
    - `file_cap` decides the over-cap verdict — `size > budget.file_cap()` is the `SkippedFile`;
    - `batch_bytes` decides where batch boundaries fall.

    `FleetConfig` gained `MAX_FILE_BLOCK_BYTES` (`AGENT_PROMPT_CAP / PROMPT_SHARES_PER_FILE_BLOCK`, so 256 KiB), `file_block_cap()` (`batch_size.min(MAX_FILE_BLOCK_BYTES)`), and `batch_budget(framing)` which pairs the constant cap with the framing-sensitive `file_payload_budget(framing)`. Building both in one place is what stops a call site collapsing them again.

    `SkippedFile::budget()` became `cap()`, and the report now says "over the N-byte per-file cap ... (split the file)" instead of "over the N-byte batch budget ... (narrow the scope)". Narrowing the scope can no longer change the verdict, so the old advice was wrong.

    RED -> GREEN was verified by toggling `batch_budget` back to the pre-fix cap. With the defect restored, `a_file_inside_the_cap_stays_inside_it_when_the_change_around_it_grows` failed with exactly the reported symptom:

        SkippedFile { path: "src/subject.rs", validator: "bulk", size: 197707, cap: 50946 }

    The subject renders 197707 bytes and satisfies the 262144-byte cap, but the bigger change's shared probe evidence dropped the old threshold to 50946.

    ### Discovery, recorded as ^x8z9hgf

    `prompt_framing_bytes` is 360112 bytes on the 9-file round and 469950 on the 15-file round, against a 524288-byte cap — 69% then 90% of every prompt. It is dominated by the `<changed-set>` shared probe evidence, which `project_onto_files` carries verbatim into every batch. That squeeze is a separate defect; new card `^x8z9hgf` covers it.
  timestamp: 2026-08-07T02:30:26.953880+00:00
- actor: claude-code
  id: 01kzd0xt1ftrt6cs8a1mst01mt
  text: |
    ### implement — changed
    - evidence: 12 files — crates/swissarmyhammer-validators/src/review/scope/batch.rs, review/scope.rs, review/scope/tests.rs, review/fleet.rs, review/fleet/render.rs, review/fleet/tests.rs, review/synthesize.rs, review/test_support.rs, review/mod.rs, crates/swissarmyhammer-tools/src/mcp/tools/review/mod.rs, review/review_op/response.rs, review/tests.rs. `cargo test -p swissarmyhammer-validators` 455 passed, 0 failed; `cargo test -p swissarmyhammer-tools --lib` 1313 passed, 0 failed; clippy `-D warnings` clean on both crates; `cargo check --workspace --all-targets` clean.
    - next: /review
  timestamp: 2026-08-07T02:30:37.871171+00:00
- actor: claude-code
  id: 01kzd1fj9s9xw6a89egzcz7z65
  text: |-
    ### commit — changed
    - evidence: 7df31c4b4 fix(review): split batch budget into a per-file cap and a batch cap (^tsram0q)
    - next: review
  timestamp: 2026-08-07T02:40:19.769637+00:00
- actor: claude-code
  id: 01kzd1hhfrjn2hz1w1ggzh92n4
  text: 'Check-in from the watching session (card author, 2026-08-07): both acceptance criteria are met by construction and by test — the over-cap verdict now comes from a constant per-file cap (the `a_file_inside_the_cap_stays_inside_it_when_the_change_around_it_grows` test reproduces the exact ^cbnfe97 symptom with the defect restored, cap 50946 vs 262144), and the two newtypes stop the collapse from recurring. The report-text correction ("split the file" instead of "narrow the scope") matters too — the old advice could not change the verdict. Your ^x8z9hgf discovery (framing at 69-90% of the prompt) is the right follow-up split. Nothing to add.'
  timestamp: 2026-08-07T02:41:24.472232+00:00
- actor: claude-code
  id: 01kzd2vmw3yfh6x11cbtfrqtk5
  text: |
    ### review — findings
    - evidence: `review sha HEAD~1..HEAD` (7df31c4b4). counts: 3 findings, 3 confirmed, 30 refuted, 36 attempted, 0 failed, 0 skipped, `skipped_files: []`. 2 recorded — crates/swissarmyhammer-validators/src/review/test_support.rs:55, crates/swissarmyhammer-validators/src/review/test_support.rs:57.
    - dropped: crates/swissarmyhammer-validators/src/review/scope/tests.rs:825 (magic number 5 to a named constant). The line is authored by 503b743463, not by the commit under review, and the subject is restyling test code that already existed — the skill's blanket test exception drops it.
    - subject evidence: `skipped_files` is empty on a 12-file diff. The round-2 15-file diff reported 13 over-cap files. The per-file cap is now constant, so no over-cap finding was raised against this card's own change.
    - note: all 3 findings sit on lines authored by 503b743463; the commit under review touched neither line.
    - next: fix the two path-traversal findings in test_support.rs, then re-review.
  timestamp: 2026-08-07T03:04:24.195220+00:00
- actor: claude-code
  id: 01kzd2wepynpr0zwh7pmsgjsb9
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 12 files; batch_work_list takes BatchBudget with a constant file_cap and a separate batch_bytes; new card ^x8z9hgf records the shared-probe framing bloat
    - test: green — cargo nextest run --workspace 13663 passed, doc tests 0 failed, fmt clean, clippy clean
    - commit: 7df31c4b4
    - review: findings — crates/swissarmyhammer-validators/src/review/test_support.rs:55, :57 (write_tool_rule_fixtures interpolates the rule name into a path with no confine_relative guard)
    - acceptance evidence: the review reported skipped_files [] over a 12-file diff. The round-2 baseline on this card was 13 over-cap files on a 15-file diff. The per-file cap no longer follows the diff size.
  timestamp: 2026-08-07T03:04:50.654291+00:00
- actor: claude-code
  id: 01kzd3aayxahz0vtv8dzfnf9xv
  text: |
    ### Implementation — path-traversal findings

    Both findings name `write_tool_rule_fixtures`, but the cause is the file's habit of
    joining a caller-supplied name straight onto a base directory. I removed that cause
    from the whole file, not only from the two named lines.

    New private helper next to `confine_relative`:

        fn join_confined(base: &Path, rel: &str) -> PathBuf {
            base.join(confine_relative(rel))
        }

    Every path in the file that a caller-supplied name builds now goes through it:

    - `write_tool_rule_fixtures` — both writes (the two named lines).
    - `write_tool_rule_ruleset` — `base.join(name)`, the third site the findings did
      not name. It was the worse one: a `..` in `name` put the whole ruleset directory,
      `rules/`, and `VALIDATOR.md` outside `base`.
    - `TestRepo::write` — was already calling `confine_relative` by hand; it now calls
      the shared helper, so one guard covers all four writes.

    `confine_relative`'s doc said "working-tree-relative"; it is no longer repo-only, so
    the doc now says "a base directory".

    ### RED -> GREEN, verified

    Two new tests, both watched failing first against the unguarded code:

        write_tool_rule_fixtures_confines_a_traversing_rule_name  FAILED
          a `..` rule name must not climb out of the fixtures dir to
          /var/.../T/escape_rule_.tmpdzFDb8.fail.rs
        write_tool_rule_ruleset_confines_a_traversing_name  FAILED
          a `..` ruleset name must not climb out of the base dir to
          /var/.../T/escape_ruleset_.tmpzQd96p

    Both write real files OUTSIDE the temp dir before the fix — the vulnerability is
    reproduced, not asserted from reading. Both pass after it. The escaped artifacts the
    RED run left in the system temp dir were removed.

    ### Not a path, so not routed through the guard

    `rename_current_branch_to` and `checkout_new_branch` interpolate `name` into
    `refs/heads/{name}`. That is a git refname handed to libgit2, not a filesystem
    join — `repo.branch(name, ..)` normalizes and rejects it first, and a path guard
    would corrupt a legitimate refname. No other site in the file builds a path from an
    untrusted name; every remaining `join` takes a literal (`fixtures`, `rules`,
    `VALIDATOR.md`, `.code-context`, `index.db`).

    ### Refactor

    Four confinement tests each rebuilt the same unique-marker `format!` with a
    different prefix literal. Extracted to `escape_marker(prefix, dir)` in the test
    module — one block with an argument instead of four copies.

    ### Blast radius

    Callers pass literal names (`docs-tool`, `tooled`, `tooled-set`), so no behavior
    changed for any of them: `review/drive.rs`, `review/tool_rules.rs`,
    `validators/loader.rs`, and `swissarmyhammer-tools`' `mcp/tools/review/tests.rs`.
  timestamp: 2026-08-07T03:12:25.565707+00:00
- actor: claude-code
  id: 01kzd3ahaffj0bbemk49z3mjv6
  text: |
    ### implement — changed
    - evidence: 1 file — crates/swissarmyhammer-validators/src/review/test_support.rs. Both `## Review Findings` items flipped to `- [x]`. `cargo test -p swissarmyhammer-validators --all-features` 455 passed, 0 failed, doc tests 2 passed; `cargo test -p swissarmyhammer-tools --lib mcp::tools::review` 69 passed, 0 failed; `cargo clippy -p swissarmyhammer-validators --all-targets --all-features -- -D warnings` clean; `cargo fmt --all --check` clean; `cargo check --workspace --all-targets` clean.
    - next: /review
  timestamp: 2026-08-07T03:12:32.079733+00:00
- actor: claude-code
  id: 01kzd4pkekcnmgmzyjegfw3ben
  text: |
    ### review — findings
    - evidence: review sha HEAD~1..HEAD (99130d71e) — 1 finding, 1 confirmed, 3 refuted, 9 attempted, 0 skipped. crates/swissarmyhammer-validators/src/review/test_support.rs:213
    - prior: the 2 findings from 2026-08-06 21:40 are truly closed — write_tool_rule_fixtures now routes both the fail and the pass fixture path through join_confined.
    - next: make commit_only use join_confined, or correct its docstring. Task stays in review.
  timestamp: 2026-08-07T03:36:36.051391+00:00
- actor: claude-code
  id: 01kzd4q1gxn05arnvd5r2j6m3f
  text: |-
    ### finish iteration 2 — findings
    - implement: changed — crates/swissarmyhammer-validators/src/review/test_support.rs; the 2 prior findings are closed and verified
    - test: green — cargo nextest run --workspace 13665 passed, doc tests 0 failed, fmt clean, clippy clean
    - commit: 99130d71e
    - review: findings — crates/swissarmyhammer-validators/src/review/test_support.rs:213 (commit_only still calls confine_relative directly while its docstring promises the join_confined path)
    - guardrail: no repeat. Each round names a different line and a different cause.
  timestamp: 2026-08-07T03:36:50.461679+00:00
- actor: claude-code
  id: 01kzd510v3et1rewhjs26k6y24
  text: |
    ### Implementation — the commit_only confinement finding

    The finding gives two options. Only one of them is correct here.

    `join_confined(base, rel)` returns `base.join(...)` — an ABSOLUTE path.
    `commit_only` hands its result to `git2::Index::add_path`, which stages a path
    relative to the working tree and rejects an absolute one. Routing `commit_only`
    through `join_confined` would break every caller. So the docstring is the part
    that is wrong, and the docstring is what I corrected.

    `confine_relative` is already the shared guard both paths apply. The two paths
    differ only in what they do with its result: `write` (through `join_confined`)
    resolves it against a base directory; `commit_only` stops at the relative form.
    The old docstring hid that difference behind "the same way `write` confines".

    ### Swept the whole file, not only line 213

    The cause is a docstring that describes a confinement shape the code no longer
    has. Three docstrings carried that cause:

    1. `commit_only` — the named one. Now states the shared `confine_relative`
       guard AND the difference: not joined onto the repo root, because libgit2's
       index stages a working-tree-relative path.
    2. `join_confined` — said "Every path these fixtures build from a
       caller-supplied name goes through here". False: `commit_only` does not. Now
       scoped to every FILESYSTEM path, and names `commit_only` as the one confined
       caller that builds no filesystem path.
    3. `confine_relative` — framed its whole purpose as guarding a `Path::join`
       onto a base. That describes only one of its two callers. Now names both
       callers and what each does with the result.

    Every remaining direct `confine_relative` call was audited. There are exactly
    two: `join_confined` (line 163) and `commit_only` (line 227). Both are now
    documented as the two callers, and the docs agree with the code.

    ### No test

    Documentation only — no behavior changed, so there is no new behavior to drive
    RED. The confinement behavior the docs describe is already under test:
    `write_confines_an_absolute_rel_under_the_repo_root`,
    `write_confines_a_dotdot_rel_under_the_repo_root`,
    `write_tool_rule_fixtures_confines_a_traversing_rule_name`, and
    `write_tool_rule_ruleset_confines_a_traversing_name`.

    ### Verification

    `cargo doc -p swissarmyhammer-validators --no-deps --all-features
    --document-private-items` emits zero warnings against `test_support.rs`, so the
    new intra-doc links (`[TestRepo::commit_only]`, `[join_confined]`,
    `[confine_relative]`) all resolve.
  timestamp: 2026-08-07T03:42:17.443302+00:00
- actor: claude-code
  id: 01kzd5168xtm30jceaefykpr3x
  text: |
    ### implement — changed
    - evidence: 1 file — crates/swissarmyhammer-validators/src/review/test_support.rs (docstrings on `confine_relative`, `join_confined`, `TestRepo::commit_only`). The `## Review Findings (2026-08-06 22:31)` item is flipped to `- [x]`. `cargo test -p swissarmyhammer-validators --all-features` 455 passed, 0 failed, doc tests 2 passed; `cargo clippy -p swissarmyhammer-validators --all-targets --all-features -- -D warnings` clean; `cargo fmt --all --check` clean; `cargo check --workspace --all-targets` clean; `cargo doc --document-private-items` zero warnings in test_support.rs.
    - next: /review
  timestamp: 2026-08-07T03:42:23.005184+00:00
position_column: doing
position_ordinal: '8480'
title: Review batch budget shrinks as the diff grows — over-cap splits cannot converge
---
Found while watching ^cbnfe97. The per-batch prompt budget is not stable between review runs on the same code:

- Round 1 (checkpoint 716951039, 9-file diff): batch budget 164176 bytes. 4 files over cap.
- Round 2 (checkpoint 503b74346, 15-file diff): batch budget 54338 bytes — one third of round 1. 13 files over cap, INCLUDING files the round-1 fix just split (fleet.rs 82→52KB source, scope.rs 177→49KB source) and files untouched by the diff logic (doctor.rs).

The feedback loop: an over-cap finding says "split the file" (by design — c45ba2d40 made over-cap a confirmed finding). Splitting adds files to the next diff. A bigger diff shrinks the per-batch budget. A smaller budget puts MORE files over cap. Each fix round makes the next round worse. This cannot converge.

Work:
- Find where the batch budget is computed (batch_work_list / cost math, now in review/scope/batch.rs) and confirm the budget depends on diff size or batch count.
- Make the over-cap threshold stable per file: a constant cap, independent of how many files the diff carries.
- An over-cap verdict must be reproducible: the same file content gets the same verdict on every run.

Acceptance:
- Two consecutive review runs over the same content report the same set of over-cap files.
- A file that satisfied the cap in run N cannot be over cap in run N+1 without growing.

#tool-validators

## Review Findings (2026-08-06 21:40)

Scope: `review sha HEAD~1..HEAD` (7df31c4b4). Acceptance evidence: this run reported
`skipped_files: []` — zero over-cap files across a 12-file diff, where the round-2
15-file diff reported 13. The cap no longer moves with diff size.

- [x] `crates/swissarmyhammer-validators/src/review/test_support.rs:55` — Path traversal vulnerability: the `rule` parameter is used in a format string to construct a filesystem path without validation. If `rule` contains path traversal sequences like `../`, files can be written outside the intended `fixtures` directory. Validate the `rule` parameter to reject path traversal attempts. Use a function like `confine_relative()` (already defined in this file at line 123) to strip non-normal path components, or explicitly validate that `rule` matches a safe pattern (e.g., `^[a-zA-Z0-9_-]+$`).
- [x] `crates/swissarmyhammer-validators/src/review/test_support.rs:57` — Path traversal vulnerability: the `rule` parameter is used in a format string to construct a filesystem path without validation. If `rule` contains path traversal sequences like `../`, files can be written outside the intended `fixtures` directory. Apply the same fix as line 55: validate the `rule` parameter to reject path traversal attempts using `confine_relative()` or an explicit whitelist pattern.

## Review Findings (2026-08-06 22:31)

- [x] `crates/swissarmyhammer-validators/src/review/test_support.rs:213` — The fix routes caller-supplied names through join_confined in write (line 187), write_tool_rule_fixtures (line 60–65), and write_tool_rule_ruleset (line 389), but commit_only still uses confine_relative directly. The docstring at lines 204–205 promises that paths are confined 'the same way write confines', which is now false after this change. Apply join_confined to commit_only's path parameter to achieve uniform confinement across all caller-supplied name functions, or update the docstring to acknowledge the different confinement mechanisms.
