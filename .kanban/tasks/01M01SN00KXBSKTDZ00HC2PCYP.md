---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m0720xbwgp6qdazj00e3rkjt
  text: |-
    Research and measurement, on Dart SDK 3.11.0.

    The card states that a stale floor makes both rules under-report in silence. I drove the real tools over real probe packages to find the shape of that loss.

    1. The analyzer RECOVERS from an expression-level feature. A dot shorthand (Dart 3.10) in a field initializer, in a `case` pattern and in a `switch` expression, a digit separator (Dart 3.6), a wildcard parameter (Dart 3.7) and a null-aware element (Dart 3.8) each write `EXPERIMENT_NOT_ENABLED` as a SYNTACTIC_ERROR under a low floor, and the row count of `public_member_api_docs` and of `no_magic_number` does NOT move. So the dot-shorthand probe of the card measures the syntax error, not a lost finding.

    2. The analyzer LOSES a whole DECLARATION the floor does not know. An `extension type` (Dart 3.3) is the live case for `missing-docs-dart`, whose floor was `>=3.0.0`:

    | the probe constraint | rows |
    |---|---|
    | `>=3.0.0 <5.0.0` | 7, 8 and 10, and one SYNTACTIC_ERROR naming `inline-class` |
    | `^3.11.0` | 1, 2, 4, 7, 8 and 10 |

    Three of six rows go away at exit 0.

    3. `magic-numbers-dart` has the same mechanism but no live loss at its own floor, because `>=3.5.0` is above the 3.3 of `extension type`. Measured over one `extension type` holding two unnamed literals beside one plain function holding a third: 1 of 3 findings at `>=3.0.0 <5.0.0`, 3 of 3 at `>=3.5.0 <4.0.0`, and 3 of 3 at `^3.11.0`.

    4. The `<4.0.0` upper bound is a second cost. Measured with `sdk: '>=3.5.0 <3.6.0'` on Dart SDK 3.11.0: `dart pub get --offline` and `dart pub get` each exit 1 and write `Because sah_probe requires SDK version >=3.5.0 <3.6.0, version solving failed.`
  timestamp: 2026-08-17T05:10:03.388081+00:00
- actor: claude-code
  id: 01m07218bxxw73899mhj1cqar6
  text: |-
    ### implement — changed
    - evidence: 3 files.
      - `builtin/validators/code-hygiene/rules/missing-docs-dart.md` — the script reads `dart --version` with `sed` and writes `sdk: '^<version>'` in place of `>=3.0.0 <5.0.0`; a `dart` that names no version exits 1 with a named line; `doctor.check_command` adds `sed`; new section "The probe package states the language version of the installed SDK" holds the measurement, and the `dart pub get` section states the measured window failure.
      - `builtin/validators/code-hygiene/rules/magic-numbers-dart.md` — the same derivation in place of `>=3.5.0 <4.0.0`; `doctor.check_command` adds `sed`; the same new section holds its own measurements.
      - `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs` — new acceptance test `the_shipped_dart_missing_docs_tool_rule_reports_a_member_of_a_newer_declaration`, and the doc comment of the pub-get test now states the measured window failure in place of the fixed window.
    - RED: with `sdk: '>=3.0.0 <5.0.0'` put back, the new test failed. Reported `lib/language_version.dart:7, :8, :10` against the required `:1, :2, :4, :7, :8, :10`.
    - GREEN: with the derived constraint, the same test passes.
    - `cargo nextest run -p swissarmyhammer-validators`: 796 tests run, 796 passed, 0 skipped. `cargo fmt --all -- --check` clean. `cargo clippy -p swissarmyhammer-validators --all-targets`: no warning.
    - next: `/review`
  timestamp: 2026-08-17T05:10:14.653807+00:00
- actor: claude-code
  id: 01m0746gbqwjh3qz7s0bs3j8gr
  text: |-
    ### review — findings
    - evidence: 4 findings — missing_docs.rs:689, missing_docs.rs:702, missing_docs.rs:722, missing_docs.rs:753 (scope `review sha HEAD~1..HEAD`)
    - next: Rename the three Dart language-version constants with the `DART_MISSING_DOCS_` prefix. Add the error-case test for a `dart --version` that names no version. The card stays in review.
  timestamp: 2026-08-17T05:48:03.831788+00:00
- actor: claude-code
  id: 01m074732qbbhnsv4c9dj22240
  text: |
    ### finish iteration 1 — findings
    - implement: changed — 3 files. Both Dart probe rules read the version out of `dart --version` and write `sdk: '^<version>'`. Measured on Dart SDK 3.11.0: the missing-docs probe at the fixed floor loses the 3 members of an extension type and exits 0.
    - test: green — cargo nextest run --workspace, 14140 passed, 0 failed. fmt and clippy clean. The test step found that magic-numbers-dart changed with NO test of its own, and added two. It also found `sort` missing from the missing-docs-dart doctor check.
    - commit: f5b15a80f
    - review: findings — missing_docs.rs:689, :702, :722, :753. Three ask for the `DART_MISSING_DOCS_` prefix on the new constants. One asks for the missing "dart --version names no version" test.
  timestamp: 2026-08-17T05:48:22.999967+00:00
- actor: claude-code
  id: 01m074jah5pb12h5wnpr4nmfcf
  text: |-
    Round 2, the four review findings.

    Sweep for the naming finding. `rg '^const DART_'` over `missing_docs.rs` gives 41 constants. The diff of `HEAD~1..HEAD` shows that the Dart language-version work added three of them, and all three lacked the rule prefix. So the sweep of the whole file finds the same three the review printed, and no more. The other unprefixed `DART_` constants stood before this card.

    The new error-case test reuses `DART_BINARY_NAME`, which the file already holds, in place of a second binary constant. Its doc said "The binary both runs of the shipped Dart script call"; the script now makes three runs, so the doc says "every run" instead.

    RED, with the break taken out of `missing-docs-dart.md`: the run writes `sdk: '^'`, `dart pub get` exits 65, and the test failed with `the run must break with 'dart --version names no version'; got '... missing-docs-dart: dart pub get exited 65 ...'`.

    GREEN, with the rule body put back byte for byte (md5 `389c3e47a346f7c81cbbdc4e09292e43`, and `git diff -- builtin/` is empty): the test passes.

    The doc of the new test names the same measurement `magic-numbers-dart.md` states. Neither rule file names its no-version acceptance test in that section, so `missing-docs-dart.md` needs no new text and stays unchanged.
  timestamp: 2026-08-17T05:54:31.077049+00:00
- actor: claude-code
  id: 01m074jhm6sf9ncq5wq1jqywka
  text: |-
    ### implement — changed
    - evidence: 1 file — `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs`. Renamed `DART_LANGUAGE_VERSION_PATH`, `DART_LANGUAGE_VERSION_SOURCE` and `DART_LANGUAGE_VERSION_HEADS` to the `DART_MISSING_DOCS_` prefix. Added `the_shipped_dart_missing_docs_tool_rule_breaks_when_dart_version_names_no_version` with `DART_MISSING_DOCS_VERSION_SUBCOMMAND`, `DART_MISSING_DOCS_NO_VERSION_PATH`, `_SOURCE`, `_STAGED`, `_ERROR` and `_PROBE`.
    - `cargo nextest run -p swissarmyhammer-validators`: 799 tests run, 799 passed, 0 skipped. `cargo fmt --all -- --check` clean. `cargo clippy -p swissarmyhammer-validators --all-targets`: no warning.
    - All 4 findings are checked.
    - next: `/review`
  timestamp: 2026-08-17T05:54:38.342382+00:00
position_column: doing
position_ordinal: '8280'
title: dart probe packages pin the language version to a stale floor
---
`missing-docs-dart` and `magic-numbers-dart` each build a probe package and write a fixed `environment: sdk:` constraint into its `pubspec.yaml`:

- `missing-docs-dart` writes `sdk: '>=3.0.0 <5.0.0'`
- `magic-numbers-dart` writes `sdk: '>=3.5.0 <4.0.0'`

A Dart package's LANGUAGE VERSION is the LOWER bound of that constraint, and the analyzer refuses syntax newer than that version. So each probe reads the copied files as Dart 3.0 or Dart 3.5 source, whatever SDK is installed, and a project using a newer language feature is analyzed wrongly.

Measured on Dart SDK 3.11.0 over one file using a dot shorthand (`Shade undocumentedField = .light;`, a Dart 3.10 feature), with `public_member_api_docs` on:

| the probe constraint | what `dart analyze` reports |
|---|---|
| `>=3.0.0 <5.0.0` | 1 `EXPERIMENT_NOT_ENABLED`, 6 `PUBLIC_MEMBER_API_DOCS` |
| `^3.11.0` | 6 `PUBLIC_MEMBER_API_DOCS` |

The same floor is worse for a whole-file parse. Measured over the 3508 `.dart` files of `flutter/packages` at `a3e763e`, through a probe stating `sdk: '>=3.5.0 <4.0.0'`: `dart analyze` writes `This requires the 'dot-shorthands' language feature to be enabled` as a SYNTACTIC_ERROR. A file the analyzer cannot parse yields no member and no diagnostic of the kind either rule selects, so both rules under-report in silence — which is the shape `builtin/validators/README.md` names as a tool that reads a dirty file as clean.

`function-length-dart` already answers this. Its script reads the version out of `dart --version` and writes `sdk: '^<version>'`, so the probe always parses with the language version of the installed SDK, and the caret keeps the constraint correct across a major version too. The section "The probe package states the language version of the installed SDK" in `builtin/validators/code-hygiene/rules/function-length-dart.md` records the measurement.

## Done when

- `missing-docs-dart` and `magic-numbers-dart` each derive the probe `sdk:` constraint from `dart --version` rather than stating a fixed floor.
- Each rule file records the measurement, the way `function-length-dart` does.
- An acceptance test drives at least one of the two over a file using a language feature newer than the old floor, and holds the run to the findings it must report.

#tool-validators #objectivity

## Review Findings (2026-08-17 00:40)

> Scope: `review sha HEAD~1..HEAD` — reviewed the diffs only — lines this change added or modified. 2 file(s) reviewed, 4 not reviewed.

> 4 file(s) not reviewed — excluded by an ignore rule:
> - `.kanban/ (from .reviewignore)` — 4 file(s)

- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs:689` `naming/naming-consistency` — Constant name omits the rule prefix — should be `DART_MISSING_DOCS_LANGUAGE_VERSION_PATH` to match the naming pattern established by the test function `the_shipped_dart_missing_docs_tool_rule_reports_a_member_of_a_newer_declaration` and the parallel constants in magic_numbers.rs like `DART_MAGIC_NUMBERS_LANGUAGE_VERSION_PATH`. Rename `DART_LANGUAGE_VERSION_PATH` to `DART_MISSING_DOCS_LANGUAGE_VERSION_PATH` to include the rule name and maintain consistency with the parallel naming in magic_numbers.rs.
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs:702` `naming/naming-consistency` — Constant name omits the rule prefix — should be `DART_MISSING_DOCS_LANGUAGE_VERSION_SOURCE` to match the naming pattern established by the test function and the parallel constants in magic_numbers.rs. Rename `DART_LANGUAGE_VERSION_SOURCE` to `DART_MISSING_DOCS_LANGUAGE_VERSION_SOURCE`.
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs:722` `naming/naming-consistency` — Constant name omits the rule prefix — should be `DART_MISSING_DOCS_LANGUAGE_VERSION_HEADS` to match the naming pattern established by the test function and the parallel constants in magic_numbers.rs. Rename `DART_LANGUAGE_VERSION_HEADS` to `DART_MISSING_DOCS_LANGUAGE_VERSION_HEADS`.
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs:753` `completeness/invariant-propagation` — The missing_docs validator tests lack an error-case test for when `dart --version` returns no version. The magic_numbers validator tests include this test (magic_numbers.rs lines 534–544), but missing_docs tests do not. Both validators' scripts now read `dart --version` dynamically and break when it names no version, so both test suites should verify this failure path. Add a test function after line 775 (parallel to magic_numbers.rs lines 534–544) that verifies the missing_docs validator breaks when `dart --version` returns no version. Define constants DART_NO_VERSION_PATH, DART_NO_VERSION_SOURCE, DART_NO_VERSION_ERROR, DART_NO_VERSION_STAGED, and DART_NO_VERSION_PROBE (lines 490–523 in magic_numbers.rs are the reference pattern).
