---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m1ydxkqe4nhzfynnk044mhss
  text: |-
    Research done. Measurements made on this machine, Apple Swift 6.4.

    New facts the card's table did not hold, each measured:

    1. A `--configuration` whose `rules` table names only some rules turns EVERY
       unnamed rule OFF. `AlwaysUseLowerCamelCase` is ON by default and reports
       `let Bad_One = 1`; under a `rules` table naming only
       `AlwaysUseLiteralForEmptyCollectionInit` it is silent. So one small JSON
       file states the whole rule set, and the other configuration keys
       (`lineLength`, `indentation`) keep their defaults.
    2. The nine unstoppable tags still come through that configuration:
       `[Indentation]` and `[LineLength]` each reported under it. The tag filter
       stays the gate.
    3. A DIRECTORY in a shared work list costs the WHOLE run: `swift format lint
       --strict Dirty.swift Adir.swift` exits 64, writes ZERO findings, and
       `Dirty.swift` is judged in none of it. That is the measured reason to keep
       one run for each file, and it is stronger than the reason the body records.
    4. The run's `--configuration` wins over a project `.swift-format`: a project
       file holding `"AlwaysUseLiteralForEmptyCollectionInit": false` did not stop
       the finding.
    5. `AlwaysUseLiteralForEmptyCollectionInit` reports `[Int]()` and
       `[String: Int]()`. It stays SILENT for `Set<String>()`.
    6. `NoVoidReturnOnFunctionSignature` reports `-> ()` AND `-> Void`, and stays
       silent for a closure parameter `(Int) -> ()`. That last shape belongs to
       `ReturnVoidInsteadOfEmptyTuple`, which the allowlist does not name.
    7. `printf '' | swift format lint --strict -` exits 0, so it is a doctor probe.
       `swift format --version` writes `main` and names no release, so the floor is
       read off the SUBCOMMAND standing at all.

    Blast radius the card did not list, found by reading the callers:

    - `tests/shipped/swift_guidelines_partial.rs` reads the shipped `idioms-swift`
      script for `swiftformat --lint` and five options. Those options go away with
      this move, so that half of
      `swift_partial_agrees_with_the_shipped_swift_tool_validators` loses its
      source and the test would fail. Task ^ywgq61n owns the partial rewrite; this
      task makes the minimum edit that keeps the test honest and green.
    - `tests/shipped/swift_judgment_rules.rs` uses `SWIFT_VERSION_SUPPORT`, which
      this task deletes.
    - `idioms.md` still states the empty-collection bullet and the omit-the-clause
      bullet, and the gate now decides both. That two-owner state is temporary and
      task ^2hd90th owns it, so no prompt bullet moves here.
    - `preferFinalClasses` has no successor rule in the toolchain, so the
      `value-semantics.md` `final` bullet has no owner until ^2hd90th returns it.
      Its row leaves the superseded-bullet table here.
  timestamp: 2026-09-07T17:16:11.886218+00:00
- actor: claude-code
  id: 01m1yfx1047wrp2taeh1c17ge8
  text: |-
    Implementation landed. What the gate is now:

    The script writes ONE list of seven rule tags, and that list does two jobs: a
    `sed` turns it into the `{"rules": {...}}` configuration the run passes with
    `--configuration`, and a `paste` turns it into the alternation the output
    filter reads. One list, so the two cannot drift.

    Per file: `[ ! -e "$file" ]` first, then
    `swift format lint --strict --configuration ... "$file" > /dev/null 2>err`
    with `|| status=$?`. A stderr line holding `: error: [` is a tagged finding;
    every other line is trouble, and a file that wrote one is declined whole with
    a `sah-diagnostic:` line. The findings pass through
    `grep -E ": error: \[($allowed)\] "` and a `sed` that writes
    `path:line: RuleName: reason`.

    TDD evidence, both directions measured:

    - RED, whole set: with the rule file and the two fixtures stashed back to the
      swiftformat version, `cargo nextest run -p swissarmyhammer-validators
      idioms_swift` answered 6 passed, 9 FAILED.
    - RED, the filter alone: with the allowlist alternation replaced by
      `[A-Za-z]*` in the shipped script, the same run answered 11 passed, 4
      FAILED, and
      `the_shipped_swift_idioms_tool_rule_drops_every_tag_outside_its_allowlist`
      failed with its own message — "the `[AddLines]` probe holds a layout defect
      and no idiom ... the run reported [\"AddLines\", \"AddLines\"]".
    - GREEN: 15 of 15 pass, and `cargo nextest run -p swissarmyhammer-validators
      swift` answers 86 of 86.

    Measured numbers now written into the body and held by a test:

    - The failing fixture: 11 findings, exit 0, carrying all 7 allowlist rules.
    - The passing fixture: 0 findings through the filter, while the raw
      `swift format lint --strict` over the same bytes writes 34 `[Indentation]`
      lines. The doctor's own fixture pair therefore measures the filter.
    - Each of the nine unstoppable tags: 0 findings through the shipped script.

    Two things this task did NOT do, both by the card's own order:

    - No prompt bullet moved. `idioms.md` still states the empty-collection bullet
      and the omit-the-clause half of the `Void` bullet, and the gate now decides
      both. The body names that two-owner state and names the task that ends it.
    - `UseWhereClausesInForLoops` stays OFF, and
      `the_shipped_swift_idioms_tool_rule_leaves_the_where_clause_rule_off` holds
      the shipped script to naming it NOWHERE — a configuration key alone would
      turn it on with no allowlist entry.

    `preferFinalClasses` left the superseded-bullet table, because the toolchain
    carries no rule like it. The `value-semantics.md` `final` bullet therefore has
    no owner until task ^2hd90th states it again. The body says so in as many
    words.
  timestamp: 2026-09-07T17:50:49.860398+00:00
- actor: claude-code
  id: 01m1yfxe5z161nce1fya740dmd
  text: |-
    One test item of the card is answered in part, and the gap is pre-existing.

    "Run `sah doctor`. The rule reports as healthy, and its fixture pair passes."

    The fixture pair passes. `every_shipped_idioms_tool_rule_passes_its_fixtures`
    drives the doctor's OWN `check_tool_rule` path over the shipped pair, and it
    passes.

    `sah doctor` writes no ROW for this rule. Measured over a probe Swift package,
    the table names `dead-code-swift`, `function-length-swift`,
    `magic-numbers-swift` and `missing-docs-swift`, and neither `idioms-swift` nor
    `disallowed-constructs-swift`. The same probe answers the same 4 rows with the
    working tree stashed back to commit `4fed2803d`, so the defect predates this
    change and is not its work. Task ^15vxtng records it, with what is already
    known and the hypothesis to measure first.

    Two more notes for the tasks that follow.

    - ^ywgq61n owns the partial. This task made ONE minimum edit to
      `swift_guidelines_partial.rs` to keep it honest: the half of
      `swift_partial_agrees_with_the_shipped_swift_tool_validators` that read five
      swiftformat options off the shipped script had no source left, so it now
      reads the LINT COMMAND HEAD off the shipped script and holds the partial to
      naming it. The partial already writes `swift format lint`, so no partial
      edit was needed. The empty-collection half is untouched.
    - ^37yt9xy owns the document sweep.
      `crates/swissarmyhammer-validators/src/review/tool_rules/tests.rs` still
      carries a doc comment saying this script intersects its roster with
      `swiftformat --rules`. That sentence is now false, and the card of ^37yt9xy
      lists that file, so it was left there.
  timestamp: 2026-09-07T17:51:03.359919+00:00
- actor: claude-code
  id: 01m1yfxq7etkx42z8m4bs6kvp0
  text: |-
    ### implement — changed
    - evidence: 8 files — builtin/validators/code-hygiene/rules/idioms-swift.md, builtin/validators/code-hygiene/fixtures/idioms-swift.fail.swift.tmpl, builtin/validators/code-hygiene/fixtures/idioms-swift.pass.swift.tmpl, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/idioms_swift.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/swift_judgment_rules.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/swift_guidelines_partial.rs, crates/swissarmyhammer-validators/src/builtin/mod.rs. `cargo nextest run -p swissarmyhammer-validators swift` — 86 run, 86 passed. `cargo nextest run -p swissarmyhammer-validators` — 854 run, 816 passed, 38 failed; the same 38 names fail at HEAD with the change stashed, so every one is pre-existing (missing go, revive, staticcheck, golangci-lint and eslint-plugin-sonarjs). `cargo clippy -p swissarmyhammer-validators --tests` — 0 warnings. `cargo fmt --check` — clean.
    - next: /review
  timestamp: 2026-09-07T17:51:12.622142+00:00
- actor: claude-code
  id: 01m1ygnfqt7bs6jgh3m5ab7qac
  text: |-
    ### review — findings
    - evidence: `review sha HEAD~1..HEAD` (checkpoint 7f5e52261) gave 0 findings over 5 Rust files. The engine matched no validator to `builtin/validators/code-hygiene/rules/idioms-swift.md`, which carries the gate script, so that file had no cover. Direct verification of the script found 3 items: idioms-swift.md:46, idioms-swift.md:40, and tests/shipped/idioms_swift.rs.
    - The cause is one: both greps match `: error: [` as text anywhere in the line, and `swift format` writes the file path at the head of each line. A path that holds that text carries a tag outside the allowlist through the gate, and it also stops the decline guard.
    - Measurements that hold as stated: fail fixture 11 findings over 7 rules; pass fixture 0 findings; 34 raw `[Indentation]` lines and 0 through the filter; two reporting files give the findings of both at exit 0; the run rewrites no file; `UseWhereClausesInForLoops` is OFF. `cargo nextest run -p swissarmyhammer-validators swift` — 86 passed.
    - next: anchor both greps to the true `path:line:column:` head, then add the probe named for a diagnostic message that `missing_docs.rs` already sets as the pattern.
  timestamp: 2026-09-07T18:04:11.386194+00:00
- actor: claude-code
  id: 01m1ygpdkxwwhrh77w4227chc1
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 8 files. The gate now runs `swift format lint`. One list of seven rule tags writes the configuration JSON and the output filter, thus the two cannot become different.
    - test: green — cargo nextest -p swissarmyhammer-validators swift, 86 run, 86 passed; cargo fmt --all --check clean; cargo clippy -p swissarmyhammer-validators --all-targets -D warnings clean. The driver ran these.
    - commit: 7f5e52261
    - review: findings — builtin/validators/code-hygiene/rules/idioms-swift.md:40, builtin/validators/code-hygiene/rules/idioms-swift.md:46, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/idioms_swift.rs
    - The defect is one cause at two sites. Both `grep` patterns read `: error: [` at ANY position of the line, and `swift format` writes the path at the head of each line. Thus a file with the name `x: error: [UseShorthandTypeNames] y.swift` puts a tag through the filter that the allowlist does not name, and it stops the decline of a file the parser cannot read. The fix is to anchor both patterns to the true `path:line:column:` head.
    - Review confirmed every other measurement: 11 findings from the failing fixture carrying all 7 rules; 0 findings from the passing fixture through the filter; 34 raw `[Indentation]` lines from the same bytes; the file checksum does not change, thus the gate is read-only; `UseWhereClausesInForLoops` is absent from the written configuration.
  timestamp: 2026-09-07T18:04:41.981247+00:00
- actor: claude-code
  id: 01m1yhpnng83k1gedgtfakbs4q
  text: |
    All three review findings are answered. One cause, removed from the whole
    script.

    The cause: `swift format` writes the PATH FIRST on every line. Both readings of
    that output matched their text at ANY position, so a file NAME could give the
    pattern the reading looks for.

    The fix, at both sites: the script now builds the head of a real diagnostic
    before it reads anything.

        quoted="$(printf '%s' "$file" | sed -e 's/[][\\.^$*+?(){}|]/\\&/g')"
        diagnostic="^$quoted:[0-9]+:[0-9]+: error: "

    The `sed` puts a backslash in front of every pattern operator, so the path
    stands in the pattern as TEXT. The decline guard reads
    `grep -v -E "$diagnostic\[[A-Za-z]+\] "`, the tag filter reads
    `grep -E "$diagnostic\[($allowed)\] "`, and the rewrite is a `sed -E` anchored
    on the same head, with the path recaptured as group 1 rather than written back
    as text. The tool makes that head for a finding and for nothing else.

    TDD, both directions measured.

    - RED: the two new tests were written first and run against the shipped
      script. `..._measures_a_file_named_for_a_diagnostic_head` failed with "the
      `[Indentation]` probe holds a layout defect and no idiom ... the run reported
      [\"Indentation\"]". `..._declines_a_file_named_for_a_diagnostic_head` failed
      with three findings, each a raw parser line, and no marked line. 2 run, 0
      passed, 2 failed.
    - GREEN: the same two tests, 2 run, 2 passed.
    - `cargo nextest run -p swissarmyhammer-validators swift` — 88 run, 88 passed.
      86 before, plus the two new ones.

    Everything the review verified still holds, measured again with the shipped
    script over the same bytes:

    - the failing fixture: 11 findings, exit 0, carrying all 7 allowlist rules;
    - the passing fixture: 0 findings through the filter, beside 34 raw
      `[Indentation]` lines from the same bytes;
    - two reporting files in one run give the findings of both, at exit 0, beside
      one `sah-diagnostic:` line for a missing path;
    - the gate is read-only: `shasum` over both judged files is the same before and
      after the run;
    - `UseWhereClausesInForLoops` stands nowhere in the shipped script.

    Row 5 of the new body table is the half a reader could miss. An anchor tight
    enough to drop the crafted layout line could take a TRUE finding away as well,
    so `w: error: [UseShorthandTypeNames] s.swift`, holding one `Array<Int>`
    parameter, is measured to still report its `UseShorthandTypeNames` finding. The
    second half of `..._measures_a_file_named_for_a_diagnostic_head` holds it.

    Notes for the next agent:

    - The body carried the sentence that made the defect — "a line holding
      `: error: [` is a tagged finding". That sentence is now corrected, and the new
      section "Each reading is anchored on the head of a real diagnostic" states the
      measurement in five rows, with the loose script beside the shipped one.
    - `SwiftIdiomsFixtureRun` is renamed `SwiftIdiomsRun`, because a second helper
      `swift_idioms_staged_run` now answers the same shape over a probe file rather
      than over the fixture.
    - The `[Indentation]` probe and its tag are now named constants, so the
      tag-probe list and the new test read ONE file rather than two copies.
  timestamp: 2026-09-07T18:22:18.800167+00:00
- actor: claude-code
  id: 01m1yhpxfsjachmh5qtht0sttt
  text: |
    ### implement — changed
    - evidence: 2 files — builtin/validators/code-hygiene/rules/idioms-swift.md, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/idioms_swift.rs. All 3 review findings flipped to `- [x]`. `cargo nextest run -p swissarmyhammer-validators swift` — 88 run, 88 passed (86 before, plus 2 new). `cargo nextest run -p swissarmyhammer-validators` — 856 run, 818 passed, 38 failed; the failing name set is IDENTICAL with the change stashed, so every one is pre-existing (missing go, revive, staticcheck, golangci-lint and eslint-plugin-sonarjs). `cargo fmt --all --check` clean. `cargo clippy -p swissarmyhammer-validators --all-targets -D warnings` clean.
    - next: /review
  timestamp: 2026-09-07T18:22:26.809491+00:00
- actor: claude-code
  id: 01m1yhrzj4tn0tjdmhx6gr5h0f
  text: |-
    ### finish iteration 2 — pending review
    - implement: changed — 2 files. All 3 findings closed. One cause removed from the whole script: each reading is now anchored on `^<path>:<line>:<column>: error: `, and the file name is escaped for the regular expression first.
    - test: green — cargo nextest -p swissarmyhammer-validators swift, 88 run, 88 passed (86 before, plus 2 new); cargo fmt --all --check clean. The driver ran these.
    - commit: 3d79443c9
    - review: pending
    - The two new tests came first. Both failed before the fix: one reported `["Indentation"]`, and the other gave three raw parser lines as findings with no marked line.
  timestamp: 2026-09-07T18:23:34.468853+00:00
- actor: claude-code
  id: 01m1yjf5bm5a4b03841cey53fg
  text: |-
    ### review — findings
    - evidence: `review sha HEAD~1..HEAD` (checkpoint 3d79443c9), round 2. Engine: 1 finding, 1 confirmed, 0 refuted; it again matched no validator to `builtin/validators/code-hygiene/rules/idioms-swift.md`. 3 more findings come from direct measurement of that uncovered gate script. Open at `builtin/validators/code-hygiene/rules/idioms-swift.md:41`, `builtin/validators/code-hygiene/rules/idioms-swift.md:49`, `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/idioms_swift.rs:281`, and `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/idioms_swift.rs` (anchor coverage).
    - The three findings of round 1 are genuinely fixed. Both new tests are RED against the old loose script and GREEN against the shipped one.
    - Two NEW defects in the same anchoring code: a path with a precomposed character (`café.swift`) is declined whole, because the tool writes the path in NFD and the argv spelling is NFC; and a path holding `|` leaves the line unrewritten, because `|` is the s-command delimiter of the `sed -E`. The escape set itself is complete, and the group-1 recapture does keep `&` out of the replacement.
    - Prior invariants hold: failing fixture 11 findings / 7 rules at exit 0; passing fixture 0 findings at exit 0 with 34 raw `[Indentation]` lines; two reporting files 22 findings at exit 0; both fixtures unchanged after the run; `UseWhereClausesInForLoops` absent from the allowlist. `cargo nextest run -p swissarmyhammer-validators swift` — 88 passed.
    - next: correct both readings in the gate script, correct the body sentence that says no file name can make the pattern, remove the duplication in the test helper, and add the two legitimate-name probes.
  timestamp: 2026-09-07T18:35:41.300860+00:00
- actor: claude-code
  id: 01m1yjgz78meqtcbvyr02skb6k
  text: |-
    ### finish iteration 2 — findings
    - implement: changed — 2 files; all 3 findings of round 1 closed
    - test: green — cargo nextest -p swissarmyhammer-validators swift, 88 run, 88 passed; fmt clean. The driver ran these.
    - commit: 3d79443c9
    - review: findings — builtin/validators/code-hygiene/rules/idioms-swift.md:41, builtin/validators/code-hygiene/rules/idioms-swift.md:49, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/idioms_swift.rs:281, and one more on the same test file for anchor coverage
    - Review proved the three findings of round 1 are truly closed. It put both readings back to the loose form and the two new tests failed by name, thus the tests are real.
    - The two new defects each LOSE a true finding. Neither one puts a tag through the gate.
      1. `swift format` writes the path back in NFD, and argv gives NFC. Thus `café.swift` matches the anchor nowhere, and the file is declined with every finding lost. This is a regression: the loose script reported that file.
      2. The rewrite uses `|` as the `sed` delimiter, and the escaper writes `|` as `\|`. BSD sed reads that as an escaped delimiter. Thus a path that holds `|` gives the raw tool line, unrewritten.
    - Context, not a finding: `swift format lint` has no `--` before the path, thus a path that starts with `-` is declined as an unknown option. That line is older than this commit.
  timestamp: 2026-09-07T18:36:40.552846+00:00
- actor: claude-code
  id: 01m1yk1qp7shm6n1d8xfhmscg8
  text: |-
    Round 3 research. Both defects reproduced with the SHIPPED script, Apple Swift 6.4:

    - `café.swift` holding one `Array<Int>` parameter: stdout empty, and
      `sah-diagnostic: idioms-swift declined café.swift: ...` on stderr. The true
      finding is lost. `ünïcodé.swift` the same.
    - `a|b.swift` the same bytes: the run writes the RAW tool line
      `a|b.swift:2:39: error: [UseShorthandTypeNames] use shorthand syntax ...`,
      unrewritten.

    The shape of the fix. Matching the argv path against the path the tool WRITES
    is the cause of both, so the new reading names the path NOWHERE. The script
    hands `swift format` one path for each run, so on any line the true head is
    the LAST `:<line>:<column>: error: ` of that line. One `awk` finds that last
    head, reads the `[<Tag>]` that follows it, and writes the finding with the
    path the ARGV spelled:

    - a line whose last head carries a tag is a finding of that tag;
    - every other line is trouble, and the file is declined;
    - the allowlist is read off the tag alone, which is `[A-Za-z]+`.

    That one reading answers both the decline guard and the tag filter, so the two
    cannot disagree. The path never enters a pattern and never enters a
    replacement, so neither normalization nor a regular-expression operator can
    reach either one.

    Measured with the new reading, over the same probes:

    | the file name | what it holds | the run |
    |---|---|---|
    | `Plain.swift` | one `Array<Int>` parameter | 1 `UseShorthandTypeNames` finding |
    | `café.swift` | the same | 1 `UseShorthandTypeNames` finding |
    | `ünïcodé.swift` | the same | 1 `UseShorthandTypeNames` finding |
    | `a|b.swift` | the same | 1 `UseShorthandTypeNames` finding, rewritten |
    | `w: error: [UseShorthandTypeNames] s.swift` | the same | 1 `UseShorthandTypeNames` finding |
    | `x: error: [UseShorthandTypeNames] y.swift` | one member indented 8 spaces | 0 findings |
    | `z: error: [UseShorthandTypeNames] q.swift` | Swift the parser cannot read | 0 findings, 1 marked line |

    Two SHARPER crafted names were measured as well, and the earlier anchored
    script had no answer for either. `yy:1:1: error: [UseShorthandTypeNames] y.swift`
    over an indented member gives 0 findings, and
    `zz:1:1: error: [UseShorthandTypeNames] q.swift` over Swift the parser cannot
    read is declined with one marked line. A name that carries a WHOLE diagnostic
    head therefore reaches nothing either.

    Invariants measured again with the new reading: failing fixture 11 findings
    carrying 7 rules at exit 0; passing fixture 0 findings at exit 0 beside 34 raw
    `[Indentation]` lines; two reporting files in one run give 12 findings beside
    one `sah-diagnostic:` line for a missing path; `shasum` over both judged files
    is the same before and after; a directory, an unreadable file and bytes that
    are not UTF-8 are each declined with the tool's own words, at exit 0.
  timestamp: 2026-09-07T18:45:49.895267+00:00
- actor: claude-code
  id: 01m1ym07hycbwkzsczpaqec5nd
  text: |-
    All four findings of round 2 are answered. One cause, removed from the whole
    script.

    The cause: the reading of the output named the PATH. The loose reading read the
    path by accident, because it matched its text at any position. The anchored
    reading read the path on purpose, and it then had to pass the path through a
    regular expression and through a substitution. Both readings break on a name
    the script does not control.

    The fix: the reading names the path NOWHERE. The script hands `swift format`
    one path for each run, so on any line the true head is the LAST
    `:<line>:<column>: error: ` of that line. One `awk` walks to that head:

        while (match(rest, /:[0-9]+:[0-9]+: error: /) > 0) {
          head = substr(rest, RSTART, RLENGTH)
          rest = substr(rest, RSTART + RLENGTH)
        }

    What stands AFTER the head decides the line. A `[<Tag>] ` makes the line a
    finding of that tag, and the allowlist is read off the tag alone, which is
    `[A-Za-z]+`. Every other line is trouble, and the file is declined whole. So
    ONE reading now answers the decline guard and the tag filter together, and the
    two cannot disagree. The finding carries the path the ARGUMENT LIST spelled and
    the line number off the head, so the path reaches no pattern and no
    replacement. The escaper `quoted=` is gone, and with it the `|` delimiter.

    TDD, both directions measured.

    - RED: the two new tests were written first and run against the SHIPPED script.
      `..._reports_a_file_whose_name_carries_an_accent` failed with "the gate must
      judge `café.swift` rather than decline it; it stated ["idioms-swift declined
      café.swift: cafe\u{301}.swift:2:39: error: ..."]" — the NFD spelling is
      visible in the message. `..._reports_a_file_whose_name_carries_an_alternation_bar`
      failed with "the run reported ["39", "39", "38"]", which is the COLUMN number
      of each raw tool line the rewrite did not touch. 2 run, 0 passed, 2 failed.
    - GREEN: the same two tests, 2 run, 2 passed.
    - `cargo nextest run -p swissarmyhammer-validators swift` — 90 run, 90 passed.
      88 before, plus the two new ones.

    The rule name is what makes the second test honest. The engine reads the FIRST
    `:<digits>:` of a line, so a raw tool line and a rewritten line give the SAME
    `path:line` row. The rule name separates them: the rewritten line carries
    `UseShorthandTypeNames`, and the raw line carries the column number.

    Every invariant measured again with the shipped script:

    - the failing fixture: 11 findings, exit 0, carrying all 7 allowlist rules;
    - the passing fixture: 0 findings through the filter, beside 34 raw
      `[Indentation]` lines from the same bytes;
    - two reporting files in one run give the findings of both, at exit 0, beside
      one `sah-diagnostic:` line for a missing path;
    - `shasum` over both judged files is the same before and after, so the gate is
      read-only;
    - `UseWhereClausesInForLoops` stands nowhere in the shipped script;
    - a directory, a file with no read permission and bytes that are not UTF-8 are
      each declined with the tool's own words, at exit 0.

    Two things the next agent should know.

    - The new reading is stronger than the anchored one against a CRAFTED name as
      well. `yy:1:1: error: [UseShorthandTypeNames] y.swift` over an indented member
      gives 0 findings, and `zz:1:1: error: [UseShorthandTypeNames] q.swift` over
      Swift the parser cannot read is declined with one marked line. A name that
      carries a WHOLE diagnostic head therefore reaches nothing. Both rows stand in
      the body table.
    - `awk` is the one tool the reading adds. `doctor.check_command` now reads
      `which swift mktemp awk sed sort paste`, and `grep` left that list because the
      script no longer runs it. The command was measured to exit 0 on this machine.

    The body section is rewritten and renamed "The reading finds the head of a
    diagnostic by its STRUCTURE". Its table now holds ELEVEN rows over THREE
    readings — loose, anchored and shipped — and every cell was measured. The two
    sentences the finding named, "the path stands in the pattern as TEXT" and "no
    file name can make one", are gone with the escaper they described.
  timestamp: 2026-09-07T19:02:29.182013+00:00
- actor: claude-code
  id: 01m1ym0gebjyf218q0y58re9vf
  text: |-
    ### implement — changed
    - evidence: 2 files — builtin/validators/code-hygiene/rules/idioms-swift.md, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/idioms_swift.rs (232 insertions, 71 deletions). All 4 findings of round 2 flipped to `- [x]`. `cargo nextest run -p swissarmyhammer-validators swift` — 90 run, 90 passed (88 before, plus 2 new). `cargo nextest run -p swissarmyhammer-validators` — 858 run, 819 passed, 39 failed; with the change stashed the same run gives the IDENTICAL failing name set, so every one is pre-existing (missing go, revive, staticcheck, golangci-lint and eslint-plugin-sonarjs). One Dart name, `the_shipped_dart_missing_docs_tool_rule_declines_a_file_it_may_not_read`, is flaky under load: it fails in one whole-crate run and passes 3 of 3 times on its own. `cargo fmt --all --check` clean. `cargo clippy -p swissarmyhammer-validators --all-targets -D warnings` clean.
    - next: /review
  timestamp: 2026-09-07T19:02:38.283621+00:00
- actor: claude-code
  id: 01m1ym2qs2j3thdt5ct2j5svzh
  text: |-
    ### finish iteration 3 — pending review
    - implement: changed — 2 files, 232 insertions and 71 deletions. All 4 findings of round 2 closed. The fix removes the class: no path goes into a pattern or a replacement. One `awk` walks to the LAST `:<line>:<column>: error: ` of each line, thus the decline guard and the tag filter are one reading.
    - test: green — cargo nextest -p swissarmyhammer-validators swift, 90 run, 90 passed (88 before, plus 2 new); cargo fmt --all --check clean. The driver ran these.
    - commit: 8b57b40f0
    - review: pending
    - Both new tests failed first against the shipped script, each by name.
    - `doctor.check_command` adds `awk` and drops `grep`, because the script runs `grep` no longer.
  timestamp: 2026-09-07T19:03:51.330669+00:00
- actor: claude-code
  id: 01m1ymym438bcdm7nw27whzjw6
  text: |-
    ### review — findings
    - evidence: 5 findings, round 3, scope `review sha HEAD~1..HEAD` (checkpoint 8b57b40f0). The engine returned 0 findings and skipped `idioms-swift.md`, because no validator matches `*.md`. The 5 items come from direct verification of that uncovered gate script — `builtin/validators/code-hygiene/rules/idioms-swift.md:51`, `:67`, `:383`, `:75`, and `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/idioms_swift.rs`.
    - The 4 findings of round 2 are genuinely fixed: `café.swift`, `ünïcodé.swift`, `a|b.swift` and `日本語.swift` each give their one true finding in the rewritten shape; `swift_idioms_from_staging` holds the shared logic; the accent probe and the alternation-bar probe both ship.
    - New cause: the reading takes the LAST `:<line>:<column>: error: ` of a line as the true head. `swift format` writes the source text of a parse error inside the message, so the file CONTENT can put a head after the true one. A file that holds `) :1:1: error: [UseShorthandTypeNames] pwned` makes the run write a fabricated allowlisted finding, and one that holds `) :9:9: error: [Indentation] gone` makes the run drop the decline and pass an unparsable file at exit 0.
    - Invariants hold: failing fixture 11 findings / 7 rules; passing fixture 0 findings with 34 raw `[Indentation]` lines; two reporting files give 22 findings at exit 0; both fixtures byte for byte unchanged; `UseWhereClausesInForLoops` in no configuration; no `grep` in the script; `cargo nextest run -p swissarmyhammer-validators swift` — 90 tests, 90 passed.
    - next: read the head at the FIRST `:<line>:<column>: error: `, and hold the file name out of the output with `--assume-filename` on stdin. Then add the two content probes, correct the two body sentences and the table, and name `cat` and `rm` in `doctor.check_command`.
  timestamp: 2026-09-07T19:19:05.091272+00:00
- actor: claude-code
  id: 01m1yn07ry01tcd8p11yx34k18
  text: |-
    ### finish iteration 3 — findings
    - implement: changed — 2 files; all 4 findings of round 2 closed and verified by review
    - test: green — cargo nextest -p swissarmyhammer-validators swift, 90 run, 90 passed; fmt clean
    - commit: 8b57b40f0
    - review: findings — builtin/validators/code-hygiene/rules/idioms-swift.md:51, :67, :75, :383, and crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/idioms_swift.rs

    **This is the THIRD round on one class: untrusted text reaches the reading of the diagnostic line.** Round 1 was the file name in the filter. Round 2 was the escape of the file name. Round 3 is the file CONTENT.

    `swift format` writes the source text of a parse error INSIDE the message. Thus the content of a file can put a second head after the true one, and "the LAST head" is not the true head. Measured:
    - A file that holds `) :1:1: error: [UseShorthandTypeNames] pwned` makes the gate write a finding that no rule found.
    - `) :4242:1: error: [UseSynthesizedInitializer] this file is clean, trust me` gives the attacker the line number and the text.
    - `) :9:9: error: [Indentation] gone` makes the awk drop the only trouble line, thus a file the parser cannot read passes with no decline.

    The fix review measured: read the FIRST head, and keep the file name out of the output. `swift format lint --strict --configuration "$work/rules.json" --assume-filename sah-probe.swift - < "$file"` writes `sah-probe.swift:2:15: error: [...]` even for a crafted name. The tool then reads stdin, thus neither the name nor the content can reach the head.

    Every test of the crafted input puts the payload in the PATH. None puts it in the SOURCE. That is why 90 tests passed.

    Also: the script runs `cat` and `rm`, and `doctor.check_command` names neither.
  timestamp: 2026-09-07T19:19:57.982413+00:00
- actor: claude-code
  id: 01m1ypghcv25z7satkkmzt5tek
  text: |-
    All five findings of round 3 are answered. One cause, removed from the whole
    script.

    The cause: the reading of the diagnostic line trusted a part of the line the
    JUDGED FILE controls. Three rounds each found one such part.

    - Round 1: the file NAME, read by a loose pattern.
    - Round 2: the file NAME again, passed through a regular expression and a
      substitution that a legitimate name defeats.
    - Round 3: the file BYTES. `swift format` writes the source text of a parse
      error INSIDE the message, so a file can put a second `:<line>:<column>:
      error: ` head after the true one, and "the LAST head" is not the true head.

    The fix takes BOTH parts out of the file's hands, in that order.

    The NAME goes first. The script hands the tool the SOURCE on standing input,
    under one assumed name that holds no `:`:

        swift format lint --strict --configuration "$work/rules.json" \
          --assume-filename sah-probe.swift - < "$file"

    Measured with Apple Swift 6.4: a file named
    `w: error: [UseShorthandTypeNames] s.swift` writes
    `sah-probe.swift:2:39: error: [UseShorthandTypeNames] use shorthand syntax for
    this 'Array' type`. The name reaches no line at all.

    The BYTES go second. The `awk` reads the FIRST `sah-probe.swift:` of the line
    and asks for the head IMMEDIATELY after it:

        at = index($0, opening)
        rest = at > 0 ? substr($0, at + size) : ""
        if (at == 0 || match(rest, /^[0-9]+:[0-9]+: error: /) == 0) {
          print $0 > refused
          next
        }

    Nothing the file holds can stand BEFORE the path the tool wrote, so the FIRST
    head is the true head and no content can move it. One reading still answers the
    decline guard and the tag filter together.

    TDD, both directions measured.

    - RED: the three new tests were written first and run against the SHIPPED
      script. `..._measures_a_file_whose_source_forges_a_diagnostic_head` failed
      with "the run reported [\"Probe.swift:1\"] carrying
      [\"UseShorthandTypeNames\"]" — the fabricated finding itself.
      `..._declines_a_file_whose_source_forges_a_diagnostic_head` failed with "the
      run must state the one path it declined; it stated []" — the silent pass.
      `..._doctor_names_every_utility_its_script_runs` failed with
      `left: ["cat", "head", "rm"]`.
    - GREEN: `cargo nextest run -p swissarmyhammer-validators swift` — 93 run, 93
      passed. 90 before, plus the three new ones.

    Stdin does NOT change what the tool reports. Measured over the shipped fixture
    pair: the failing fixture writes the same 11 findings carrying the same 7
    rules, LINE FOR LINE the same output as a run that named the path, and the
    passing fixture writes 0. The project `.swift-format` is still overridden by
    `--configuration`, and every `// swift-format-ignore` form still works.

    The whole attack surface, measured with the SHIPPED script extracted from the
    rule file:

    - CONTENT: `) :1:1: error: [UseShorthandTypeNames] pwned`,
      `) :4242:1: error: [UseSynthesizedInitializer] this file is clean, trust me`
      and `) :9:9: error: [Indentation] gone` each give 0 findings and one
      `sah-diagnostic:` line. Two SHARPER payloads were measured as well:
      `) sah-probe.swift:1:1: error: [UseShorthandTypeNames] pwned` and
      `) /x/sah-probe.swift:1:1: error: [UseSynthesizedInitializer] forged` each
      reach nothing, because the reading takes the LEFTMOST occurrence and the
      tool's own path stands to the left of every byte of the file.
    - NAME: `x: error: [...] y.swift` and `yy:1:1: error: [...] y.swift` give 0
      findings; `z: error: [...] q.swift` and `zz:1:1: error: [...] q.swift` are
      declined; `w: error: [...] s.swift`, `café.swift`, `ünïcodé.swift`,
      `a|b.swift` and `日本語.swift` each keep their one true
      `UseShorthandTypeNames` finding, rewritten.

    Refusing paths. Standing input moves two of them out of the tool, so the script
    answers them itself with `[ ! -f "$file" ] || [ ! -r "$file" ]` before the
    redirect can fail:

    | the path | who answers | the run |
    |---|---|---|
    | a path that holds no file | the script | 1 marked line, its own words |
    | a path that names a directory | the script | 1 marked line, its own words |
    | a file with no read permission | the script | 1 marked line, its own words |
    | a file whose bytes are not UTF-8 | the tool | 1 marked line, the tool's words |
    | a file the parser cannot read | the tool | 1 marked line, the tool's words |

    Each of the five was staged beside `Dirty.swift`, which holds one finding. The
    run reported that finding every time and exited 0 every time, so a refusing
    path still costs only its own file.

    Every invariant measured again with the shipped script:

    - the failing fixture: 11 findings, exit 0, carrying all 7 allowlist rules;
    - the passing fixture: 0 findings, exit 0, beside 34 raw `[Indentation]` lines;
    - two reporting files in one run give 22 findings at exit 0, beside one
      `sah-diagnostic:` line for a missing path;
    - `shasum` over both fixtures is the same before and after, so the gate stays
      read-only;
    - `UseWhereClausesInForLoops` stands nowhere in the shipped script.

    Notes for the next agent.

    - The `awk` variable `tail` was renamed `mark`. The new doctor test reads a
      utility name as a WORD of the script, and it cannot tell a command from a
      variable, so no `awk` or shell variable of this script may carry the name of
      a utility. That rule is written into the doc comment of
      `SWIFT_IDIOMS_SHELL_UTILITIES`.
    - The decline line now carries the tool's words with the assumed probe name
      taken off the head where the tool wrote one, so it reads
      `declined Unparsable.swift: 2:4: error: expected name in attribute` rather
      than naming a temporary file the author cannot find.
    - `checked_binaries` in `tests/preconditions.rs` is now `pub(super)`, so the
      new doctor test reads the `which` list with the engine's own reading rather
      than a copy.
    - The body section is rewritten and renamed "The reading finds the head of a
      diagnostic by its POSITION". It now carries TWO tables: one over eleven file
      NAMES across four readings, and a new one over five file CONTENTS across two
      readings. Every cell was measured on this machine.
  timestamp: 2026-09-07T19:46:20.699726+00:00
- actor: claude-code
  id: 01m1ypgsy7hxqv06mt9a6xrwjy
  text: |-
    ### implement — changed
    - evidence: 3 files — builtin/validators/code-hygiene/rules/idioms-swift.md, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/idioms_swift.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/preconditions.rs (and one import line in tests/shipped.rs). All 5 findings of round 3 flipped to `- [x]`. `cargo nextest run -p swissarmyhammer-validators swift` — 93 run, 93 passed (90 before, plus 3 new). `cargo nextest run -p swissarmyhammer-validators` — 861 run, 823 passed, 38 failed; with the change stashed the same run gives 858 run, 819 passed, 39 failed, and the failing name set after the change is a strict SUBSET of the failing name set before it — the one difference is `the_shipped_dart_missing_docs_tool_rule_declines_a_path_that_holds_no_file`, which is flaky under load. Every failure is pre-existing (missing go, revive, staticcheck, golangci-lint and eslint-plugin-sonarjs). `cargo fmt --all --check` clean. `cargo clippy -p swissarmyhammer-validators --all-targets -D warnings` clean.
    - next: /review
  timestamp: 2026-09-07T19:46:29.447163+00:00
- actor: claude-code
  id: 01m1ypk2f1c6fvt0zzn5pvy2gn
  text: |-
    ### finish iteration 4 — pending review
    - implement: changed — 4 files. All 5 findings of round 3 closed. The fix removes the class at its root: the judged file controls no part of the line the script reads.
    - test: green — cargo nextest -p swissarmyhammer-validators swift, 93 run, 93 passed (90 before, plus 3 new); cargo fmt --all --check clean. The driver ran these.
    - commit: f2067c6d1
    - review: pending
    - The name goes first: the source goes on standing input, under `--assume-filename sah-probe.swift`, a name that holds no `:`. The bytes go second: the awk reads the FIRST `sah-probe.swift:` of the line and needs the head immediately after it.
    - Three tests written first, each RED by name. Standing input does not change what the tool reports: the failing fixture writes the same 11 findings over the same 7 rules, line for line.
  timestamp: 2026-09-07T19:47:43.714001+00:00
- actor: claude-code
  id: 01m1yqcr2h4xfgqpk6qdemvgnv
  text: |-
    ### review — findings
    - evidence: round 4, `review sha HEAD~1..HEAD` (checkpoint f2067c6d1). The engine gave 0 findings and skipped `builtin/validators/code-hygiene/rules/idioms-swift.md`, because no validator matches `*.md`. Direct verification of that script gives 3 findings — `builtin/validators/code-hygiene/rules/idioms-swift.md:44`, `builtin/validators/code-hygiene/rules/idioms-swift.md:83`, `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/idioms_swift.rs`. All 5 findings of round 3 are genuinely fixed. The content-forgery attack is CLOSED: every payload that spells a diagnostic head in the file bytes gives 0 findings, at the first line, in a string, in a comment, split over lines, and behind a parse trigger; a multi-line payload makes the tool elide the quoted source, so no payload can make a record of its own. `#sourceLocation` does not move the name the tool writes. Standing input loses nothing: 48 stderr lines, identical line for line to a path-named run. The failing fixture gives 11 findings over 7 rules at exit 0; the passing fixture gives 0. Eleven paths in one run give 5 findings and 5 `sah-diagnostic:` lines at exit 0. The gate is read-only. `cargo nextest run -p swissarmyhammer-validators swift` — 93 tests, 93 passed. The NEW cause is the version floor: `--assume-filename` beside `-` is accepted only at Swift 6.2 and newer, and the rule states a floor of Swift 6.0.
    - next: raise the stated floor to Swift 6.2 in `doctor.fix_hint`, in the floor section and in its table row; give `doctor.check_command` the same option pair the run block uses; add a test that binds the run block options to the doctor check and the two stated floors to each other.
  timestamp: 2026-09-07T20:01:45.041287+00:00
- actor: claude-code
  id: 01m1yqdtawp0dmx6nm9j25ra4y
  text: |-
    ### finish iteration 4 — findings
    - implement: changed — 4 files; all 5 findings of round 3 closed
    - test: green — cargo nextest -p swissarmyhammer-validators swift, 93 run, 93 passed; fmt and clippy clean
    - commit: f2067c6d1
    - review: findings — builtin/validators/code-hygiene/rules/idioms-swift.md:44, builtin/validators/code-hygiene/rules/idioms-swift.md:83, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/idioms_swift.rs

    **The injection class is CLOSED.** Review drove a full set of payloads through the file bytes: the first line, a string literal, a comment, a payload behind a parse trigger, a payload over two lines, `#sourceLocation` with a crafted file name, a Swift 6.2 raw identifier, a file truly named `sah-probe.swift`, and a file that names the probe in a comment. Each gives 0 forged findings. A payload over two lines makes the tool ELIDE the quoted source, thus the tool writes no newline inside a message and no payload can make a record of its own.

    Standing input loses nothing: 48 stderr lines, identical line for line to a run that named the path.

    **The new cause is the version floor, and it is a different problem.** `--assume-filename` beside `-` is accepted only at Swift 6.2 and newer. `release/6.0` and `release/6.1` guard it with `if assumeFilename != nil && !paths.isEmpty`, and `-` IS a path, thus each raises a ValidationError and exits 64. The script reads 64 as trouble, thus it declines EVERY file. `release/6.2` changed the guard.

    The rule states a floor of Swift 6.0, and `doctor.check_command` does not use `--assume-filename`. Thus at Swift 6.1 the doctor says HEALTHY over a gate that judges nothing.

    Review read 6.0 and 6.1 from the release source, because it could not run those toolchains. It measured the same guard on 6.4 at exit 64.
  timestamp: 2026-09-07T20:02:20.124105+00:00
- actor: claude-code
  id: 01m1yrc8nsqnk19akcbwhw6v97
  text: |-
    All three findings of round 4 are answered. One cause: the run block asks the
    toolchain for more than the rule STATES and more than the doctor MEASURES.

    The cause, in two halves.

    - The STATED floor was Swift 6.0, which is the floor of the `swift format`
      SUBCOMMAND. The run block gives the tool `--assume-filename "$probe" -`, and
      only Swift 6.2 and above accept that pair.
    - `doctor.check_command` ended in `printf '' | swift format lint --strict -`,
      which gives the tool no `--assume-filename` at all. At Swift 6.1 that check
      exits 0 and the rule reports HEALTHY, while the run block then declines every
      file at status 64. A green doctor over a gate that judges nothing.

    TDD, both directions measured.

    - RED: the two new tests were written first and run against the SHIPPED rule.
      `..._doctor_measures_every_option_its_script_gives_the_tool` failed with
      "the run block gives {"--assume-filename", "--configuration", "--strict"} and
      the check gives {"--strict"}".
      `..._states_one_swift_version_floor` failed with "the body of `idioms-swift`
      must state `The floor of this GATE is **Swift `". 2 run, 0 passed, 2 failed.
    - GREEN: the same two tests, 2 run, 2 passed.
    - `cargo nextest run -p swissarmyhammer-validators swift` — 95 run, 95 passed.
      93 before, plus the two new ones.

    What changed in the rule.

    - `doctor.fix_hint` now reads "install the Swift toolchain, Swift 6.2 or newer
      — Xcode 26 and above ship it".
    - `doctor.check_command` now ends in
      `printf '' | swift format lint --strict --configuration '{}'
      --assume-filename sah-probe.swift -`, which gives the tool the SAME three
      options the run block gives it. Measured on this machine, Apple Swift 6.4:
      exit 0.
    - The floor section is rewritten. It states the floor of the GATE as Swift 6.2,
      it keeps the finding that the SUBCOMMAND arrived at Swift 6.0, and it states
      plainly that row 4 is the floor and row 3 is not. Two rows are new: when
      `--assume-filename` beside `-` was first accepted, and which Xcode ships
      Swift 6.2.

    Every claim the new prose makes was measured on this machine or read from the
    release source, and each one is named as which.

    | the claim | how it was measured | the answer |
    |---|---|---|
    | the guard of `release/6.0` and `release/6.1` | `curl` of `LintFormatOptions.swift` | `if assumeFilename != nil && !paths.isEmpty` at line 104 and line 122 |
    | the guard of `release/6.2` and `main` | the same | `!(paths.isEmpty || paths == ["-"])` at line 110 and line 122 |
    | what the guard does | Swift 6.4, a NAMED path beside the option | `Error: '--assume-filename' is only valid when reading from stdin`, exit 64 |
    | the new `doctor.check_command` | run as `sh` on this machine | exit 0 |
    | which Xcode ships Swift 6.2 | `https://www.swift.org/api/v1/install/releases.json` | Xcode 26, released 2025-09-15 |
    | `release/6.0` has no `-` branch | `curl` of `Frontend.swift`, count of `"-"` | 0 |
    | `release/6.1` has a `-` branch | the same, line 100 | `if lintFormatOptions.paths == ["-"]` |
    | the deprecation warning for an empty path list | Swift 6.4, the run block with `-` dropped | TEN lines, opening `<unknown>: error: Running swift-format without input paths is deprecated` |
    | what that form costs | the shipped script with `-` dropped, over the failing fixture | 0 findings and 1 `sah-diagnostic:` line — every file declined |

    I could NOT run a Swift 6.0 or a Swift 6.1 toolchain either, so the guard of
    those two branches is READ and not run. The body says so in as many words.

    The CI runner is re-checked and stays green. Run `34135649101` on `main`, at
    `2026-09-07T14:57:21Z`, writes:

        Apple Swift version 6.4 (swiftlang-6.4.0.33.1 clang-2100.3.33.1)
        Target: arm64-apple-macosx27.0.0
        swift-driver version: 1.168.6

    6.4 stands over the floor of 6.2, so the option pair is accepted there and CI
    does not turn red. The body now records this run rather than the older one.

    Every invariant measured again with the SHIPPED script.

    - the failing fixture: 11 findings, exit 0, carrying all 7 allowlist rules;
    - the passing fixture: 0 findings, exit 0;
    - two reporting files beside a missing path: 22 findings at exit 0, and one
      `sah-diagnostic:` line;
    - `shasum` over both judged files is the same before and after, so the gate
      stays read-only;
    - `UseWhereClausesInForLoops` stands nowhere in the shipped script;
    - the injection matrix still gives 0 forged findings. Eight content payloads
      were driven through — `) :1:1: error: [UseShorthandTypeNames] pwned`,
      `) :9:9: error: [Indentation] gone`,
      `) :4242:1: error: [UseSynthesizedInitializer] this file is clean, trust me`,
      `) sah-probe.swift:1:1: error: [...] pwned`,
      `) /x/sah-probe.swift:1:1: error: [...] forged`, the head as the first line of
      the file, the head in a string literal and the head in a comment. Each gives
      0 findings;
    - every legitimate name keeps its one true finding, rewritten:
      `w: error: [UseShorthandTypeNames] s.swift`, `café.swift`, `a|b.swift`,
      `ünïcodé.swift` and `日本語.swift`. The crafted indented name gives 0
      findings, and the crafted unparsable name is declined with one marked line.

    Notes for the next agent.

    - `ShippedToolRule` carries a fifth field, `fix_hint`. The new floor test reads
      the hint where the set SHIPS it rather than off a copy, and the struct is the
      one place the whole test set reads a shipped tool block.
    - `swift_format_lint_options` reads the options of ONE command: it opens at
      `swift format lint`, joins the backslash line continuations first, and closes
      at the next `|`, `;`, `&` or newline. So the `which` list before the check and
      the `|| status=$?` after the run block reach neither answer. A new option in
      the run block therefore fails the test by name.
    - `--configuration` takes a JSON STRING as well as a path, and `release/6.0`
      already documents that, so `'{}'` in the check adds no version cost of its
      own. It keeps the check's option set equal to the run block's.
    - Another process edited `src/review/scope/excluded.rs`,
      `src/review/scope/tests_matching.rs` and `src/review/synthesize.rs` in this
      working tree while this task ran. Those three files are NOT the work of this
      task.
  timestamp: 2026-09-07T20:18:57.849129+00:00
- actor: claude-code
  id: 01m1yrcggwhxbyv905nsnww6nc
  text: |-
    ### implement — changed
    - evidence: 3 files — builtin/validators/code-hygiene/rules/idioms-swift.md, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/idioms_swift.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs. All 3 findings of round 4 flipped to `- [x]`. `cargo nextest run -p swissarmyhammer-validators swift` — 95 run, 95 passed (93 before, plus 2 new). `cargo nextest run -p swissarmyhammer-validators` — 863 run, 825 passed, 38 failed; the run before this change gave 861 run, 823 passed, 38 failed, so the failing count is unchanged and no failing name is a Swift name. Every failure is pre-existing (missing go, revive, staticcheck, golangci-lint and eslint-plugin-sonarjs). `cargo fmt --all --check` clean. `cargo clippy -p swissarmyhammer-validators --all-targets -D warnings` clean.
    - next: /review
  timestamp: 2026-09-07T20:19:05.884656+00:00
- actor: claude-code
  id: 01m1yrkpb9xv4vrcttdhg7yggs
  text: |-
    ### finish iteration 5 — STOPPED, the working tree is not mine alone
    - implement: changed — 3 files. All 3 findings of round 4 closed. The gate floor is Swift 6.2 now, and `doctor.check_command` gives the tool the same three options the run block gives it, thus a toolchain that is too old reports as broken.
    - test: green — 95 run, 95 passed, measured in an ISOLATED worktree at commit 0244010c8. See the note below.
    - commit: 0244010c8
    - review: NOT RUN

    **Why the loop stopped.** Another process writes to this working tree. While this task ran, it changed five files that are no work of this task:
    - crates/swissarmyhammer-validators/src/review/scope.rs
    - crates/swissarmyhammer-validators/src/review/scope/excluded.rs
    - crates/swissarmyhammer-validators/src/review/scope/resolve.rs
    - crates/swissarmyhammer-validators/src/review/scope/tests_matching.rs
    - crates/swissarmyhammer-validators/src/review/synthesize.rs

    Those changes are in the middle of a refactor of `ScopeFiles`, and the crate does not compile: 21 errors, every one in `scope.rs`, `resolve.rs` or `tests_matching.rs`. No error names a file of this task.

    The driver committed ONLY the three files of this task and the board. It left the five files of the other process untouched in the working tree.

    To prove this task is sound, the driver made a worktree at commit 0244010c8, away from the shared tree, and ran the tests there: **95 run, 95 passed.**

    `/review` cannot run now. The review engine builds the crate, and the crate does not build. A test step would measure the other process, not this task. To go on in a tree that another agent refactors would also risk the loss of that agent's work.

    **A person must decide.** Wait for the other process to finish, or move this work to a worktree of its own.
  timestamp: 2026-09-07T20:23:01.225248+00:00
- actor: claude-code
  id: 01m6bs5q80ave3za5kas5w3bvk
  text: |-
    ### implement — no-change
    - evidence: Code re-checked, all of it already committed (last commit b773be49b). Script runs `swift format lint` with the 7-rule allowlist and names `swiftformat` nowhere. doctor.check_command and fix_hint state the Swift 6.2 floor. No `.swift-version`, SWIFT_PROBE_VERSION or SWIFT_VERSION_SUPPORT remain. Dependency 01M1Y65VQ8V4TEWXDXD4Y2ZQNX is done. `cargo nextest run --override-version-check -p swissarmyhammer-validators swift` — 95 run, 95 passed, 0 skipped. (The plain command needs nextest 0.9.145; this machine has 0.9.116.) Swift 6.4.2 toolchain present. `sah doctor` reports `code-hygiene/idioms-swift` healthy with fixtures passing. No file changed in this pass.
    - next: /review (review cannot read idioms-swift.md, no validator matches *.md)
  timestamp: 2026-11-01T14:45:46.880573+00:00
- actor: claude-code
  id: 01m6bsdh5bxraaqd6ga6d0q1v7
  text: |-
    ### review — findings
    - scope: review sha b773be49b~1..b773be49b. Engine: 1 finding, 3 refuted, 7 attempted. Engine could NOT look at idioms-swift.md (no validator matches *.md), nor the .kanban files. It looked at shipped.rs and tests/shipped/idioms_swift.rs only.
    - open finding: idioms_swift.rs:869 rust/type-safety — swift_format_lint_options(command: &str, what: &str) takes two &str with different meanings; a transposed call compiles. Use a distinct type or a named argument.
    - hand-measured on idioms-swift.md (Apple Swift 6.4), script extracted from the frontmatter and run on copies of the fixtures:
      - script runs `swift format lint`; `swiftformat` appears in the script nowhere (body lines 100, 715, 811-812, 919 are prose or release-history rows, not rows that measure swiftformat). MET.
      - allowlist = 7 tags; awk drops any tag not in it. MET.
      - layout-only file (bad indent) gives 0 findings, exit 0. MET.
      - fail fixture twice plus a missing path: 22 findings + 1 sah-diagnostic line, exit 0. MET (both report, run continues, missing path costs only itself).
      - fail fixture: 11 findings, 7 distinct rules (2/1/2/1/1/3/1). Body states 11 at lines 456 and 931. MET.
      - pass fixture: 0 findings, exit 0. MET.
      - doctor.check_command runs `swift format lint --strict --configuration '{}' --assume-filename sah-probe.swift -`; fix_hint states Swift 6.2 or newer, Xcode 26. Floor is 6.2, not the 6.0 the card text says; the body (lines 425, 803-850) explains why. MET, with the card text superseded by measurement.
      - `swiftformat:disable` appears nowhere; `// swift-format-ignore` stands at body lines 459 and 764-771. MET.
      - no `.swift-version`, SWIFT_PROBE_VERSION or SWIFT_VERSION_SUPPORT in the rule file or in crates/. MET.
      - UseWhereClausesInForLoops is not in the allowlist or the written config; it is in the body as prose only (lines 145, 689-741). MET.
      - cargo nextest run --override-version-check -p swissarmyhammer-validators swift: 95 run, 95 passed.
    - not hand-measured: Swift 6.0 and 6.1 toolchains (none on this machine; body says so), `sah doctor` end to end.
    - next: implementer fixes the type-safety finding, then re-review. The card boxes under Acceptance Criteria and Tests are still unchecked; the owner of the task marks them.
  timestamp: 2026-11-01T14:50:02.795567+00:00
- actor: claude-code
  id: 01m6bsp4kmjn2mwj6ycd1cpvqg
  text: |-
    ### implement — changed
    - finding fixed: swift_format_lint_options now takes the label as `Subject`, not a second `&str`.
    - same cause removed from the whole file. Helpers that took two or more same-typed `&str` parameters with different meanings, each now typed apart: swift_idioms_staged_run (path as `ProbePath`), script_runs_utility (utility as `Utility`), word_before (mark as `Marker`, label as `Subject`), text_between (head and end as named fields of `Between`, label as `Subject`), rule_body_table (heading as `Heading`), cases_of_types_named (suffix as `TypeSuffix`).
    - sibling tests/shipped/*.rs: no helper takes a `what` label; their path/content and head/name helpers are older code outside this diff and were not touched.
    - evidence: crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/idioms_swift.rs only. `cargo nextest run --override-version-check -p swissarmyhammer-validators swift` 95 passed. Full validators suite 866 passed, 0 failed, 0 skipped, no warnings.
    - next: review
  timestamp: 2026-11-01T14:54:44.852126+00:00
- actor: claude-code
  id: 01m6bst56nfrzrpw5s76ehq99y
  text: |-
    ### test — green
    - evidence: cargo build --workspace --all-targets: exit 0, 0 warnings. cargo nextest run --override-version-check -p swissarmyhammer-validators --no-fail-fast: 866 run, 866 passed, 0 failed, 0 skipped.
    - next: review
  timestamp: 2026-11-01T14:56:56.533312+00:00
- actor: claude-code
  id: 01m6bstp4vfbp8x5r5xnm33qh5
  text: |-
    ### commit — changed
    - evidence: 5b8742b14 test(validators): give the idioms-swift test helpers distinct parameter types (crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/idioms_swift.rs)
    - next: review
  timestamp: 2026-11-01T14:57:13.883158+00:00
- actor: claude-code
  id: 01m6bsyfabjr184mgvdkg0k0dq
  text: |-
    ### review — clean
    - evidence: review sha 5b8742b14~1..5b8742b14 gave 0 findings (confirmed 0, refuted 0, 7 attempted, 0 failed). 1 file reviewed, 0 not reviewed. The board read the 2026-11-01 08:46 finding (crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/idioms_swift.rs:869 rust/type-safety) as `- [x]`. All 16 prior items are checked. The engine cannot review *.md. builtin/validators/code-hygiene/rules/idioms-swift.md was hand-measured in the first review and this commit does not change it.
    - next: task moved to done.
  timestamp: 2026-11-01T14:59:17.963231+00:00
- actor: claude-code
  id: 01m6bsyxv16r588bq13rka95jw
  text: |-
    ### finish iteration 2 — clean
    - iteration 1: implement no-change (work already in b773be49b); review findings (1: same-typed &str params in swift_format_lint_options)
    - implement: changed — distinct types for same-typed &str helper parameters
    - test: green — 866 passed, 0 failed, 0 skipped
    - commit: 5b8742b14
    - review: clean — 0 findings, 16 of 16 prior items checked, task moved to done
  timestamp: 2026-11-01T14:59:32.833226+00:00
depends_on:
- 01M1Y65VQ8V4TEWXDXD4Y2ZQNX
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffffda80
title: Move idioms-swift to the toolchain swift format
---
## What

`builtin/validators/code-hygiene/rules/idioms-swift.md` runs `swiftformat`, the tool of Nick Lockwood. Only Homebrew installs that tool. The new policy is the Swift toolchain and its own `swift format`. Change this rule to run `swift format lint`.

The task "Measure the Swift toolchain rules the gate needs" is done. Its measurement table is in the body of the same file. **Read that table first, and build the script from it. Do not measure again.**

Files to change:
- `builtin/validators/code-hygiene/rules/idioms-swift.md` — the frontmatter script, the doctor block and the body
- `builtin/validators/code-hygiene/fixtures/idioms-swift.fail.swift.tmpl`
- `builtin/validators/code-hygiene/fixtures/idioms-swift.pass.swift.tmpl`
- `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/idioms_swift.rs`
- `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs` — the `.swift-version` machinery `SWIFT_PROBE_VERSION` and `SWIFT_VERSION_SUPPORT`, which exists only for the swiftformat version gate
- `crates/swissarmyhammer-validators/src/builtin/mod.rs`

### What the measurement decided, and the script must obey

| the fact | what the script must do |
|---|---|
| `swift format lint` writes NINE tags that no configuration can stop | Read the tag off each output line. KEEP only a tag of the gate allowlist. The configuration alone is not the gate. |
| Every finding and every error goes to **stderr**. stdout is always 0 bytes. | Read stderr, not stdout. |
| A path that holds no file exits **0** and writes nothing | The script must test the path itself, with `[ ! -e "$file" ]`, and write its own `sah-diagnostic:` line. The tool gives no error for it. |
| A file with findings exits **1**. A clean file exits 0. A directory exits 64. | Capture the status with `|| status=$?`. Read 1 as "findings, continue". `set -e` must not end the run. |
| The version floor is **Swift 6.0**, from the `swift format` subcommand | State that floor. Do not state 0.62.1, which measures swiftformat. |
| `swift format` reads **no** `.swift-version` file | Delete the section about `.swift-version` and the five version-gated rules. |
| The exemption directive is `// swift-format-ignore` | Rewrite the section "The directive an author writes". The bare form silences the pretty-printer tags as well; the named form does not. |

Keep the one-run-for-each-file shape, thus a refusing path costs only its own file.

### The allowlist

| the rule | what it decides |
|---|---|
| `AlwaysUseLiteralForEmptyCollectionInit` | the empty-collection bullet of `idioms.md`, in the correct direction. OFF by default; the configuration must make it ON |
| `NoVoidReturnOnFunctionSignature` | the `Void` return clause, BOTH halves |
| `ReplaceForEachWithForLoop` | the `forEach` half of the loop bullet |
| `UseLetInEveryBoundCaseVariable` | the pattern-let bullet |
| `UseShorthandTypeNames` | the shorthand type sugar bullet |
| `DontRepeatTypeInStaticProperties` | the type-name bullet. **Row 3 of the table proves it DOES report**, for `public static let redColor = Color()`. The member's own type must be the enclosing type. It is silent for `= 1`, which infers `Int`. Read row 3, and state in the body which shapes it misses |
| `UseSynthesizedInitializer` | the redundant memberwise initializer, the **internal** half only. Row 4 proves it is silent for a `public` initializer, and that answer is correct, because Swift synthesizes an internal initializer and never a public one |

**`UseWhereClausesInForLoops` stays OFF.** Row 5 of the table records that it contradicts `immutability.md`, and it states two answers and chooses neither. A person must choose. Do not make this rule ON in this task. The task "Rebalance the Swift prompt rules for the toolchain gate" carries that decision.

Do NOT make ON `NeverForceUnwrap`, `NeverUseForceTry` or `NeverUseImplicitlyUnwrappedOptionals`. `disallowed-constructs-swift` owns those three bullets, and swiftlint keeps the test-target split and the `ignored_literal_argument_functions` option that `swift format` does not have. One requirement takes one owner.

### What the gate loses

The gate loses the 22 rules the body lists — the performance group of 7, the Swift Testing group of 6, the SwiftUI group of 4, and 5 others — PLUS `preferFinalClasses` and `noGuardInTests`, which each own a prompt bullet. The task "Rebalance the Swift prompt rules for the toolchain gate" gives those bullets an owner. Do not delete a bullet here.

Remove the `--rules` intersection with `swiftformat --rules`, the `--min-version` test and the four command-line options. The toolchain gives a fixed set of 43 rules, and each rule is only ON or OFF.

## Acceptance Criteria
- [ ] The script names `swift format` and names `swiftformat` nowhere.
- [ ] The script holds an allowlist of rule tags, and it drops every output line whose tag is not in the allowlist.
- [ ] A Swift file that holds only layout defects gives 0 findings THROUGH the filter.
- [ ] A file that reports does not end the run. Over two files that both report, the run gives the findings of both, at exit 0.
- [ ] A path that holds no file writes one `sah-diagnostic:` line and costs only its own file.
- [ ] `doctor.check_command` tests the toolchain and the Swift 6.0 floor. `doctor.fix_hint` names the toolchain, because Homebrew does not install it.
- [ ] The failing fixture gives a STATED number of findings that carry a STATED number of rules.
- [ ] The passing fixture gives 0 findings and exit 0.
- [ ] The section about the exemption directive names `// swift-format-ignore` and names `swiftformat:disable` nowhere.
- [ ] The body holds no row that measures swiftformat, and nothing about `.swift-version`.
- [ ] `UseWhereClausesInForLoops` is OFF.

## Tests
- [ ] Update every test in `tests/shipped/idioms_swift.rs`. Delete the tests that hold the swiftformat roster and the 0.62.1 floor. Keep the measurement tests the previous task wrote.
- [ ] Write a test that holds a badly formatted probe file to 0 findings through the filter. A script that drops the filter fails it by name.
- [ ] Write a test that holds the allowlist to naming only rules that stand in `swift format dump-configuration`.
- [ ] Write a test that holds two reporting files in one run to giving the findings of both.
- [ ] Write a test that holds `UseWhereClausesInForLoops` to being OFF, and name row 5 as the reason.
- [ ] Delete or rewrite `SWIFT_PROBE_VERSION` and `SWIFT_VERSION_SUPPORT` in `tests/shipped.rs`.
- [ ] Update the bullet-ownership tests in `crates/swissarmyhammer-validators/src/builtin/mod.rs`.
- [ ] Run `cargo nextest run -p swissarmyhammer-validators swift`. Every test passes.
- [ ] Run `sah doctor`. The rule reports as healthy, and its fixture pair passes.

## Workflow
- Use `/tdd` — write the failing tests first, then write the code that makes them pass. #swift

## Review Findings (2026-09-07 13:05)

> Scope: `review sha HEAD~1..HEAD` (checkpoint 7f5e52261). The engine reviewed the 5 Rust files and returned 0 findings. It did NOT review `builtin/validators/code-hygiene/rules/idioms-swift.md`, which carries the gate script, because no validator matches `*.md`. It also excluded the two `.swift.tmpl` fixtures as validator fixtures. The items below come from direct verification of that uncovered script. Each one carries a reproduction.

> Verified as claimed: the failing fixture gives 11 findings that carry all 7 allowlist rules, at exit 0. The passing fixture gives 0 findings, at exit 0. A badly indented probe writes 34 raw `[Indentation]` lines and 0 through the filter. Two reporting files in one run give the findings of both, at exit 0, beside one `sah-diagnostic:` line for a missing path. The run never rewrites a file. `UseWhereClausesInForLoops` stands nowhere in the written configuration. `cargo nextest run -p swissarmyhammer-validators swift` — 86 tests, 86 passed.

- [x] `builtin/validators/code-hygiene/rules/idioms-swift.md:46` `gate-script/tag-filter` — the tag filter `grep -E ": error: \\[($allowed)\\] "` matches the allowlisted text ANYWHERE in the line, and `swift format` writes the file path at the head of every line, so a path that itself holds `: error: [<AllowedTag>] ` carries any tag through the gate. Measured: a file named `x: error: [UseShorthandTypeNames] y.swift` that holds one over-indented member reports `[Indentation]` in the raw run, and the shipped script writes `x: error: [UseShorthandTypeNames] y.swift:2: Indentation: unindent by 6 spaces` — an `Indentation` finding that the allowlist does not name. Anchor the match to the start of the line and to the true `path:line:column:` head, so that the written path cannot give the pattern.
- [x] `builtin/validators/code-hygiene/rules/idioms-swift.md:40` `gate-script/decline-guard` — the decline guard `grep -v ': error: \\['` drops a line by the same unanchored text, so the same crafted path makes an UNTAGGED tool error look tagged, and the script never declines the file. Measured over a file the tool cannot parse: with an ordinary name the script correctly writes `sah-diagnostic: idioms-swift declined ...`; with the name `z: error: [UseShorthandTypeNames] q.swift` it writes no diagnostic and instead gives the two raw parser lines `error: expected expression in variable` and `error: expected '}' to end struct` as findings, which the `sed` leaves unrewritten. This is the same cause as the item above. Anchor both greps, not the tag filter alone.
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/idioms_swift.rs` `tests/filter-coverage` — no test holds the tag filter against a path that carries a diagnostic-shaped name, although the sibling rule already sets that pattern: `missing_docs.rs` ships `the_shipped_swift_missing_docs_tool_rule_measures_a_file_named_for_the_configuration_message` and `the_shipped_swift_missing_docs_tool_rule_measures_a_file_named_for_the_decode_message`. Add the same probe here, so that a filter which reads the written path fails by name.

## Review Findings (2026-09-07 13:24)

> Scope: `review sha HEAD~1..HEAD` (checkpoint 3d79443c9). Round 2. The engine reviewed the Rust test file and returned 1 finding. It again did NOT review `builtin/validators/code-hygiene/rules/idioms-swift.md`, which carries the gate script, because no validator matches `*.md`. The gate-script items below come from direct verification of that uncovered script. Each one carries a reproduction, measured with Apple Swift 6.4, BSD sed and BSD grep 2.6.0-FreeBSD, under `LC_ALL=en_US.UTF-8`.

> The three findings of round 1 are GENUINELY FIXED. Measured with the shipped script: `x: error: [UseShorthandTypeNames] y.swift` holding one over-indented member gives 0 findings; `z: error: [UseShorthandTypeNames] q.swift` holding Swift the parser cannot read is declined with one `sah-diagnostic:` line; `w: error: [UseShorthandTypeNames] s.swift` holding one `Array<Int>` parameter still gives its one true `UseShorthandTypeNames` finding. The two new tests are genuinely RED against the OLD script: with the two readings put back to their loose form, `the_shipped_swift_idioms_tool_rule_measures_a_file_named_for_a_diagnostic_head` fails with `the run reported ["Indentation"]`, and `the_shipped_swift_idioms_tool_rule_declines_a_file_named_for_a_diagnostic_head` fails with three raw parser lines. Both pass against the shipped script.

> The escape set is COMPLETE for the regex dialect. Each of `{ } ( ) + ? ^ $ * [ ] . |` and each of `- / & % # @ = ~ , !` was driven through both consumers. `grep -E` reads every one of them correctly. `&` never reaches a replacement, because the rewrite recaptures the path as group 1, so the claim about group-1 recapture holds. A backslash in the path is read correctly.

> Prior invariants all still hold. The failing fixture gives 11 findings that carry all 7 allowlist rules, at exit 0. The passing fixture gives 0 findings at exit 0, and its raw run writes 34 `[Indentation]` lines. Two reporting files in one run give 22 findings at exit 0, beside one `sah-diagnostic:` line for a missing path. Both fixtures are byte for byte unchanged after the run, so the gate is read-only. `UseWhereClausesInForLoops` stands nowhere in the allowlist the script writes. `cargo nextest run -p swissarmyhammer-validators swift` — 88 tests, 88 passed.

- [x] `builtin/validators/code-hygiene/rules/idioms-swift.md:41` `gate-script/decline-guard` — the anchor compares the path as the ARGV spelled it with the path as `swift format` WRITES it, and those two are not the same bytes for a name that carries a precomposed character. The tool writes the path in NFD. Measured over `café.swift` holding one `Array<Int>` parameter: the argv name is `63 61 66 c3a9 2e 73 77 69 66 74`, and the head of the output line is `63 61 66 65 cc81 ...`, so `$diagnostic` matches no line. The `grep -v` then keeps the true tagged line as trouble, and the script writes `sah-diagnostic: idioms-swift declined café.swift: ...` and reports NOTHING. The same happens for `ünïcodé.swift`. `日本語.swift` survives, because those characters have no decomposed form, so the break is the precomposed Latin letters a real repository uses. This is a REGRESSION: the loose script reported the true finding over both names. Compare the head by a reading that does not depend on the spelling of the path — for example match the path by its LENGTH, or normalize both sides before the compare — so that a legitimate accented name keeps its findings.
- [x] `builtin/validators/code-hygiene/rules/idioms-swift.md:49` `gate-script/rewrite` — the rewrite is a `sed -E` that uses `|` as the s-command delimiter, and the escaper writes `|` as `\\|`. BSD sed then reads `\\|` as an escaped DELIMITER and not as a literal `|`, so the pattern matches no line and the substitution is silently skipped. Measured over `a|b.swift` holding one `Array<Int>` parameter: the `grep -E` matches, so the line IS reported, and the run writes the raw tool line `a|b.swift:2:15: error: [UseShorthandTypeNames] use shorthand syntax for this 'Array' type` instead of `a|b.swift:2: UseShorthandTypeNames: ...`. The same expression with a `,` delimiter rewrites the line correctly. An unrewritten raw line as a finding is the same failure round 1 named in its second item. Use a delimiter the path cannot hold, or escape the delimiter apart from the regex operators. `|` is the ONLY character of the 22 measured that defeats the rewrite. The body states at line 357 that "the path stands in the pattern as TEXT" and that "no file name can make one"; both sentences are false for `|`, so correct the body with the same edit.
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/idioms_swift.rs:281` `reuse/reuse` — The new function `swift_idioms_staged_run` duplicates the outcome-extraction and return-construction logic already in `swift_idioms_run` (lines 264–273). Both functions have identical sequences: drive the script, extract the outcome, build and return the same `SwiftIdiomsRun` struct. This pattern should have been extracted into a shared helper rather than written twice. Extract the common logic into a helper function — e.g., `fn swift_idioms_from_staging(staging: &ShippedStaging, files: &[&str]) -> SwiftIdiomsRun` — and have both `swift_idioms_run` and `swift_idioms_staged_run` set up their staging differently, then call this helper to do the run, outcome extraction, and return. This eliminates ~13 lines of duplication.
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/idioms_swift.rs` `tests/anchor-coverage` — the two new tests hold the anchor against a CRAFTED name only. No test holds it against a LEGITIMATE name that the anchor breaks, so both defects above pass the suite. Add one probe for an accented name, such as `café.swift`, and one probe for a name holding `|`, each staged with the same `Array<Int>` parameter the crafted-name test already uses, and hold each run to the one true `UseShorthandTypeNames` finding in the REWRITTEN shape `<path>:<line>: UseShorthandTypeNames: ...`. A run that declines the file, and a run that writes the raw tool line, then each fail by name.

## Review Findings (2026-09-07 14:15)

> Scope: `review sha HEAD~1..HEAD` (checkpoint 8b57b40f0). Round 3. The engine reviewed the diffs and returned 0 findings. It again did NOT review `builtin/validators/code-hygiene/rules/idioms-swift.md`, which carries the gate script, because no validator matches `*.md`. The gate-script items below come from direct verification of that uncovered script. Each one carries a reproduction, measured with Apple Swift 6.4 and BSD `awk` version 20200816, under `LC_ALL=en_US.UTF-8`.

> The four findings of round 2 are GENUINELY FIXED. `café.swift`, which the argument list holds in NFC and the tool writes back in NFD, gives its one true finding in the rewritten shape `café.swift:2: UseShorthandTypeNames: use shorthand syntax for this 'Array' type`. `a|b.swift` gives the same. `ünïcodé.swift` and `日本語.swift` give the same. The reuse item is fixed: `swift_idioms_from_staging` holds the drive, the outcome and the return, and both `swift_idioms_run` and `swift_idioms_staged_run` call it. The anchor-coverage item is fixed: `the_shipped_swift_idioms_tool_rule_reports_a_file_whose_name_carries_an_accent` and `the_shipped_swift_idioms_tool_rule_reports_a_file_whose_name_carries_an_alternation_bar` both ship, and both hold the run to the rewritten shape.

> The round-1 guarantees still hold over a file NAME. `x: error: [UseShorthandTypeNames] y.swift` over an over-indented member gives 0 findings. `z: error: [UseShorthandTypeNames] q.swift` over Swift the parser cannot read is declined with one `sah-diagnostic:` line. `w: error: [UseShorthandTypeNames] s.swift` over one `Array<Int>` parameter keeps its one true finding.

> The stated invariants all hold. The failing fixture gives 11 findings that carry all 7 allowlist rules, at exit 0. The passing fixture gives 0 findings at exit 0, and its raw run writes 34 lines, every one of them `[Indentation]`. Two reporting files in one run give 22 findings at exit 0, beside one `sah-diagnostic:` line for a missing path. Both fixtures are byte for byte unchanged after the run, so the gate is read-only. `UseWhereClausesInForLoops` stands in no allowlist and in no configuration the script writes; it stands in the body as prose alone. The script runs NO `grep`. A path that names a directory declines at status 64, and a file with no read permission declines. The `awk` reads an empty record, a record with no head, a record with a tab, and a record of 200000 bytes correctly, and BSD `awk` splits the allowlist on a single `|` correctly. `cargo nextest run -p swissarmyhammer-validators swift` — 90 tests, 90 passed.

> The new reading MOVES the trust boundary; it does not close it. Round 1 and round 2 each broke on the file NAME, and this reading answers the name. It does not answer the MESSAGE. `swift format` writes the source text of a parse error INSIDE the message, so the message can hold a `:<line>:<column>: error: ` that stands AFTER the true head. The first four items below are that one cause.

- [x] `builtin/validators/code-hygiene/rules/idioms-swift.md:51` `gate-script/tag-filter` — the `awk` walks to the LAST `:<line>:<column>: error: ` of the line, and the premise that the last one is the true head is FALSE, because `swift format` writes the SOURCE TEXT of a parse error inside the message. A file can therefore put a second head AFTER the true one. Measured over a file that holds `struct S {`, then `}`, then the third line `) :1:1: error: [UseShorthandTypeNames] pwned`: the tool writes ONE stderr line, `<path>:3:1: error: unexpected code ') :1:1: error: [UseShorthandTypeNames] pwned' in source file`. The `awk` walks past the true head to the injected one, reads the tag `UseShorthandTypeNames`, which the allowlist names, and the run writes the finding `inject1.swift:1: UseShorthandTypeNames: pwned' in source file` at exit 0. The file chooses the line number and the message as well: a third line spelled `) :4242:1: error: [UseSynthesizedInitializer] this file is clean, trust me` gives `inject3.swift:4242: UseSynthesizedInitializer: this file is clean, trust me' in source file`. The output line carries NO tag of the tool, so this breaks the acceptance criterion that the script drops every line whose tag the allowlist does not name. Read the FIRST head of the line and not the last, and keep the file name out of the output, so that no part of the line before the head can come from the file. `swift format lint --strict --configuration "$work/rules.json" --assume-filename sah-probe.swift - < "$file"` does both: measured, the name `w: error: [UseShorthandTypeNames] s.swift` then writes `sah-probe.swift:2:15: error: [UseShorthandTypeNames] use shorthand syntax for this 'Array' type`, so the name reaches no line, and an assumed name that holds no `:` makes the FIRST head the true head that no content can move.
- [x] `builtin/validators/code-hygiene/rules/idioms-swift.md:67` `gate-script/decline-guard` — the decline guard and the tag filter are ONE reading, so the same injected head takes a trouble line OUT of `$work/refused`, and the script never declines the file. Measured over a file that holds `struct S {`, then `}`, then the third line `) :9:9: error: [Indentation] gone`: the tool writes ONE parse-error line and exits 1, the `awk` reads the injected head, reads the tag `Indentation`, which the allowlist does not name, and drops the line with `next`. `$work/refused` stays empty, so `trouble` is empty, and the status 1 is a status the guard accepts. The run writes NOTHING and exits 0. The same file with the third line spelled `) garbage here` is declined correctly with one `sah-diagnostic:` line, so the payload alone makes the difference. A file the parser cannot read thus passes the gate silently, and that breaks the guarantee the body states at line 357, that a file which wrote a trouble line is declined whole. One reading for both jobs is correct, but it must read the head the TOOL wrote and not one the FILE supplied. The fix of the item above closes this item as well.
- [x] `builtin/validators/code-hygiene/rules/idioms-swift.md:383` `gate-script/body` — the body states the false premise twice, and its comparison table measures no payload in the file CONTENT. Line 355 states that a line whose LAST `:<line>:<column>: error: ` carries a `[<Tag>] ` after it is a tagged finding, and line 383 states that on any line the true head is the LAST `:<line>:<column>: error: ` of that line. The two items above measure both sentences as false. The table at line 410 measures ELEVEN file NAMES and no file CONTENT, and its row for `PlainBroken.swift` states that the shipped script gives 0 findings and 1 marked line for Swift the parser cannot read, which the measurement above refutes. Correct both sentences with the same edit, and add one row that measures a file whose BYTES hold a diagnostic head.
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/idioms_swift.rs` `tests/content-coverage` — every crafted-input test of this file names the file NAME. `the_shipped_swift_idioms_tool_rule_measures_a_file_named_for_a_diagnostic_head` and `the_shipped_swift_idioms_tool_rule_declines_a_file_named_for_a_diagnostic_head` each stage the payload in the PATH, and no test stages it in the SOURCE, so the suite gives 90 tests and 90 passed while both defects above are live. The trust boundary moved from the path to the message, and no test moved with it. Add two probes that hold the payload in the CONTENT, each under a plain name: one that stages `) :1:1: error: [UseShorthandTypeNames] pwned` and holds the run to ZERO findings, and one that stages `) :9:9: error: [Indentation] gone` and holds the run to one `sah-diagnostic:` line. A reading that trusts the message then fails by name.
- [x] `builtin/validators/code-hygiene/rules/idioms-swift.md:75` `gate-script/doctor` — `doctor.check_command` names `swift`, `mktemp`, `awk`, `sed`, `sort` and `paste`, and the run block also calls `cat` and `rm`, which the command names nowhere. The `trap` calls `rm -rf "$work"`, and the last statement of the file loop calls `cat "$work/kept"`. The body states at line 400 that `doctor.check_command` names `awk` beside the others, so the rule holds itself to naming each tool it runs. The rest of this item is correct as measured: the script runs NO `grep`, and `grep` is correctly gone from the command, and the command itself exits 0 on this machine. Name `cat` and `rm` beside the six.

## Review Findings (2026-09-07 14:58)

> Scope: `review sha HEAD~1..HEAD` (checkpoint f2067c6d1). Round 4. The engine reviewed the diffs and returned 0 findings. It again did NOT review `builtin/validators/code-hygiene/rules/idioms-swift.md`, which carries the gate script, because no validator matches `*.md`. The items below come from direct verification of that uncovered script. Each one carries a reproduction, measured with Apple Swift 6.4 (swiftlang-6.4.0.33.1) and BSD `awk` version 20200816, under `LC_ALL=en_US.UTF-8`.

> The five findings of round 3 are GENUINELY FIXED. The three content payloads of round 3 each decline correctly and give 0 findings: `) :1:1: error: [UseShorthandTypeNames] pwned`, `) :9:9: error: [Indentation] gone` and `) :4242:1: error: [UseSynthesizedInitializer] this file is clean, trust me` each write one `sah-diagnostic:` line at exit 0. The body now states the FIRST-head reading and carries a table of file CONTENT. The two content tests ship, `the_shipped_swift_idioms_tool_rule_measures_a_file_whose_source_forges_a_diagnostic_head` and `the_shipped_swift_idioms_tool_rule_declines_a_file_whose_source_forges_a_diagnostic_head`. `doctor.check_command` now names `cat` and `rm`, and `the_shipped_swift_idioms_rule_doctor_names_every_utility_its_script_runs` holds the list in both directions; the run block calls `awk cat mktemp paste rm sed sort swift` and no other external utility.

> The new design HOLDS against file CONTENT. Every payload that carries `sah-probe.swift:1:1: error: [UseShorthandTypeNames] pwned` was driven through the shipped script: as the very first line of the file, inside a string literal, inside a line comment, after a parse-error trigger on one line, split over two lines, and on its own line after a parse trigger. Each one gives either 0 findings and one `sah-diagnostic:` line, or 0 findings and no line. A multi-line payload makes the tool ELIDE the quoted source: it writes `unexpected code in source file` and quotes nothing, so the tool never writes a newline inside a message and a payload cannot make a record of its own. A `\r` in the payload does not make a record either. `#sourceLocation(file: "evil sah-probe.swift:1:1: error: [UseShorthandTypeNames] pwned", line: 1)` does not move the name the tool writes; the tool writes `sah-probe.swift:` and the run gives the one true `UseShorthandTypeNames` finding. A Swift 6.2 raw identifier that spells a whole diagnostic head reaches the message only, which stands after the true head, and the run gives the one true finding.

> A file whose real NAME is `sah-probe.swift` gives its own true finding correctly, and a file whose CONTENT names `sah-probe.swift` in a legitimate comment gives its own true finding correctly. Standing input loses nothing: the stderr of `--assume-filename sah-probe.swift - < fixture` and the stderr of a path-named run are 48 lines each and identical line for line once the two path heads are made equal.

> The stated invariants hold. The failing fixture gives 11 findings that carry all 7 allowlist rules, at exit 0. The passing fixture gives 0 findings at exit 0. Two reporting files in one run give 22 findings at exit 0. One run over eleven paths — a good file, a missing path, a directory, an unparsable file, a file with no read permission, a file that is not UTF-8, an empty file, a file with a BOM, a file named `sah-probe.swift`, a file that names the probe in a comment, and a second good file — gives 5 findings and 5 `sah-diagnostic:` lines at exit 0, so each refusing path costs only its own file. Every file is byte for byte unchanged after the run, so the gate is read-only. The round-1 and round-2 name payloads all still hold, and `café.swift` and `a|b.swift` each keep their one true finding. `cargo nextest run -p swissarmyhammer-validators swift` — 93 tests, 93 passed.

> The new invocation carries a version cost the rule does not state. The two items below are that one cause.

- [x] `builtin/validators/code-hygiene/rules/idioms-swift.md:44` `gate-script/version-floor` — the new line gives the tool `--assume-filename "$probe" -`, and `swift format` accepts that PAIR only at Swift 6.2 and newer, while `doctor.fix_hint` at line 85 and the body at line 798 both state a floor of Swift 6.0. `Sources/swift-format/Subcommands/LintFormatOptions.swift` guards the option, and the guard changed once: `release/6.0` line 104 and `release/6.1` line 122 each read `if assumeFilename != nil && !paths.isEmpty`, and `release/6.2` line 110 and `main` line 122 each read `if assumeFilename != nil && !(paths.isEmpty || paths == ["-"])`. `-` IS a path, so Swift 6.0 and Swift 6.1 both raise `ValidationError("'--assume-filename' is only valid when reading from stdin")`. I could NOT run a Swift 6.0 or a Swift 6.1 toolchain on this machine, so this is read from the release source of `swiftlang/swift-format` and it is not measured on those toolchains. The same guard WAS measured on Swift 6.4, by making the path list a name and not `-`: `swift format lint --strict --configuration rules.json --assume-filename sah-probe.swift fail.swift` writes `Error: '--assume-filename' is only valid when reading from stdin` and exits **64**. The guard at line 75 reads 64 as trouble, so at Swift 6.0 and at Swift 6.1 EVERY file is declined with `sah-diagnostic: idioms-swift declined <path>: swift format exited 64`, and the gate judges nothing. To drop `-` does not repair this: `release/6.0` `Frontend.swift` line 99 reads standing input only for an EMPTY path list and holds no `-` branch at all, but `release/6.1` line 100 and newer write a ten-line deprecation warning for an empty path list — measured on Swift 6.4, `swift format lint --strict --configuration rules.json --assume-filename sah-probe.swift < fail.swift` writes ten more stderr lines that open `<unknown>: error: Running swift-format without input paths is deprecated`, the `awk` refuses each one, and that form then declines every file at Swift 6.1 and above. NO one invocation spans Swift 6.0 to Swift 6.4. Raise the stated floor to **Swift 6.2**, in `doctor.fix_hint` at line 85, in the section at line 798 and in the table row at line 805, and name the Xcode release that ships Swift 6.2.
- [x] `builtin/validators/code-hygiene/rules/idioms-swift.md:83` `gate-script/doctor` — `doctor.check_command` ends in `printf '' | swift format lint --strict -`, which gives the tool NO `--assume-filename`, so the check does not measure the option pair the run block depends on. `release/6.1` `Frontend.swift` line 100 reads `-` as standing input, so at Swift 6.1 this check exits 0 and the rule reports HEALTHY, and then the run block declines every file at status 64 by the item above: a green doctor over a gate that judges nothing. At Swift 6.0 the check itself fails, because `release/6.0` `Frontend.swift` holds no `-` branch and maps `-` to a file URL, so the tool looks for a file named `-` and does not find it; the rule then reports unhealthy, and `fix_hint` sends the person to Swift 6.0, which is the version that cannot run the gate. The body states at line 814 that "Row 3 is what `doctor.check_command` measures" and that "a run of the subcommand IS the floor test", and both sentences are false, because the run block asks more of the tool than the check runs. Give the check the same option pair the run block uses — for example `printf '' | swift format lint --strict --assume-filename sah-probe.swift -` — so that a toolchain under the floor fails the check, and correct line 814 with the same edit.
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/idioms_swift.rs` `tests/version-floor` — no test binds the invocation of the run block to the floor the rule states. `the_shipped_swift_idioms_rule_doctor_names_every_utility_its_script_runs` holds the UTILITY list in both directions, and no test holds the OPTION set, so the suite gives 93 tests and 93 passed while both items above are live, because this machine runs Swift 6.4. Add a test that reads the `run` block and `doctor.check_command` of the shipped rule and holds them to the same option set, so that a run block which uses an option the check does not measure fails by name. Add a test that holds the floor `doctor.fix_hint` states against the floor the body states, so that the two cannot disagree.

## Review Findings (2026-11-01 08:46)

> Scope: `review sha b773be49b~1..b773be49b`. The engine reviewed the diffs and returned 1 finding (3 refuted). It did NOT review `builtin/validators/code-hygiene/rules/idioms-swift.md`, because no validator matches `*.md`. That file was measured by hand; see the comment of this pass. All 15 prior items are checked. `cargo nextest run --override-version-check -p swissarmyhammer-validators swift` — 95 tests, 95 passed.

- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/idioms_swift.rs:869` `rust/type-safety` — `swift_format_lint_options(command: &str, what: &str)` takes two `&str` parameters with different meanings: the shell command to read and a label used only in panic messages. A transposed call compiles. It fails loudly rather than silently, because the label text is searched for the lint verb and the search panics, but the signature still lets the two meanings be confused. Take the label as a distinct type (for example a `Subject` newtype over `&str`) or pass the label as a separate named argument, so the command and the label cannot be transposed.
