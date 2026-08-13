---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzsyc9ccarwmr2pxmhcgbxbz
  text: |-
    Measured every claim of the card again with swiftlint 0.65.0, before any edit. The card was stale on two of its four claims.

    **Generated code — the card is wrong today.** Card ^xv57pf8 rebuilt the script. It now names the project's own `.swiftlint.yml` as the PARENT of its child configuration and passes `--force-exclude`. Measured over two files that each hold one function of cyclomatic complexity 16, one under `Generated/` and one under `Sources/`, beside `excluded: [Generated]`: the run reports `Sources/Staged.swift` alone. With no project file it reports both. The carve-out IS reproduced.

    **A long SwiftUI `body` — the card is wrong.** `function_body_length` reads no computed variable. Measured over one body of 300 code lines in each shape: `func` reports 300, `init` reports `Initializer body`, `deinit` reports `Deinitializer body`, `subscript` reports `Subscript body`, the `get` accessor of a `subscript` reports `Accessor body`; a computed `var`, the same `var` with an explicit `get`, a `static var` and a closure held in a `let` each report nothing. `cyclomatic_complexity` reads `func` and `init` alone: measured over one body of complexity 16 in each of the same shapes, `deinit`, `subscript`, the subscript `get`, the computed `var`, the `static var` and the closure each report nothing.

    **Tests — the card is right.** Measured over one `Tests/StagedTests.swift`: `func testEndToEnd()` of 300 body lines inside an `XCTestCase` subclass reports `function_body_length` at row 4, and `func buildRequest()` of cyclomatic complexity 16 reports `cyclomatic_complexity` at row 308.

    **Initializers — the card is right.** Measured: an `init` of 260 body lines reports `Initializer body should span 250 lines or less ... currently spans 260 lines`; a builder of 300 `.opt(n)` lines reports 301; a dictionary of 300 entries in a `func` reports 302.

    **The option list.** `swiftlint rules cyclomatic_complexity` names `warning`, `error` and `ignores_case_statements`. `swiftlint rules function_body_length` names `warning` and `error`. No option of either rule reads a declaration name, a superclass, a file header or a data line. So no carve-out but the `switch` one is reachable through the configuration.

    **The annotation.** Measured 21 spellings over one function of complexity 16. These give no finding: the directive on the line directly above; with no space after the `//`; with a reason after the rule name, with a dash and without; naming two rules; `all`; the region form `// swiftlint:disable <rule>`; `:this` on the declaration line; `:previous` on the line UNDER it; a doc line above the directive. These give one finding: a blank line between the directive and the declaration; a doc line between them; capital letters; a block comment; `:previous` on the line ABOVE; a directive that names the other rule, with a reason and without; an unknown rule name; no rule name; `// noqa:`. The directive never expires under this run, because `only_rules` leaves `superfluous_disable_command` out.

    **The silent-zero probes.** A file it cannot read: the `[ ! -r "$file" ]` guard names it and exits 1 — already answered, and a file at mode 000 answers the same. A swiftlint that cannot start: a shim that exits 127 and an empty PATH each give 0 findings at exit 1 — already answered. A file that does not parse: swiftlint states NO parse failure. It parses with recovery and lints what it recovered. Measured over 12 broken head lines above one function of complexity 16, the run reported the function for 11 of them, and the file whose body never closes reported as well. One head — `@@@ this is not swift ((( ]]]` — swallowed the function, and swiftlint then wrote an empty array, 0 bytes of stderr and exit 0. No swiftlint flag states that shape. `swiftc -parse` does, and it also breaks the file whose body never closes, which swiftlint measured correctly, so a `swiftc` gate would trade a true finding for the shape; `swiftc` is also a toolchain `doctor.check_command` does not name. The rule states the gap.

    **One real defect.** swiftlint reads a source file as UTF-8 alone. Measured over one file that holds `let name = "café"` in Latin-1, above one function of complexity 16: swiftlint writes ``Could not read contents of `<path>` `` to stderr, writes an empty array (5 bytes) to stdout and exits 0. Beside one file that holds a finding it writes the same stderr line, writes 1 entry, and exits 2. Both rows read as a measured run, so the file swiftlint never read reached the engine as a clean tree.
  timestamp: 2026-08-12T02:56:14.220703+00:00
- actor: claude-code
  id: 01kzsycrh2kbakkm3xnhwvbtw5
  text: |-
    ### implement — changed

    - evidence:
      - `builtin/validators/code-hygiene/rules/complexity-swift.md` — the script now tests stderr for `Could not read contents of` after it forwards swiftlint's own message, writes `complexity-swift: swiftlint could not read the contents of a file this run names`, and exits 1. RED to GREEN with the real swiftlint: before the test, the Latin-1 file gave 0 findings, 0 tool errors and exit 0; after it, 0 findings and 1 tool error. Four new sections state the measurements: "What each gate reaches, and what neither reaches", "A file swiftlint cannot decode", "A file that does not parse", "The annotation an author writes" and "The carve-outs the two prompt rules state".
      - `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/complexity_swift.rs` — 5 new acceptance tests that drive the shipped bytes: `..._breaks_on_a_file_it_cannot_decode`, `..._reports_a_test_method_and_its_helper`, `..._answers_the_length_gate_annotation`, `..._answers_the_complexity_gate_annotation`, `..._reads_no_computed_property_body`.
      - `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs` — `stage_probe_bytes` splits out of `stage_probe_files`, and `ShippedNamedPath.source` takes bytes rather than text, because a probe of a file the tool cannot DECODE stages bytes that are not UTF-8 and no `&str` holds those.
      - `.../shipped/complexity_go.rs`, `complexity_python.rs`, `complexity_rust.rs`, `magic_numbers.rs`, `missing_docs.rs` — 9 `source: Some(X)` entries take `X.as_bytes()`. No behaviour of those rules moves.
    - tests: `cargo nextest run -p swissarmyhammer-validators` — 670 run, 670 passed. `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` — 3433 run, 3429 passed, 4 failed; the 4 are the pre-existing failures of this tree (three `review_e2e` tests missing the duplication claim, and `review_progress_is_received_by_a_real_client_over_a_byte_stream_transport` timing out). `cargo fmt --all --check` clean. `cargo clippy -p swissarmyhammer-validators --all-targets -- -D warnings` clean.
    - new cards: `^3gfsk7k` — `magic-numbers-swift` and `missing-docs-swift` hold the same undecodable-file silent zero. `^0fqsxwa` — `function-length` measures a closure and this rule does not; measure `closure_body_length` and decide.
    - next: `/review`
  timestamp: 2026-08-12T02:56:29.730638+00:00
- actor: claude-code
  id: 01kzsz85m592qrayw22cf5ppp6
  text: |
    ### review — findings

    - scope: `review sha HEAD~1..HEAD` (commit `d797a8630`), 24 validators attempted, 0 failed, 0 skipped.
    - engine: 1 finding, 1 confirmed, 0 refuted. That finding is DROPPED. It asks to make `copy_shipped_fixture` call the new `stage_probe_bytes` at `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:210`. `copy_shipped_fixture` is test code that was there before this commit, and `git diff --unified=0` puts line 210 outside each hunk of the commit. The review skill drops a finding that asks to refactor a test that already existed.
    - open findings: 2, each measured here with swiftlint 0.65.0, each on `builtin/validators/code-hygiene/rules/complexity-swift.md`.
      - line 56 — the new `grep -qF 'Could not read contents of'` breaks a run that measured correctly, because swiftlint writes the file path into stderr with `Error: No lintable files found at paths: '<path>'`. A file with the name `Could not read contents of.swift`, beside a project `excluded:` list that covers it, exits 1 with the tool-error line. The same run over `Plain.swift` exits 0 with no finding.
      - line 51 — the earlier `grep -qF 'Could not read configuration'` holds the same cause and makes a WRONG FINDING. A file with the name `Could not read configuration.swift`, beside the same exclude list, makes the script drop the project configuration and report 1 finding on a file the project excludes.
    - measurements that CONFIRM the claims of the delta:
      - the undecodable file, both rows: the Latin-1 file alone and the Latin-1 file beside a healthy one each exit 1, and swiftlint's own message names the path above the rule's line. The healthy file alone reports 1 finding and exits 0.
      - the declaration table, all 18 cells: `function_body_length` reports for `func`, `init`, `deinit`, `subscript` and a subscript `get`, and is silent for a computed `var`, a `var` with an explicit `get`, a `static var` and a closure. `cyclomatic_complexity` reports for `func` and `init` alone. The messages read `Function body`, `Initializer body`, `Deinitializer body`, `Subscript body` and `Accessor body`.
      - the generated-code carve-out, both ways: 1 finding beside `excluded: [Generated]`, and 2 findings with no project file.
      - the stderr test makes no wrong break for: a run with a finding beside a `// swiftlint:disable:next` comment and a clean file (exit 0); a project configuration that writes `warning: The key(s) 'whitelist_rules' ... is/are invalid.` to stderr (exit 0); a file with the name `Could not read contents of.swift` on a healthy run (exit 0).
      - the parse table, all 8 rows: 7 heads report the function, and `@@@ this is not swift ((( ]]]` reports nothing with an empty array and exit 0.
      - the nine `.as_bytes()` call sites: `complexity_go` 1, `complexity_python` 1, `complexity_rust` 1, `complexity_swift` 1, `magic_numbers` 1, `missing_docs` 4. Each is one line, each keeps its constant, and no staged content moves.
    - tests: `touch crates/swissarmyhammer-validators/src/builtin/mod.rs`, then `cargo nextest run -p swissarmyhammer-validators complexity_swift` — 13 run, 13 passed.
    - next: correct both stderr tests, then `/review ^h2ezbs7` again.
  timestamp: 2026-08-12T03:11:27.877101+00:00
- actor: claude-code
  id: 01kzsz9898kdjr58tb4b4k0s7c
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 8 files; a Swift file swiftlint cannot decode no longer reaches the engine as clean, and two stale claims of the card are corrected with measurement
    - test: green — cargo nextest -p swissarmyhammer-validators 670 run, 670 passed; rdeps 3433 run, 3429 passed with only the 4 failures that came before
    - commit: d797a8630
    - review: findings — builtin/validators/code-hygiene/rules/complexity-swift.md:56 and :51. Every other claim measured true: both undecodable rows break, all 18 cells of the declaration table match, the carve-out gives 1 finding with the project exclude list and 2 without. Two greps read all of stderr, so a file whose NAME holds the words breaks a run that measured correctly. The answer is named: anchor each grep to the start of a line.
  timestamp: 2026-08-12T03:12:03.368516+00:00
- actor: claude-code
  id: 01kzt05wscngbx9a34czb7qzc3
  text: |-
    Both findings hold one cause: a `grep` that reads ALL of stderr, where swiftlint writes the PATH of a file. The whole script holds THREE tests on stderr, and each one is now anchored on the start of a line, where swiftlint writes its own message:

    - `grep -qE '^Could not read configuration:'`
    - ``grep -qE '^Could not read contents of ` '``
    - `grep -qE '^Error: No lintable files found at paths:'`

    The exact lines swiftlint 0.65.0 writes, each measured:

    - `Could not read configuration: file Configuration.swift, line 278` — for `child_config: other.yml` and for bytes that are not YAML.
    - ``Could not read contents of `<path>` `` — for the Latin-1 file.
    - `Error: No lintable files found at paths: '<path>'` — for an excluded run, a hollow directory, an absent path and a `.txt` file.

    TDD, RED to GREEN, on the shipped bytes. Two new acceptance tests, each staged under `Generated/` beside `excluded: [Generated]`, each expecting no finding and no tool error:

    - `..._measures_a_file_named_for_the_decode_message` — RED: one tool error, `Error: No lintable files found at paths: 'Generated/Could not read contents of.swift'` and the rule's own decode line, exit 1. GREEN: no finding, no error.
    - `..._measures_a_file_named_for_the_configuration_message` — RED: 1 finding, `Generated/Could not read configuration.swift`, on a file the project excludes. GREEN: no finding.

    Both directions, measured by driving the SHIPPED `run:` block over 14 probe trees. The anchored script and the unanchored script were each run over the same 14 trees.

    Still fire, both scripts alike: the Latin-1 file alone (0 findings, exit 1, the decode line); the Latin-1 file beside one healthy file (0 findings, exit 1, the decode line); a project `child_config: other.yml` (1 finding, exit 0, the configuration line); a project of bytes that are not YAML (1 finding, exit 0, the configuration line); one file under `Generated/` beside `excluded: [Generated]` (0 findings, exit 0, no rule line).

    Fire for neither script: one healthy file with a finding (1, exit 0); a `// swiftlint:disable:next` comment (0, exit 0); a project that writes `warning: The key(s) 'whitelist_rules' used as rule identifier(s) is/are invalid.` (1 finding, exit 0); a hollow directory (0, exit 0); a file the decode words name on a healthy run (1, exit 0); a file the configuration words name on a healthy run (1, exit 0).

    The two scripts differ at 2 of the 14 trees, and those 2 are the findings:

    | the file name, under an excluded directory | unanchored | anchored |
    |---|---|---|
    | `Could not read contents of.swift` | 0 findings, exit 1, the tool-error line | 0 findings, exit 0 |
    | `Could not read configuration.swift` | 1 finding on an excluded file | 0 findings, exit 0 |

    The third grep, `No lintable files found`, moved no row: a file named `No lintable files found.swift` under an excluded directory answers 0 findings and exit 0 with each script. The decode test stands above it, and each other broken shape measured here — a version mismatch and a configuration abort — writes no path into stderr. The anchor holds that test on swiftlint's own message anyway, which is what removes the cause from the WHOLE script.

    The two sibling rules carry the same defect, measured over one file under `Generated/` beside `excluded: [Generated]`: `magic-numbers-swift` and `missing-docs-swift` each report 0 findings for `Plain.swift` and 1 finding for `Could not read configuration.swift`, and each writes its own "cannot read .swiftlint.yml" line for the second. That is recorded on ^3gfsk7k rather than corrected here.
  timestamp: 2026-08-12T03:27:41.868245+00:00
- actor: claude-code
  id: 01kzt06aesx51jtzc284yamaxx
  text: |-
    ### implement — changed

    - evidence: 2 files.
      - `builtin/validators/code-hygiene/rules/complexity-swift.md` — each of the 3 stderr greps of the script is anchored on the start of a line. New section "Each stderr test reads swiftlint's own message, and not a file name" states the pattern of each test, the line swiftlint writes for it, the 4-row file-name table (unanchored beside anchored) and both directions over 11 runs. The sections "What the script accepts at status 2", "A project configuration swiftlint cannot read beside this rule" and "A file swiftlint cannot decode" each name the anchored form, and the decode section states the measurement the finding asked for.
      - `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/complexity_swift.rs` — 2 new acceptance tests on the shipped bytes: `the_shipped_swift_complexity_tool_rule_measures_a_file_named_for_the_decode_message` and `..._for_the_configuration_message`. RED before the anchor: the first broke with one tool error at exit 1; the second reported 1 finding on a file the project excludes. GREEN after it: each reports no finding and no tool error.
    - tests: `touch crates/swissarmyhammer-validators/src/builtin/mod.rs`, then `cargo nextest run -p swissarmyhammer-validators` — 672 run, 672 passed, 0 skipped. `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` — 3435 run, 3431 passed, 4 failed; the 4 are the failures this tree carried before (3 `review_e2e` and `review_progress_is_received_by_a_real_client_over_a_byte_stream_transport`). `cargo fmt --all --check` clean. `cargo clippy -p swissarmyhammer-validators --all-targets -- -D warnings` clean.
    - cards: ^3gfsk7k now carries the anchoring defect of the two sibling rules, with the measurement.
    - next: `/review`
  timestamp: 2026-08-12T03:27:55.865347+00:00
- actor: claude-code
  id: 01kzt0w8e1gf1cch35mbhj52tf
  text: |
    ### review — clean

    - scope: `review sha HEAD~1..HEAD` (commit `a8a80ba1a`), 8 validators attempted, 0 failed, 0 skipped. Engine: 0 findings, 0 confirmed, 0 refuted.
    - prior findings: both items of the section "Review Findings (2026-08-11 22:10)" are checked, and each is measured closed here.

    **The two round-1 findings, measured again with a control.** The shipped `run:` block and the same block with the three unanchored greps were each run over the same trees, with swiftlint 0.65.0.

    | the file name, under `Generated/` beside `excluded: [Generated]` | unanchored | anchored |
    |---|---|---|
    | `Staged.swift` | 0 findings, exit 0 | 0 findings, exit 0 |
    | `Could not read contents of.swift` | 0 findings, exit 1, the tool-error line | 0 findings, exit 0 |
    | `Could not read configuration.swift` | 1 finding on an excluded file | 0 findings, exit 0 |
    | `No lintable files found.swift` | 0 findings, exit 0 | 0 findings, exit 0 |
    | `Error.swift` | 0 findings, exit 0 | 0 findings, exit 0 |

    **Each test still fires for a real failure.** The Latin-1 file alone: 0 findings, exit 1, the decode line. The Latin-1 file beside one healthy file: the same. A project `child_config: other.yml`: 1 finding, exit 0, the configuration line. A project of bytes that are not YAML: the same. A project with a bad `child_config` URL: the same.

    **The anchors are not too narrow.** Each attack shape was measured:
    - The message on line 2 or later of stderr. A `whitelist_rules` warning above the decode line: the test fires, exit 1. A `whitelist_rules` warning above `Error: No lintable files found at paths:` over the directory `Sources/Hollow.swift`: exit 0, no finding. A YAML parse error and a bad-URL error above `Could not read configuration:`: the test fires. Each message starts a line.
    - Interleaved lines. 40 Latin-1 files in one run wrote 40 stderr lines. Each line starts with the decode message. 0 lines start with anything else.
    - The configuration abort after the retry. A project `child_config: other.yml` beside a Latin-1 file: the configuration test fires, the retry writes the decode line, the decode test fires, exit 1.
    - A long line. A path of 8 directory names and a long file name, and 30 paths in one message, each wrote `Error: No lintable files found at paths:` on ONE line. swiftlint wraps no line.
    - No stderr of any shape held a carriage return. `od -c` over each stderr shows `\n` alone.

    **The byte after each anchor.** `strings` over `/opt/homebrew/bin/swiftlint` holds one literal for each message: ``Could not read contents of ` `` (the backtick is part of the literal), `Could not read configuration` and `No lintable files found at paths: '`. The colon after `Could not read configuration` comes from the `queuedFatalError` format `"<message>: file <file>, line <line>"`, so the colon is always there. Measured over three abort shapes, each wrote `Could not read configuration: file Configuration.swift, line 278`. Four decode shapes — Latin-1, UTF-16, bytes that are not text, and one bad byte in the middle — each wrote ``Could not read contents of `<path>` ``. A UTF-8 BOM file decodes and reports 1 finding.

    **No other spelling of the messages is reachable.** The alternative `No lintable files found in current working directory` needs a run with no path. The script exits 0 when it gets no file, so it always names a path.

    **The two new acceptance tests.** RED on the old bytes, measured with the unanchored script: `Could not read contents of.swift` broke at exit 1 with the tool-error line, and `Could not read configuration.swift` reported 1 finding on an excluded file. GREEN on the shipped bytes: `touch crates/swissarmyhammer-validators/src/builtin/mod.rs`, then `cargo nextest run -p swissarmyhammer-validators complexity_swift` — 15 run, 15 passed, 0 failed.

    **The diff.** `git diff --unified=0 HEAD~1..HEAD` shows 3 code hunks in the rule (lines 51, 56, 69 — the three greps), 7 prose hunks in the rule body, and 1 hunk in the test file (`@@ -402,0 +403,96 @@`, an addition alone). No assertion that was there before moves.

    - next: none. The card moves to `done`.
  timestamp: 2026-08-12T03:39:54.689221+00:00
- actor: claude-code
  id: 01kzt0xav16jts3a2cf28xng3z
  text: |-
    ### finish iteration 2 — clean
    - implement: changed — 2 files; each of the three stderr tests is anchored on the start of a line, which removes the cause from the whole script
    - test: green — cargo nextest -p swissarmyhammer-validators 672 run, 672 passed; rdeps 3435 run, 3431 passed with only the 4 failures that came before
    - commit: a8a80ba1a
    - review: clean — 8 validators attempted, 0 findings; both prior findings closed with a control over 5 file-name trees, 5 real-failure trees and 6 no-fire trees; the anchors let no real failure through, and the byte after each anchor stands in the swiftlint binary; the card moves to done
  timestamp: 2026-08-12T03:40:29.921379+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffe980
title: complexity-swift supersedes both gates but drops the test, generated and init carve-outs
---
`builtin/validators/code-hygiene/rules/complexity-swift.md` writes its own child `swiftlint.yml` with `only_rules: [cyclomatic_complexity, function_body_length]` and declares `supersedes: [cognitive-complexity, function-length]`.

## What the card stated, and what the measurement says

The card was written on 2026-08-10. Card `^xv57pf8` then rebuilt the three Swift rules. Each claim was measured again with swiftlint 0.65.0.

- **Generated code — the card is WRONG today.** The card states the rule "never reads the project's own `.swiftlint.yml`", so the project `excluded:` list is discarded. The script now names that file as the PARENT of its own configuration and passes `--force-exclude`. Measured over two files that each hold one function of cyclomatic complexity 16, one under `Generated/` and one under `Sources/`, beside `excluded: [Generated]`: the run reports the `Sources/` file alone. With no project file it reports both. The carve-out IS reproduced.
- **Tests — the card is RIGHT.** Measured: a `func testEndToEnd()` of 300 body lines in an `XCTestCase` subclass reports `function_body_length`, and a `func buildRequest()` of cyclomatic complexity 16 beside it reports `cyclomatic_complexity`.
- **Initializers — the card is RIGHT.** Measured: an `init` of 260 body lines reports `Initializer body should span 250 lines or less`. A builder of 300 `.opt(n)` lines and a dictionary of 300 entries each report too.
- **A long SwiftUI `body` — the card is WRONG.** A SwiftUI `body` is a computed variable, and `function_body_length` reads no computed variable. Measured over one body of 300 lines in each shape: `func`, `init`, `deinit`, `subscript` and a subscript `get` each report; a computed `var`, a `var` with an explicit `get`, a `static var` and a closure each report nothing.

## The silent-zero defect found here

swiftlint reads a source file as UTF-8 alone. Measured over one Swift file written in Latin-1: swiftlint writes ``Could not read contents of `<path>` `` to stderr, writes an empty JSON array to stdout, and exits 0. Beside one file that holds a finding it writes the same stderr line, writes that report, and exits 2. The status and the report each read as a measured run, so the file swiftlint never read reached the engine as a clean tree.

## What this card did

- [x] Correct each stale claim of the card with the measurement.
- [x] Add the stderr test for `Could not read contents of` to the script, so a file swiftlint could not decode breaks the run.
- [x] State each carve-out of BOTH superseded prompt rules on the rule, with what answers it: the run, the author's annotation, or nothing.
- [x] State each spelling of `// swiftlint:disable` that works and each that does not, with the place each must stand.
- [x] State what each gate reaches, and what neither gate reaches.
- [x] Hold each behaviour with an acceptance test that drives the shipped bytes.

Found by the `supersedes` survey on ^h7garpc. #tool-validators #objectivity

## Review Findings (2026-08-11 22:10)

- [x] `builtin/validators/code-hygiene/rules/complexity-swift.md:56` — The new stderr test breaks a run that measured correctly. `grep -qF 'Could not read contents of'` reads all of stderr, and swiftlint writes the file path into stderr when it finds no file to lint. Measured with swiftlint 0.65.0 over one file with the name `Could not read contents of.swift` under `Sources/`, beside a project `.swiftlint.yml` that states `excluded: [Sources]`: swiftlint writes `Error: No lintable files found at paths: 'Sources/Could not read contents of.swift'` to stderr and exits 1, the new test matches the path, and the script writes `complexity-swift: swiftlint could not read the contents of a file this run names` and exits 1. The same run over `Sources/Plain.swift`, with the same exclude list, reports no finding and exits 0. The engine therefore reads a broken tool for a run that measured correctly, and the file name alone makes the difference. swiftlint writes its own decode message at the start of a line, and it writes the path echo after `Error: `. Measured: `grep -qE '^Could not read contents of `'` matches the decode message and does not match the path echo. Anchor the test on the start of the line, and state the measurement in the section "A file swiftlint cannot decode".

- [x] `builtin/validators/code-hygiene/rules/complexity-swift.md:51` — The same cause makes a WRONG FINDING at the earlier stderr test, so the correction must remove the cause from the whole script and not from the new test alone. `grep -qF 'Could not read configuration'` also reads all of stderr. Measured with swiftlint 0.65.0 over one file with the name `Could not read configuration.swift` under `Sources/`, beside a project `.swiftlint.yml` that states `excluded: [Sources]`: the script writes `complexity-swift: swiftlint cannot read .swiftlint.yml beside this rule. The run drops the project exclude list.`, runs swiftlint a second time without the project configuration, and reports 1 finding on a file the project excludes. The rule states that a wrong finding is a requirement to change correct code. Anchor this test on swiftlint's own message in the same way, and hold both shapes with an acceptance test.
