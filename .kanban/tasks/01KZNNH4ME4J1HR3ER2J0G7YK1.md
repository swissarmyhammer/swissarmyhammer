---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzq8ghcvjt9kzt3c5838bwxp
  text: |
    ### Measurement before implementation — Dart SDK 3.11.0

    I ran the shipped script as a replica against real files. Results:

    **Claim 1 — test files. CONFIRMED.** A real package with `lib/`, `test/`, `bin/`, `tool/`, `example/`, `integration_test/`, `test_driver/`, `benchmark/`, `web/` and a package-root file reports `public_member_api_docs` ONLY under `lib/` (`lib/a.dart`, `lib/src/b.dart`). The shipped probe over `test/widget_test.dart` holding `class TestHelper`, `void reset()` and `void buildHarness()` reports all 3.

    **Claim 2 — generated code. CONFIRMED.** A real package reports `lib/gen.g.dart`; the same package with `analyzer: exclude: ["**/*.g.dart"]` reports nothing there. The shipped probe reports `model.g.dart` in full, and reports an orphan `part of` generated file too (2 findings). A generated file that carries `// ignore_for_file: type=lint` (freezed writes one) is silent already.

    **Claim 3 — `toString()` and `operator ==` need a doc comment. REFUTED.** The lint carves out every member that overrides a resolved member. Measured: `toString()` (no `@override`), `@override bool operator ==`, `@override int get hashCode` and `@override void undocumentedHook()` over a documented base all report NOTHING. Only the base declaration and a plain method reported. The `@override` annotation is not what does it — resolution is.

    **Claim 4 — simple getters and setters. CONFIRMED.** `int get value` and `set value(int)` both report.

    **Claim 5 — the private carve-out is reproduced. CONFIRMED.** `_privateField`, `_privateMethod`, `_privateTopLevel`, `_PrivateClass` and every member inside a private class report nothing.

    **New, measured, NOT on the card:** `void main()` is carved out by the lint. And an override of a member the probe CANNOT RESOLVE is reported. The probe declares no dependencies, so `package:flutter/material.dart` does not resolve, and `@override Widget build(BuildContext)` reports. Measured on a two-package probe: the app package analyzed in place reports 1 finding; the shipped probe over the same file reports 2 (the override is the false one); the probe run with `--packages=<app>/.dart_tool/package_config.json` reports exactly 1, the same as the project. `--packages` restores the carve-out and the lint still fires. That is a separate defect on a separate axis; I raise a new card for it.

    **Design decision.** The probe copies every file it is given, and the probe's `analysis_options.yaml` gains an `analyzer: exclude:` list. A `lib/`-only copy filter is not possible: the doctor materializes a fixture flat as `missing-docs-dart.fail.dart` with no directory, so a `lib/` filter would skip the fail fixture and make the pair fail. The exclude list is measured to keep the loose fixture and to drop every other position. Verified over 19 staged paths: reported = `example/lib/a.dart`, `lib/src/real.dart`, `missing-docs-dart.fail.dart`, `packages/pkg/lib/a.dart`; excluded = every `test/`, `integration_test/`, `test_driver/`, `bin/`, `tool/`, `benchmark/`, `web/` position, the loose `example/a.dart`, and each generator suffix. `example/` uses `**/example/*.dart` and not `**/example/**`, because `example/lib/` is the public API of a package of its own.
  timestamp: 2026-08-11T01:55:35.963194+00:00
- actor: claude-code
  id: 01kzq97h1qvphgbqpqny7v25vc
  text: |
    ### implement — changed

    **What landed.**

    1. `builtin/validators/code-hygiene/rules/missing-docs-dart.md`. The probe's `analysis_options.yaml` is now written with a heredoc and carries an `analyzer: exclude:` list. The list holds the package directories that are not `lib/` (`test/`, `integration_test/`, `test_driver/`, `bin/`, `tool/`, `benchmark/`, `web/`), the loose Dart file of an `example/` directory (`**/example/*.dart`), and ten generator output suffixes. The body is rewritten: it states what the probe moves, why the list puts it back, what the tool carves out for itself, and what it does not.

    2. The fixture pair. The fail fixture gains an undocumented getter and setter, which are the carve-out the tool drops. The pass fixture gains the carve-outs the tool makes for itself: `_` privacy, three overrides, and the `// ignore: public_member_api_docs` marker. Measured: fail = 5 findings, pass = 0.

    3. Two acceptance tests on the real pipeline, in `shipped.rs`. `..._reports_every_fail_fixture_line` holds the run to exactly the five undocumented lines. `..._reads_only_the_package_library` stages the same undocumented class in `lib/staged.dart`, `test/staged_test.dart` and `lib/staged.g.dart` and holds the run to the two findings in the `lib/` file alone.

    **RED proved.** With the exclude list taken back out of the rule, `..._reads_only_the_package_library` fails: `left: ["lib/staged.dart", "lib/staged.dart", "lib/staged.g.dart", "lib/staged.g.dart", "test/staged_test.dart", "test/staged_test.dart"]`, `right: ["lib/staged.dart", "lib/staged.dart"]`. The rule was restored and the suite re-run green.

    **Two design decisions, and why.**

    A `lib/`-only filter in the copy loop is not possible. The doctor materializes a fixture flat as `missing-docs-dart.fail.dart` with no directory, so such a filter would skip the fail fixture and the pair would fail. The exclude list keeps the loose fixture and drops every other position. Verified over 19 staged paths.

    `example/` is `**/example/*.dart` and not `**/example/**`, because many repositories put a package of its own under `example/`, and that package's `lib/` is a public API. Measured: `example/lib/a.dart` reports and `example/a.dart` stays silent.

    **Discovered while measuring, and it changed the fixture.** A setter that has no getter of the same name reports NOTHING. Measured: `set loneSetter(int next) {}`, the same setter with a block body, and a top-level setter are each silent, while `set pairedValue(int next)` beside `int get pairedValue` reports. The first fail fixture used an unpaired setter and reported 4 findings rather than 5. The fixture now pairs them, and the rule body states the behaviour.

    **Duplication.** The source-line extraction closure stood three times in `shipped.rs`. It is now the named `fail_fixture_source_line`, and the three call sites plus my new one use it. `sah tool code_context duplication find` over the two changed Rust files reports one finding, `dead_code_work` against `complexity_work`, and both pre-date this change.

    **Not done here, raised as a card.** ^2vxg70a — the probe declares no dependencies, so an `@override` of a member from an unresolved import reports. Every Flutter `build()` is a false finding. `--packages` fixes it, measured.

    **Not touched.** `builtin/validators/code-hygiene/rules/missing-docs.md`. Its closing note yields the "obvious implementation" and "simple getter" carve-outs to the Swift and Rust rules only, and Dart now drops the getter carve-out too. That file is shared with the five sibling cards of this batch, so a change there would go stale. The Dart rule body states the gap and names the inline suppression as the recourse.

    - evidence: 5 files — `builtin/validators/code-hygiene/rules/missing-docs-dart.md`, `builtin/validators/code-hygiene/fixtures/missing-docs-dart.fail.dart.tmpl`, `builtin/validators/code-hygiene/fixtures/missing-docs-dart.pass.dart.tmpl`, `crates/swissarmyhammer-validators/src/review/tool_rules/tests.rs`, `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs`. `cargo nextest run -p swissarmyhammer-validators` 583 passed, 0 failed, 0 skipped. `cargo clippy --workspace --all-targets -- -D warnings` clean. `cargo fmt --all` clean.
    - next: review
  timestamp: 2026-08-11T02:08:09.271799+00:00
- actor: claude-code
  id: 01kzq9jvhf4jgx6182y7k5hkgc
  text: |-
    ### review — clean
    - evidence: `review sha HEAD~1..HEAD` (09b658343) — 8 validators attempted, 1 finding, 1 confirmed, 0 refuted, 0 failed, 0 skipped. The one finding was `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:207` `dead_code_work` as a near-duplicate of `complexity_work` at line 121. Both functions are identical and at the same lines in HEAD~1 and HEAD, so the commit did not touch them. The written rule that drops a finding about test code that already existed applies. Zero actionable findings remain.
    - evidence: no prior `## Review Findings` section is on this card, so no earlier item is open.
    - evidence: staleness check across the six `missing-docs-*` rules. No sibling rule names `missing-docs-dart`, and the changed rule names no sibling. No sentence is stale.
    - evidence: objectivity check on the changed rule body. Each claim carries a measurement on Dart SDK 3.11.0 with the count that was seen. Each exemption is a tool setting (`analyzer: exclude:`, `// ignore: public_member_api_docs`, `// ignore_for_file:`) or a structural fact (a generator output suffix, the Dart `_` privacy prefix, the resolution of an override by the analyzer). No exemption asks a reader to make a decision. The rule also states its own limit: the probe package declares no dependencies, so an `@override` of a member that `package:flutter` declares does not resolve and does report.
    - next: none. The card moves to `done`.
  timestamp: 2026-08-11T02:14:20.463916+00:00
- actor: claude-code
  id: 01kzq9kfvvarps4w6nwfvsvn8s
  text: |-
    ### finish iteration 1 — clean
    - implement: changed — 5 files; the probe analysis_options.yaml now carries an analyzer exclude list; one card claim refuted (an override needs no doc comment)
    - test: green — cargo nextest run -p swissarmyhammer-validators, 583 passed; RED proved (6 findings without the exclude list, 2 with it); clippy -D warnings clean
    - commit: 09b658343 — 5 files
    - review: clean — 1 engine finding in pre-existing test code, dropped by the written exception; card moved to done
  timestamp: 2026-08-11T02:14:41.275596+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffe080
title: missing-docs-dart copies test and generated files into the probe package, where the lint applies
---
`builtin/validators/code-hygiene/rules/missing-docs-dart.md` builds a temporary probe package, copies each changed file to `<probe>/lib/<path>`, and runs `dart analyze` with an `analysis_options.yaml` that enables `public_member_api_docs`. It declares `supersedes: [missing-docs]`.

The probe design MAKES two kinds of finding the prompt rule does not.

- Tests. `public_member_api_docs` reports only for a file inside a package's `lib/` directory, which is why the probe exists. Dart test files live in `test/`, never `lib/`, so the real analyzer never reports them. This script copies every changed file, `test/foo_test.dart` included, into `<probe>/lib/`, where the lint does apply. A public helper class in a Dart test file is reported only because the rule moved it.
- Generated code. Dart projects exclude `*.g.dart` and `*.freezed.dart` through `analyzer: exclude:` in their own `analysis_options.yaml`, and this rule writes its own, so "The project's own `analysis_options.yaml` is never read." A changed `model.g.dart` is reported in full.

Both are CONFIRMED by measurement on Dart SDK 3.11.0. See the measurement comment.

One carve-out of `missing-docs.md` is dropped: "Simple getters/setters with self-explanatory names" — every public getter and setter is reported. CONFIRMED by measurement.

CORRECTED — the card's third claim is REFUTED. The card said the "Obvious implementations (Display, Debug, ToString, etc.)" carve-out is dropped, so `toString()` and `operator ==` need a doc comment. Measured: they do NOT. `public_member_api_docs` carves out every member that overrides a member the analyzer can resolve. `toString()` without `@override`, `@override bool operator ==`, `@override int get hashCode` and an `@override` of a documented base method each report nothing. `void main()` is carved out too.

The private-item carve-out IS reproduced, from Dart's `_` prefix privacy.

Decide which files the probe copies, and whether the probe's `analysis_options.yaml` carries an `exclude:` list.

Found by the `supersedes` survey on ^h7garpc. #tool-validators #objectivity