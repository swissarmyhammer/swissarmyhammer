---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m01gvq5qhf6wywj55pwwr4rq
  text: |-
    Picked up. Research done.

    Reproduced the defect on Dart SDK 3.11.0 with the card's two-package probe (`framework` holds a documented `abstract class Widget` with a documented `String build()`; `app` path-depends on it and holds `lib/screen.dart` with one `@override String build()` and one undocumented `void undocumented()`):

    - `app` analyzed in place with `public_member_api_docs` on: 1 finding, the undocumented method at line 8. The override is silent.
    - the shipped probe over the same file: 2 findings, lines 6 and 8. Line 6 is the override.
    - the probe run with `--packages=<app>/.dart_tool/package_config.json`: 1 finding, line 8. Matches the project.

    Exit-status measurements made while deciding the missing-config answer:
    - `dart analyze --format=machine` exits 0 for an INFO lint finding, and exits 3 when a file holds an unresolved import. The shipped pipe ends in `awk`, so the script takes awk's status either way.
    - `--packages=<file that does not exist>` exits 64 and writes a usage error. The run must therefore test the file before it names it.
    - `--packages=<file that is not JSON>` exits 0 AND still reports the lint. A config the analyzer cannot read is a superset, never silence.

    Engine facts the script has to answer for: a `files`-scope script receives the work-list paths, which are repo-relative, and runs with the repository root as its working directory. So the walk up for `.dart_tool/package_config.json` resolves each argument with `cd "$(dirname "$file")" && pwd -P` first.

    The harness for the acceptance test is `ShippedStagedTree` plus `drive_shipped_staged_tree_with`, which stages files the work-list does NOT name. That is how the probe carries `framework/`, the two `pubspec.yaml` files and `app/.dart_tool/package_config.json` without handing them to the script as arguments.
  timestamp: 2026-08-15T01:33:55.255037+00:00
- actor: claude-code
  id: 01m01hmhf7t408kjgnbqer137h
  text: |-
    Implementation landed. Both open questions answered, each by a measurement and each held by an acceptance test.

    **1. Where the package config comes from.** The run reads the config of the FILE, never one config for the workspace. For each argument it resolves the file's own directory with `cd "$(dirname "$file")" && pwd -P` and walks up to the first directory holding `.dart_tool/package_config.json`. `dart analyze` takes one `--packages` for one run, so the run groups its arguments by the config found for each and builds one probe package for each group.

    Measured over a monorepo probe of `packages/alpha` and `packages/beta`, each path-depending on a framework of its own and each holding a config of its own:

    | the run | rows reported |
    |---|---|
    | one probe package for each config | alpha 8, beta 8 |
    | one probe package under the alpha config | alpha 8, beta 6 and 8 |
    | one probe package, no `--packages` | alpha 6 and 8, beta 6 and 8 |

    The grouping is what tells row 1 from row 2, so it is load-bearing rather than decoration.

    **When there is none:** the run falls back to the probe package alone, which is exactly what the rule did before. It does not break, and it does not fall silent — the answer is a SUPERSET (the override reports beside the member that really is undocumented). Two measurements decided this over the alternatives:
    - `--packages=<file that does not exist>` exits 64 and lints nothing, so naming a missing file would break every project that has not run pub get.
    - `--packages=<file that is not JSON>` exits 0 and still reports, so a config the analyzer cannot read is a superset too, never silence.

    **2. What the doctor run does.** The doctor materializes fixtures into a scratch directory that holds no project, so the walk finds no config above it and the run takes the fallback. The fixture pair is unchanged. Measured over the shipped fixtures in such a directory, with the flag and without: the fail fixture reports rows 15, 18, 20, 22 and 25, and the pass fixture reports nothing, at exit 0 each time. The pass fixture only overrides `toString`, `operator ==` and `hashCode`, and `Object` resolves with no config, so the carve-out it rests on never needed one. `every_shipped_missing_docs_tool_rule_passes_its_fixtures` passes.

    Three acceptance tests on the shipped bytes, each watched RED first where it could be:
    - `the_shipped_dart_missing_docs_tool_rule_reads_the_package_config_of_the_file` — RED before the fix (reported rows 6 and 8, wanted 8), GREEN after.
    - `the_shipped_dart_missing_docs_tool_rule_reads_one_package_config_for_each_package` — RED before the fix, GREEN after.
    - `the_shipped_dart_missing_docs_tool_rule_reports_the_override_with_no_package_config` — passes before and after by design. It is the control that states the fallback is a superset rather than silence; it would go RED if a later change made the missing-config path answer nothing.

    Two shapes found and corrected along the way, both stated in the rule body:
    - The zero-argument shape changed meaning. Before, a run with no argument still cost a `dart pub get` and an analysis pass over an empty package. Now the empty file list names no config, the package loop runs no time, and no analyzer runs at all. `: > "$work/files"` was added so the grouping `awk` never reads a file that does not exist.
    - The analyzer configuration is written once into the working directory and copied into each probe package. A heredoc nested inside the package loop would carry the loop's indentation into the YAML it writes.

    Verification: `cargo nextest run -p swissarmyhammer-validators` 756 passed, 0 failed. `cargo nextest run -p mirdan` 522 passed. `cargo nextest run -p swissarmyhammer-tools -E 'test(review)'` 86 passed. `cargo fmt --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.

    ARCHITECTURE.md needs no update: the change stays inside `swissarmyhammer-validators` and one builtin rule file, and creates no module and no dependency edge.

    ### implement — changed
    - evidence: 3 files — builtin/validators/code-hygiene/rules/missing-docs-dart.md, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs
    - next: /review
  timestamp: 2026-08-15T01:47:28.615851+00:00
position_column: doing
position_ordinal: '8280'
title: missing-docs-dart probe resolves no dependency, so every Flutter @override build() reports
---
`builtin/validators/code-hygiene/rules/missing-docs-dart.md` builds a probe package that declares no dependencies. An import of `package:flutter/material.dart` therefore does not resolve inside the probe.

`public_member_api_docs` carves out a member that overrides a member the analyzer CAN RESOLVE. The documentation of the overridden member stands for it. When the supertype does not resolve, the analyzer sees no override, and the member reports.

The result: every `@override Widget build(BuildContext context)`, every `createState`, `initState`, `dispose` and `didUpdateWidget` of a Flutter project reports a missing doc comment. The project's own `dart analyze` reports none of them.

Measured on Dart SDK 3.11.0 with a two-package probe. Package `framework` declares `abstract class Widget { String build(); }`, both documented. Package `app` path-depends on it and holds `lib/screen.dart` with `class Screen extends Widget`, one `@override String build()` and one undocumented `void undocumented()`.

- The `app` package analyzed in place, with `public_member_api_docs` on: **1** finding, the undocumented method. The override is silent.
- The shipped probe over the same file: **2** findings. The override is the false one.
- The probe run as `dart analyze --packages=<app>/.dart_tool/package_config.json <probe>`: **1** finding, the same as the project.

`--packages` restores the carve-out and the lint still fires, so the fix is one flag. Two questions the work has to answer:

- Where the package config comes from. The workspace root holds `.dart_tool/package_config.json` only after the project ran `flutter pub get` or `dart pub get`. A monorepo holds one per package.
- What the doctor run does. The doctor runs the script in a scratch fixtures directory that holds no project and no package config, so the flag has to be conditional or the fixture pair breaks.

`Object` always resolves, so `toString`, `operator ==` and `hashCode` keep the carve-out with or without this fix.

Found while implementing ^j0g7yk1, which fixed a different axis of the same probe (the position and generator exclude list). #tool-validators #objectivity