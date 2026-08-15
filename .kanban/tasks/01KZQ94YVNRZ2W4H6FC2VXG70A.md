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
- actor: claude-code
  id: 01m01j3yxg4t670jnpxsf411cp
  text: |
    ### review — findings
    - evidence: engine `review sha 0d813edce~1..0d813edce` returned 0 findings over 2 Rust files. 1 hand finding on builtin/validators/code-hygiene/rules/missing-docs-dart.md:75 — unchecked `dart pub get --offline` silences the whole rule at exit 0. Measured: 2 rows after a pub get that succeeds, 0 rows when it fails.
    - next: read the exit status of `dart pub get` and fail loudly. Task stays in review.
  timestamp: 2026-08-15T01:55:53.904813+00:00
- actor: claude-code
  id: 01m01k8x2t0ttt178ts8cah4fr
  text: |-
    Pulled back from review. Finding worked.

    **The finding, reproduced through the shipped bytes.** With the script as it stood at HEAD and `dart pub get` replaced by a command that exits 127, the run answered `Ok([])` — 0 rows, exit 0 — over a probe package holding one undocumented class and one undocumented method. Two rows when the same probe runs with `pub get` standing. That is the clean-file shape.

    **The fix follows dead-code-rust / function-length-python / missing-docs-python.** Each tool call goes into a file, the status is captured, the status is tested, the run writes its own line and exits 1.

    - `dart pub get --offline --directory "$package" > "$work/pub-get" 2>&1 || pub_status=$?`, then a test of the status AND of `$package/.dart_tool/package_config.json`. The artifact test is the precondition the rule body names.
    - `dart analyze ... > "$work/analysis" 2> "$work/analysis-error" || analysis_status=$?`, then `[ "$analysis_status" -gt 3 ]`. The pipe into `awk` is gone; `awk` reads the file.

    **Measured status sets, Dart SDK 3.11.0.** `dart analyze`: 0 for an INFO lint alone, 1 under `--fatal-infos`, 2 for a WARNING, 3 for an ERROR — each of the four writes its rows to stdout — and 64 for the usage error, which judges nothing. So 0..3 are measured runs. 3 is load-bearing, not tolerated: the missing-config fallback leaves every `package:` import unresolved, which is an ERROR, so the correct fallback run takes status 3. `dart pub get`: exit 1 with no `.dart_tool` when the SDK stands outside the declared `>=3.0.0 <5.0.0` window.

    **`--offline` stays.** The probe package declares no dependency, so pub needs nothing from the network and nothing from the cache. Measured with `PUB_CACHE` naming an empty directory, a directory that does not exist, and a directory that cannot be written: `dart pub get --offline` exits 0 and writes the package config each time, and a run without the flag answers the same. So it cannot fail where a networked run would stand. Note against the finding's second cause: an unwritable `PUB_CACHE` does NOT reach the failure for this zero-dependency probe. The SDK constraint does.

    **Sweep: 18 commands threw their status away.** The two `dart` calls are two of them. The other sixteen: `mktemp`, two `pwd -P` resolutions, two template writes, four list writes, the `awk` that headed the config pipe, two `mkdir -p`, three `cp`, and the reporting `awk` (whose status was read on the last group alone). `set -e` at the head of the script now covers every one. Two commands had to be reshaped so `set -e` could reach them at all:
    - `awk ... | sort -u` became two commands writing two files. A pipeline takes the status of its last command alone and the script writes no `pipefail`.
    - `dart pub get` runs under `--directory` rather than inside `(cd "$package" && ...)`. Every command of the script now runs at the repository root, and no subshell stands between a failure and the script.

    Also added the per-file readability test the README asks for (`missing-docs-dart cannot read <path>`). `set -e` alone answers such a run with `cd: lib: No such file or directory`, which names a temporary path of the probe rather than the file the review handed over.

    **Four acceptance tests on the shipped bytes, each watched RED first.**
    - `..._breaks_when_pub_get_cannot_run` — RED `Ok([])` with the script as it stood; GREEN after.
    - `..._breaks_when_the_analyzer_cannot_run` — RED `Ok([])`; GREEN after.
    - `..._breaks_on_a_file_it_cannot_read` — RED on the wrong line (`bash: line 35: cd: lib: No such file or directory`); GREEN with the named line.
    - `..._reports_both_members_when_dart_runs` — the control. Same staged file, neither run stubbed, two rows. It stops the two status tests from being satisfied by a gate that breaks every run.

    The stub helper `verify_shipped_tree_breaks_without` was generalized to `verify_shipped_tree_breaks_without_run_of(probe, binary, subcommand)`. The script calls one binary two times, so a probe of one status has to leave the other standing. `subcommand: None` writes the same stub bytes as before, so the four existing callers are unchanged.

    Verification: `cargo nextest run -p swissarmyhammer-validators` 760 passed, 0 failed. `cargo nextest run -p mirdan` 522 passed. `cargo nextest run -p swissarmyhammer-tools -E 'test(review)'` 86 passed. `cargo fmt --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean. The doctor fixture pair is unchanged and `every_shipped_missing_docs_tool_rule_passes_its_fixtures` passes.

    ARCHITECTURE.md needs no update: the change stays inside `swissarmyhammer-validators` and one builtin rule file, and creates no module and no dependency edge.

    ### implement — changed
    - evidence: 3 files — builtin/validators/code-hygiene/rules/missing-docs-dart.md, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs
    - next: /review
  timestamp: 2026-08-15T02:16:04.442228+00:00
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

## Review Findings (2026-08-14 20:49)

> Scope: `review sha 0d813edce~1..0d813edce` — reviewed the diffs only — lines this change added or modified. 2 file(s) reviewed, 0 not reviewed.

The engine reported 0 findings over the 2 Rust files. It read NO `.md` file: no
validator declares a `*.md` glob, so the 214-line rule body — the substance of
this commit — entered no candidate set. Carded as ^j169agt. The finding below
comes from reading that file by hand, and is measured, not argued.

- [x] `builtin/validators/code-hygiene/rules/missing-docs-dart.md:75` `hand/uncovered-md` — `(cd "$package" && dart pub get --offline) > /dev/null 2>&1` discards both streams and never reads the exit status. When that command fails the probe package holds no `.dart_tool/package_config.json`, and the rule then reports NOTHING at exit 0, which the engine reads as a clean file. Measured on Dart SDK 3.11.0: a probe package holding one undocumented class and one undocumented method reports 2 rows after a `pub get` that succeeds, and 0 rows at exit 0 when `pub get` fails and writes no `.dart_tool`. An SDK outside the declared `>=3.0.0 <5.0.0` constraint, or an unwritable `PUB_CACHE`, reaches that state. Read the exit status of `dart pub get` and fail the run loudly instead of analyzing a package the analyzer does not recognize. Note the provenance: this commit MOVED this line into the per-group loop and reindented it, so git marks it changed, but the discarded exit status is PRE-EXISTING — the same unchecked call stood at the top level before this commit. This commit multiplies it across one call per config group.

### The fallback claim holds

The implementer argues that when no package config is found the run falls back
to the probe package alone and "cannot read as clean" because the answer is a
SUPERSET. Checked against the shipped script, that reasoning HOLDS:

- The fallback command at line 77 is byte-identical to the command this commit
  replaced, so the fallback is the behaviour the rule already had.
- A genuinely undocumented member is not an override, so resolution cannot
  silence it. Only the false override is added.
- `verify_supported_rows_report` asserts with `assert_eq!` over sorted names,
  so it is an EXACT set and not a subset. The test
  `the_shipped_dart_missing_docs_tool_rule_reports_the_override_with_no_package_config`
  names BOTH rows, so a fallback that fell to one row or to silence fails.
  Confirmed passing.

The silence risk this review found is NOT on the fallback path. It is the
unchecked `pub get`, which silences every path including the correct one.
