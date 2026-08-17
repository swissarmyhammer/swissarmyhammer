---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m075m09wrvvpvf8rw8ebre90
  text: |-
    Research done. Measured with swiftlint 0.65.0 and Dart SDK 3.11.0.

    swiftlint, `missing_docs` child configuration, one healthy file beside the refusing path:

    | the refusing path | status | stdout | stderr |
    |---|---|---|---|
    | a path that holds no file | 0 | 2 entries | NOTHING |
    | a file whose bytes are not UTF-8 | 0 | 2 entries | `Could not read contents of `<absolute path>`` |
    | a file with no read permission | 0 | 2 entries | the same decode line |

    So swiftlint names the file for two of the three shapes, and it says NOTHING for a path that holds no file. Measured again without `--quiet`: swiftlint writes `Linting 'Docs.swift' (1/1)` and no word of the path it dropped. The path that holds no file therefore takes a test of its own, `[ ! -e "$file" ]`, and the other two take a read of swiftlint's own message.

    `dart analyze` states NOTHING for each of the three shapes. Measured over a probe package that holds one undocumented class beside a non-UTF-8 file and a file with no read permission: 2 rows for the healthy file, exit 0, 0 bytes on stderr, and no word of the other two. `dart analyze --help` names 3 options and none of them names a decode failure; `dart --verbose analyze` over the non-UTF-8 file writes 0 bytes. `cp` breaks first for the two shapes it cannot copy, and `set -e` then breaks the whole run.

    `function-length-dart` already holds the worked answer for Dart, and this card takes its shape: a `[ ! -r "$file" ]` test and an `iconv -f UTF-8 -t UTF-8` test, each writing a `sah-diagnostic:` line and `continue`. Measured `iconv -f UTF-8 -t UTF-8`: exit 0 for a healthy file, exit 1 for each of the three refusing shapes and for a directory nobody may read.

    Discovery: `function-length-swift` holds the same pre-flight guard and the same `exit 1`. It stands outside this card.
  timestamp: 2026-08-17T06:12:54.716447+00:00
- actor: claude-code
  id: 01m076yws0579051m6gx2zwvqq
  text: |-
    Implementation landed. What each rule now does, and the measurement behind it.

    ## `missing-docs-swift` and `magic-numbers-swift`

    The pre-flight loop no longer exits. It tests `[ ! -e "$file" ]` and writes a `sah-diagnostic:` line, because swiftlint says NOTHING for a path that holds no file. The decode `grep`/`exit 1` became a `sed` substitution that takes swiftlint's own `Could not read contents of `<path>`` line and writes it under the marker. That one line answers the non-UTF-8 file AND the file with no read permission, which is what the earlier `[ ! -r ]` test could not do.

    `check_command` now names `sed`, and it names `cat`, which the script also calls.

    ## `missing-docs-dart`

    `dart analyze` states NOTHING for any refusing shape, so the script tests each path itself, in the shape `function-length-dart` already carries: `[ ! -r "$file" ]` and `iconv -f UTF-8 -t UTF-8`, each writing a marked line and `continue`. `continue` takes the file out of the work list, so the `cp` that broke the run under `set -e` is never made.

    `check_command` now names `iconv`, and it names `cp mkdir dirname cat`, which the script also calls.

    ## RED, then GREEN

    RED, with the earlier guards put back and the nine new tests run: 9 tests, 0 passed, 9 failed. Each failed for the reason the card names.

    | the test | the failure |
    |---|---|
    | swift missing-docs, absent | `Exit("missing-docs-swift cannot read Sources/Unreadable.swift")` |
    | swift missing-docs, decode | `Exit("Could not read contents of `<path>`\nmissing-docs-swift: swiftlint could not read the contents of a file this run names")` |
    | swift missing-docs, no permission | the same `cannot read` exit |
    | swift magic-numbers, three shapes | the same three shapes with its own line |
    | dart, absent | `Exit("cp: lib/unreadable.dart: No such file or directory")` |
    | dart, no permission | `Exit("cp: lib/unreadable.dart: Permission denied")` |
    | dart, decode | `the run must state the one item it declined; it stated []` — the SILENT clean answer |

    GREEN, with the shipped shape: `cargo nextest run -p swissarmyhammer-validators` reports 803 tests run, 803 passed, 0 skipped. `cargo clippy -p swissarmyhammer-validators --all-targets` reports 0 warnings. `cargo nextest run -p mirdan` reports 503 passed.

    ## What each shipped run now answers

    Measured with swiftlint 0.65.0 over one file that holds findings, beside each refusing path: the findings stand on stdout, ONE marked line stands on stderr, and the run exits 0. Measured over the same file beside all three paths: the findings stand, 3 marked lines stand, exit 0.

    Measured with Dart SDK 3.11.0 over `lib/judged.dart` beside each refusing path: 2 rows on stdout, 1 marked line on stderr, exit 0. Beside all three: 2 rows, 3 marked lines, exit 0.

    ## Discovery

    `function-length-swift` holds the same guard and the same `exit 1`. It stands outside this card, so it is now `^8491bb0`.
  timestamp: 2026-08-17T06:36:20.128558+00:00
- actor: claude-code
  id: 01m076z397p2j64dzqrskssvc7
  text: |-
    ### implement — changed
    - evidence: 5 files — builtin/validators/code-hygiene/rules/missing-docs-swift.md, builtin/validators/code-hygiene/rules/magic-numbers-swift.md, builtin/validators/code-hygiene/rules/missing-docs-dart.md, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/magic_numbers.rs. RED 9/9 failed with the earlier guards; GREEN `cargo nextest run -p swissarmyhammer-validators` 803 passed 0 skipped, `cargo clippy` 0 warnings, `cargo nextest run -p mirdan` 503 passed.
    - next: /review
  timestamp: 2026-08-17T06:36:26.791181+00:00
- actor: claude-code
  id: 01m077x1p466nvv44fd8tgx3wb
  text: |-
    ### review — clean
    - evidence: `review sha HEAD~1..HEAD` (commit 0b6901ee5). 9 rules ran. 0 findings, 0 confirmed, 0 refuted, 0 failed. 2 files reviewed. 6 `.kanban/` files not reviewed, because a `.reviewignore` rule holds them out.
    - next: The card moves to `done`. No finding is open.
  timestamp: 2026-08-17T06:52:48.196644+00:00
- actor: claude-code
  id: 01m077xmp180gfzcq4mvj6bkvw
  text: |
    ### finish iteration 1 — clean
    - implement: changed — 5 files. The three rules state a path they cannot read as a declined item at exit 0, and keep the findings of every file they could judge. Each answers all 3 refusing shapes. Measured: swiftlint writes its own message for 2 shapes and says nothing for the third; dart says nothing for any shape, so the script tests the path itself.
    - test: green — cargo nextest run --workspace, 14145 passed, 0 failed. fmt and clippy clean. The test step proved with `od -c` that each marked line opens at byte 0, drove `dart analyze` directly to confirm it truly writes nothing, and held every rule to exit 0 when EVERY file is unreadable.
    - commit: 0b6901ee5
    - review: clean — 9 rules, 0 findings.
    - note: `function-length-swift` carries the same guard and stands outside this card. It is filed as ^8491bb0.
  timestamp: 2026-08-17T06:53:07.649659+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffffa380
title: Three rules break the batch on a pre-flight readability guard that also misses the non-UTF-8 file
---
`builtin/validators/code-hygiene/rules/missing-docs-swift.md`,
`builtin/validators/code-hygiene/rules/magic-numbers-swift.md` and
`builtin/validators/code-hygiene/rules/missing-docs-dart.md` each open with a
pre-flight loop that walks the argument list and exits 1 for one path it cannot
read, before the tool runs at all. The two swift rules also exit 1 on
swiftlint's own `Could not read contents of` line.

Two defects in one guard:

1. It breaks the WHOLE run for ONE declined path, which
   `builtin/validators/README.md` refuses: "Do not exit nonzero for a declined
   item. A nonzero exit fails the WHOLE run, so one unjudged path throws away
   every finding the run did make."
2. `[ ! -r "$file" ]` cannot answer every refusing shape. Measured while
   implementing `^d3j6sbt` against three staged paths: the test is true for a
   path that holds no file and for a file with no read permission, and FALSE for
   a file whose bytes are not UTF-8 — the mode lets a reader open that one. A
   run gated on the test reads the third file as CLEAN.

`function-length-python` records both verdicts and holds the worked answer: read
what the TOOL itself said, and write it under `sah-diagnostic:` at exit 0.

The work:

- Measure, for each of the three rules, what its tool says for each of the three
  refusing paths — the report, stderr, and exit — and what it reported for the
  OTHER files of the same run.
- Replace the pre-flight guard with a read of the tool's own message, written
  under the marker at exit 0. The marker must OPEN the line.
- Rewrite the acceptance tests that lock the current break.
  `verify_unreadable_file_is_declined` in
  `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs`
  holds the shape, and `ShippedUnreadableFile` already stages all three refusing
  paths.
- State each measurement in each rule body.

Related to `^8nbxwq5`, which covers the SILENT declines of the three swiftlint
rules. This card covers the guards that break the run instead.

Found while implementing `^s8d7fva`. #tool-validators #objectivity