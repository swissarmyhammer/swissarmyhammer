---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kwcphzdgbmw9f3pz7np31dts
  text: 'Picked up. Starting investigation: how the review engine selects files per validator (fleet.rs + scope/planning), how @file_groups/* globs are resolved (builtin/file_groups/*.yaml), and the cleanest single insertion point to subtract @file_groups/test_files for data-driven + duplication before the finder runs (both review file + review sha paths).'
  timestamp: 2026-06-30T16:42:18.928044+00:00
- actor: claude-code
  id: 01kwcq4ms72dpf7xayffj8a6zf
  text: |-
    Implementation landed. Mechanism + insertion point:

    - Added `exclude: Vec<String>` (#[serde(default)]) to `ValidatorMatch` in crates/swissarmyhammer-validators/src/validators/types.rs. A path matches a validator only if it matches `files` AND matches none of `exclude`.
    - The filter lives in the SINGLE shared `matches_files()` (types.rs), used by both `RuleSet::matches` and `Validator::matches`. The review scope stage (`scope.rs::match_validators_and_files` -> `loader.matching_rulesets` -> `RuleSet::matches`) resolves files through this one matcher for EVERY scope (Working/Sha/File/Glob), so the exclusion applies to both `review file` (whole-file) and `review sha` (diff-scoped) with no duplicated logic. Excluded test files never enter the per-validator work-list, so they never reach the finder prompt — structural, not prose.
    - Data-driven: `exclude:` references `@file_groups/test_files` in frontmatter and is expanded by the same YamlExpander path as `files` (proven by parser test). No hardcoded per-validator branch.
    - builtin/validators/data-driven/VALIDATOR.md + builtin/validators/duplication/VALIDATOR.md: added `match.exclude: [@file_groups/test_files]`. Removed the redundant "** IMPORTANT ** This rule does not apply to test code." prose line, replaced with a one-liner noting the carve-out is now engine-enforced.

    RED->GREEN evidence (real pipeline test review::drive::tests::review_excludes_test_files_for_an_excluding_validator, drives the real run_review_over_agent — same path the MCP review op uses):
    - RED (field present, matches_files NOT yet honoring exclude): FAILED with "a test file matched by an excluding validator must never reach the finder" — report contained "- [ ] `src/config_test.rs:1`". The test-file source reached the finder.
    - GREEN (after matches_files honors exclude): PASS. Exactly 1 finding, for the non-test src/config.rs; zero findings reference src/config_test.rs. Proves scoped, not blanket-off.

    Also added: 2 matches_files unit tests (single-path + changed_files branches), 1 parser test proving `@file_groups/test_files` expands into `exclude`, 1 builtin-load test proving the REAL builtin data-driven + duplication validators carry the expanded exclude and do not match *_test.rs / *.test.ts.

    Verification:
    - cargo nextest run -p swissarmyhammer-validators: 310 passed, 0 failed.
    - cargo clippy -p swissarmyhammer-validators --all-targets: zero warnings.
    - swissarmyhammer-tools affected (consumes engine): cargo clippy -p swissarmyhammer-tools --all-targets clean; cargo nextest run -p swissarmyhammer-tools review = 29 passed (incl. review_e2e working/sha/file).
  timestamp: 2026-06-30T16:52:30.631723+00:00
- actor: claude-code
  id: 01kwcqef9yq0tgdjf0b0wvffh8
  text: |-
    Adversarial double-check: PASS, no findings. Verified all five scrutiny points: (1) single shared matches_files path covers all Scope variants for both review file + review sha; (2) @file_groups/test_files expands in `exclude` through the production builtin loader; (3) real-pipeline test is genuine RED->GREEN and cannot pass for the wrong reason (non-test entry fires and is asserted present); (4) exclude reuses the same case-insensitive glob options as files — no wrong-drop/wrong-keep; (5) is_empty semantics unchanged, no apply_defaults regression. Also confirmed defense-in-depth: duplication's `duplicates` probe evidence is selected per matched file, so an excluded test file gets no FileWork and no finder prompt even though the probe computes over the full changed set.

    Leaving task in `doing` for /review.
  timestamp: 2026-06-30T16:57:52.702478+00:00
position_column: doing
position_ordinal: '8380'
project: local-review
title: Skip test files structurally in data-driven and duplication validators
---
## Problem

The "does not apply to test code" exclusion on `data-driven` and `duplication` does NOT work, and we now have empirical proof it is not a plumbing problem:

- The exclusion text IS in the loaded copy: `~/.validators/data-driven/VALIDATOR.md` ends with `** IMPORTANT ** This rule does not apply to test code.`
- Task ^0wvhgd7 makes the VALIDATOR.md body reach the model (proven working).
- The finder model (qwen) flagged test code ANYWAY, in its own words: "Configuration values … must use named constants **even in tests**" — types.rs:507, :682, :767, :787 (hardcoded `timeout: 30`/`60` in test helpers/tests).

Conclusion: a **prose** carve-out, even when it reaches the model, is not honored by the small finder model. The exclusion must be **structural** — enforced before the finder sees the file.

## Change

Drop files matching `@file_groups/test_files` from the candidate set for `data-driven` and `duplication` at the engine level, so the finder never receives test-code content for these two validators. The model cannot override what it is not shown.

Design notes:
- Prefer an engine-level filter keyed on the validator + the existing `@file_groups/test_files` group (builtin/file_groups/*.yaml) over per-validator config, so it is one code path, not N.
- Confirm the filter applies to BOTH whole-file (`review file`) and sha-scoped (`review sha`) paths.
- Once structural skipping lands, the now-redundant prose line in the two VALIDATOR.md bodies can be removed (or kept as documentation — decide during implementation).
- This is independent of ^0wvhgd7 (which correctly makes the body load-bearing). This card addresses what the body alone cannot enforce.

## Real-path test (required)

A production-path test: run the real review pipeline over a file that is BOTH (a) matched by `data-driven`/`duplication` and (b) a test file, with a literal/duplicate that WOULD be flagged in non-test code — assert ZERO findings from these two validators. Mock-boundary tests do not count (see real-path-tests-not-mocks).

## Done when

- A test file with flaggable literals/dupes produces zero `data-driven`/`duplication` findings via the real pipeline.
- A non-test source file with the same patterns still produces findings (proves the filter is scoped, not a blanket-off).
- `cargo nextest run` green; clippy clean.