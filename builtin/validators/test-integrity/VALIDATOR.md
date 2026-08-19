---
name: test-integrity
description: >-
  Flag test cheating — skipped/disabled tests, commented-out test bodies,
  over-mocking, trivial assertions, swallowed failures — implementations that
  hard-code values to make a test pass ("return 42"), and test suites that an
  environment variable switches instead of separate targets that CI runs. A
  confirmed integrity violation is a blocker.
metadata:
  version: "{{version}}"
match:
  files:
    - "@file_groups/source_code"
    - "@file_groups/test_files"
    - "*.github/workflows/*.yml"
    - "*.github/workflows/*.yaml"
    - "*.gitlab-ci.yml"
    - "*.circleci/config.yml"
probes:
  - assertion-census
---

Three one-concern rules. `no-hard-code` and `test-partitioning` read the diff
alone; `no-test-cheating` also reads the `assertion-census` probe, which
measures the test bodies for it:

- `no-test-cheating` — tests that are skipped, emptied, commented out, or
  written so they assert nothing, and `try`/`catch` that swallows a failure
  instead of proving it. The engine runs the `assertion-census` probe over each
  changed file and attaches one row per test function whose body measured
  something suspect. `assertion-census` is a *candidate* probe: it measures the
  bodies, and the rule judges whether the measurement is cheating.
- `no-hard-code` — an implementation that returns a literal matching the test's
  expectation, instead of computing the answer.
- `test-partitioning` — a test that an environment variable selects, skips, or
  switches; an integration test inside the unit target; and a unit or
  integration test target that no CI task runs. The CI configuration patterns
  in `match` above exist for this rule, so a CI workflow change reaches it.
