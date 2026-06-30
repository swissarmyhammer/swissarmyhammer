---
assignees:
- claude-code
position_column: todo
position_ordinal: a480
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