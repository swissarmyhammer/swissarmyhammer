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
- actor: claude-code
  id: 01m01m2mdjq4zk973bez7s5bjy
  text: |
    ### review — findings
    - scope: `review sha b804e667b~1..b804e667b` — diffs only, 2 files reviewed, 0 not reviewed
    - evidence: 2 findings — crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs:457, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs:460
    - prior round: the one finding of 2026-08-14 20:49 is checked
    - both findings land on added lines; the one hunk of that file is `@@ -426,0 +427,184 @@`
    - hand review of the rule body (the engine reads no `*.md`, ^j169agt): `set -e` reaches all 18 commands, measured by stubbing mktemp, sort, cp, mkdir and awk to exit 9 — each stops the run at 0 rows. No pipeline and no subshell is left. The status gate `-gt 3` accepts 0 to 3 at 2 rows and breaks 4, 5, 64 and 127 at exit 1, so 64 is genuinely excluded. Status 3 is load-bearing, checked on its own: an unresolved `package:` import makes raw `dart analyze` exit 3 and the script still reports both rows.
    - dropped for a false premise: 3
    - next: reuse `DART_BROKEN_RUN_PATH` in `DART_BROKEN_RUN_ROWS`, and name the rule string once for both error arrays
  timestamp: 2026-08-15T02:30:07.538666+00:00
- actor: claude-code
  id: 01m01mseymc2z89mmvstrpxdnw
  text: |
    Pulled back from review. Both findings worked, and the cause swept over the whole file.

    **Both premises checked before acting, and both hold.** Line 457 spelled `lib/broken_run.dart` twice although `DART_BROKEN_RUN_PATH` names it four lines above. Lines 460 and 476 each spelled `missing-docs-dart`, which `DART_MISSING_DOCS_RULE` in `tests.rs` already names. No conflict to record.

    **The sweep.** Two detectors over the whole file, each comparing a constant VALUE against every string literal:
    - substring of a `&str` constant inside a literal: 125 raw hits.
    - an inline `&[&str]` slice literal equal to a slice constant: 10 hits.

    The 125 fall out to 12 real restatements. 113 are coincidental substrings of a short generic value: `pub` inside `public` and `pubspec`, `go` and `dart` as a file extension, `documented.py` inside `undocumented.py`, which means the opposite. Two more were the second line of a constant's own definition. So the file held **24 restatements**, of which the review named 3 sites:

    | what a literal restated | sites |
    |---|---|
    | `DART_BROKEN_RUN_PATH` in the two `path:line` rows | 2 (the finding) |
    | `DART_MISSING_DOCS_RULE` in the two error lists | 2 (the finding) |
    | a rule name inside a fail fixture NAME | 5 |
    | a rule name inside a fixture PATH | 1 |
    | a rule name inside the line a script writes for a file it cannot read | 3 |
    | a rule name inside swiftlint's undecodable-file line | 1 |
    | `FLUTTER_PROJECT_TYPES`, `GO_PROJECT_TYPES`, `PYTHON_PROJECT_TYPES` as `project_types: &["..."]` | 10 |

    All 24 are gone. Both detectors now answer 0 over the file.

    **How.** `concat!` takes literals, so a constant cannot feed it, and a `&'static [&'static str]` field cannot hold a value built at runtime. `stuttering_name_go.rs` already answers this shape with `go_carve_out_source!`, and this follows it:
    - `missing_docs_rule!(dart|go|python|rust|swift|typescript)` in `tests.rs`, one arm per shipped rule. Each rule-name constant is now its arm, and every longer string spelling a rule name is `concat!` over it. It stands above the `mod` declarations, because `macro_rules!` is scoped by position and a module declared above a macro cannot see it.
    - `dart_broken_run_path!()` in `missing_docs.rs`, feeding both `DART_BROKEN_RUN_PATH` and the two rows.

    The reviewer's second remedy, a lazily-evaluated constant, was rejected on measurement: `ShippedRun.expected` is `&'static [&'static str]` and every probe is a `const` item, so a `LazyLock<Vec<String>>` cannot reach it without a lifetime parameter on four probe structs and every one of their 40-odd users.

    **One restatement left standing, named here rather than hidden.** `SHIPPED_RULES_THAT_READ_A_GO_FILE` spells 26 fully-qualified rule ids, and 13 of them open with `code-hygiene/`. It is the expected value of a matcher test, its doc tells the next reader to add a name to it by hand, and 13 of the 26 name sets and rules that have no constant anywhere. Composing half the list from macros would leave one list in two spellings. It is pre-existing test code that this commit does not touch.

    **Verification.** `cargo nextest run -p swissarmyhammer-validators` 760 passed, 0 failed, 0 skipped. Every shipped test really ran its tool: `require_tool_installed` PANICS on a missing tool rather than skipping, and `dart`, `revive`, `ruff` and `swiftlint` are all installed here, so each renamed fixture, fixture path and error line was exercised end to end. `cargo fmt --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.

    ARCHITECTURE.md needs no update: the change stays inside the test tree of `swissarmyhammer-validators` and creates no module and no dependency edge.

    ### implement — changed
    - evidence: 2 files — crates/swissarmyhammer-validators/src/review/tool_rules/tests.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs
    - next: /review
  timestamp: 2026-08-15T02:42:35.604248+00:00
- actor: claude-code
  id: 01m01n1gn1z1p37a6aajp8x1ev
  text: |-
    ### review — clean
    - evidence: `review sha 714a537db~1..714a537db` — 0 findings, 0 confirmed, 0 refuted; 9 validators attempted, 0 failed, 0 skipped; 2 file(s) reviewed, 0 not reviewed. Round 3. All prior findings from 2026-08-14 20:49 (1 item) and 2026-08-14 21:18 (2 items) are checked.
    - next: task moved to done. The macros this commit added to remove restated literals raised no finding. No new cause. No fourth round.
  timestamp: 2026-08-15T02:46:59.489978+00:00
- actor: claude-code
  id: 01m01n2zj5m82jhsvtp272bz0b
  text: |-
    ### finish — done. Converged in 3 rounds (1, 2, 0 findings).
    - **Round 1**: the probe now reads the config OF THE FILE, never one config for the workspace — for each argument it walks up to the first `.dart_tool/package_config.json`, and because `dart analyze` takes one `--packages` per run, it GROUPS its arguments by the config found and builds one probe package per group. Measured over a two-package monorepo: per-config grouping reports alpha 8 and beta 8; one package under the alpha config reports alpha 8 and beta 6+8; no flag reports 6+8 in both. The grouping is what separates the first row from the second. No-config fallback is the prior behaviour and cannot read as clean, because the answer is a SUPERSET.
    - **Round 2 finding, and it was real**: `dart pub get` discarded both streams and never read its status, so a failed pub get left no package config and the run answered 0 rows at exit 0 — reproduced by replacing pub get with a command exiting 127. Fixed in the established shape. The sweep found **18** commands discarding status, not one; `set -e` now reaches all of them, and two were reshaped because a pipeline hid one and a subshell hid the other. Measured analyze statuses: 0/1/2/3 all write rows, 64 judges nothing, so the gate is `> 3`. Status 3 is LOAD-BEARING, not tolerated — the no-config fallback leaves every `package:` import unresolved, which is an ERROR. `--offline` stays, measured across three PUB_CACHE states, which also CORRECTED the finding's claim: an unwritable cache does not reach the failure for a zero-dependency probe; the SDK constraint does.
    - **Round 3**: two restated literals in the round-2 tests. The sweep used the substring detector rather than whole-literal equality and found **24 real restatements at 24 sites** where the review named 3 — 113 of 125 raw hits being coincidental (`pub` inside `pubspec`, `documented` inside `undocumented`, which means the opposite). All 24 gone, both detectors now answer 0. `concat!` takes literals and `expected` is a static slice on const probes, so a macro answers it, following the shape `stuttering_name_go.rs` already uses.
    - **The reviewer verified both safety questions rather than accepting them**: stubbing each swept command to exit 9 stops the whole run; forcing statuses 0/1/2/3 gives exit 0 with 2 rows while 4/5/64/127 give exit 1 with 0 rows; and status 3's load-bearing claim was checked independently. It also dropped 3 candidate findings on evidence.
    - Rounds 1 and 2 each needed a HAND review of the rule body, because no validator declares a `*.md` glob (^j169agt). Round 3 touches no rule body, so it has no unreviewed substance.
    - commits: 0d813edce, b804e667b, 714a537db. test: green — 760 validators, 522 mirdan, 86 review tests. fmt and clippy clean.

    **Not the ^4kzxdex churn pattern**, and the reviewer named why: round 2's fix was STRUCTURAL rather than local — 24 sites swept with two detectors that now answer zero, instead of the 3 the review named. A local fix would have left the same class ready to fire on the next commit. It did not fire.

    One restatement is left standing and named: `SHIPPED_RULES_THAT_READ_A_GO_FILE`, a roster of 26 fully-qualified rule ids whose doc tells the next reader to add a name by hand, and 13 of whose entries name sets and rules with no constant anywhere. Pre-existing, untouched.
  timestamp: 2026-08-15T02:47:47.525491+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffff8b80
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

## Review Findings (2026-08-14 21:18)

> Scope: `review sha b804e667b~1..b804e667b` — reviewed the diffs only — lines this change added or modified. 2 file(s) reviewed, 0 not reviewed.

- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs:457` `code-hygiene/magic-numbers` — The path "lib/broken_run.dart" is hardcoded in test row strings; it is already extracted as DART_BROKEN_RUN_PATH on line 444 and should be reused to avoid maintenance drift. Refactor DART_BROKEN_RUN_ROWS to construct strings using DART_BROKEN_RUN_PATH, or use a lazily-evaluated constant to build the rows with the path constant at runtime.
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs:460` `code-hygiene/magic-numbers` — The rule name "missing-docs-dart" is hardcoded in the error message; it appears again on line 476 and should be extracted to a shared named constant. Extract "missing-docs-dart" to a constant (e.g., const DART_RULE_NAME: &str = "missing-docs-dart";) and reference it in both error arrays.

Both findings land on lines this commit ADDED. The one hunk of that file is
`@@ -426,0 +427,184 @@`, so every line from 427 to 610 is new.

### The rule body, read by hand

The engine read the two Rust files and NO `.md` file. No validator declares a
`*.md` glob (^j169agt), so the 166-line delta of the rule body entered no
candidate set again. The two questions below were measured against the SHIPPED
bytes, extracted from the `run:` block, on Dart SDK 3.11.0. Both answers are
clean, so neither raises a finding.

**`set -e` reaches all eighteen commands.** No pipeline and no subshell stands
in the script now. Every `|` that is left is either a `||` that captures a
status the run then tests, or the literal `|` inside `awk -F'|'` and the awk
program. The only parentheses left are inside that awk program. The two
reshapes the commit names are both real: `awk ... | sort -u` is now two
commands writing two files, and `dart pub get` runs under `--directory`
instead of `(cd "$package" && ...)`.

Measured, with each command replaced by one that exits 9:

| stubbed command | script exit | rows |
|---|---|---|
| `mktemp` | 9 | 0 |
| `sort` | 9 | 0 |
| `cp` | 9 | 0 |
| `mkdir` | 9 | 0 |
| `awk` | 9 | 0 |

Each one stops the run. The two `dart` calls are covered by the status they
capture and test. So no swept command can fail without stopping the run.

**The tolerated status range is not too broad.** The gate is
`[ "$analysis_status" -gt 3 ]`, so 4 through 255 all break. Measured through
the shipped script, with `dart analyze` forced to each status:

| analyze status | script exit | rows |
|---|---|---|
| 0, 1, 2, 3 | 0 | 2 |
| 4, 5, 64, 127 | 1 | 0 |

64, the usage error, is genuinely excluded. Nothing else passes.

Status 3 is load-bearing, and that claim was checked on its own rather than
taken from the commit message. A file importing `package:flutter/material.dart`
with no package config above it makes the raw `dart analyze` exit 3, and the
shipped script still reports both rows at exit 0. A gate that rejected 3 would
break every project that has not run `pub get`.

Three concerns were formed and then dropped for a false premise: the status of
`dirname` inside `mkdir -p "$(dirname "$copy")"` (`dirname` cannot fail on a
non-empty argument, and `mkdir` then fails loudly anyway); `dart` eating the
stdin of the `while read config` loop and dropping a group (the two-group
acceptance test and the measured runs both complete every group); and
`rm -rf ""` from the EXIT trap if the `pwd -P` assignment fails (it removes
nothing).
