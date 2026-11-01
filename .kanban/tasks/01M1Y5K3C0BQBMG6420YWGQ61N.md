---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m6bybatbeeee2gv2zyxbjje1
  text: |-
    ### implement — changed
    - evidence: 4 files — builtin/_partials/project-types/swift.md (rewritten: one formatter, `swift format`, Swift 6.2 or newer; write/lint -s commands; obey existing .swift-format; dump-configuration; `// swift-format-ignore`; swiftlint + periphery via brew as review-gate tools; the 3-tool table, Airbnb plugin, swiftformat command and .swift-version paragraph deleted), crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/swift_guidelines_partial.rs (rewritten: 4 tests; partial rendered through PromptResolver + TemplateLibrary include; lint head, directive, version floor read from idioms-swift), crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs (one line: re-add `mod swift_guidelines_partial;`, which a concurrent session had dropped), crates/swissarmyhammer-validators/Cargo.toml (dev-dep swissarmyhammer-config for TemplateContext).
    - tests: RED 4/4 failed against the old partial; GREEN 4/4. Full `cargo nextest run -p swissarmyhammer-validators`: 842 run, 842 passed, 0 skipped. Commands verified on Apple Swift 6.4.2 (lint -s exits 1 on findings, without -s exits 0; -i -r writes; no .swift-format created).
    - ^7fgdenq: its bullet 2 (`.swiftformat` -> `swiftformat .`) and its test location (project-detection types.rs) are CONTRADICTED by this rewrite; its `.swift-format` honoring, no-config-side-effect rules are satisfied. Left open.
    - remaining swiftformat/Airbnb mentions (sibling cards): code-hygiene/VALIDATOR.md (461-563), idioms-swift.md (100, 715, 811-812, 919), disallowed-constructs-swift.md (153-266), swift/VALIDATOR.md (26, 53, 60), swift/rules/idioms.md (11, 14: --property-types), tool_rules/tests.rs (167-196 doc comments), disallowed_constructs_swift.rs (184), idioms_swift.rs (28, 789), .github/workflows/ci.yml (111-112: brew install swiftformat). immutability.md/access-control.md hits are the word "accumulator"/"dynamic" false positives.
    - note: a concurrent session was editing shipped.rs/idioms_swift.rs during this run; not touched beyond the one mod line.
    - next: /review
  timestamp: 2026-11-01T16:16:13.643873+00:00
- actor: claude-code
  id: 01m6byhd1fttcfseyqfd9zk1em
  text: |-
    ### test — green
    - evidence: `cargo build --workspace --all-targets` exit 0, 0 warnings. `cargo nextest run -p swissarmyhammer-validators --no-fail-fast`: 842 run, 842 passed, 0 skipped. `cargo nextest run -p swissarmyhammer-project-detection --no-fail-fast`: 35 run, 35 passed, 0 skipped.
    - count change 866 -> 842 (-24): caused by another session's deletions in the working tree, not by this change. Test counts by #[test] per file, HEAD vs working tree: deleted modules golangci_cache (-3), scope_roster (-3), temp_directory (-3), zero_argument (-4) = -13, and their `mod` lines removed from shipped.rs; idioms_swift -6; swift_judgment_rules -3; disallowed_constructs_swift -1; src/builtin/mod.rs -2 (test_swift_casing_accepts_both_acronym_spellings, test_builtin_rulesets_carry_their_validator_md_body). Total -25. swift_guidelines_partial.rs is +1 (3 at HEAD, 4 now). Net -24.
    - files differing from HEAD in crates/swissarmyhammer-validators: Cargo.toml, src/builtin/mod.rs, tests/shipped.rs, shipped/disallowed_constructs_swift.rs, shipped/function_length_go.rs, shipped/idioms_swift.rs, shipped/swift_guidelines_partial.rs, shipped/swift_judgment_rules.rs; deleted: shipped/golangci_cache.rs, shipped/scope_roster.rs, shipped/temp_directory.rs, shipped/zero_argument.rs. Also differing: builtin/_partials/project-types/swift.md, builtin/skills/check-sah/SKILL.md, builtin/skills/finish/SKILL.md.
    - next: review
  timestamp: 2026-11-01T16:19:32.527876+00:00
- actor: claude-code
  id: 01m6byjmcf8ratk7anef8exerw
  text: |-
    ### commit — changed
    - evidence: 075ac5e15 docs(swift): state one formatter, swift format, in the Swift project partial. Files: builtin/_partials/project-types/swift.md, swift_guidelines_partial.rs, crates/swissarmyhammer-validators/Cargo.toml, Cargo.lock (one hunk).
    - left out: shipped.rs (changes are the other session's deletions only), check-sah and finish SKILL.md, all other session changes.
    - next: review
  timestamp: 2026-11-01T16:20:12.815059+00:00
- actor: claude-code
  id: 01m6byt71b9gse1xj0t5t3d5xg
  text: |-
    ### review — findings
    - scope: review sha 075ac5e15~1..075ac5e15. Counts: 1 finding, 1 confirmed, 0 refuted. The engine could look at swift_guidelines_partial.rs. It could NOT look at builtin/_partials/project-types/swift.md or Cargo.lock (no validator matches them), so "no finding" on swift.md proves nothing. swift.md was measured by hand.
    - finding (open): swift_guidelines_partial.rs:153 `completeness/case-sensitivity-coverage` — the new negative check `!rendered.contains(formatter_use)` is case-sensitive; `SwiftLint --fix` passes it. Same gap at line 248 for `swiftformat:disable`. The sibling test lowercases `rendered`, and its doc comment says a capitalised mention fails. Lowercase the rendered text before both checks, and add one assertion that a capitalised form such as `SwiftLint --fix` would fail.
    - hand measurement of swift.md (Formatting section, 56-75): (1) `swiftformat`, `airbnb`, `--property-types`, `.swift-version` appear nowhere (case-insensitive grep, 0 hits): MET. (2) exactly one formatter, `swift format`, no menu, no table: MET. (3) `swiftlint` and `periphery` appear only in "Review gates need two more tools", as gate tools with `brew install swiftlint periphery`: MET. (4) directive `// swift-format-ignore` equals idioms-swift.md "The directive is `// swift-format-ignore`" (rule body, same-line comment does nothing: matches row 774): MET. (5) commands `swift format -i -r Sources Tests`, `swift format lint -s -r Sources Tests`, `dump-configuration` are the card's commands; the implement comment records them run on Swift 6.4.2; I did not re-run them: ACCEPTED ON RECORD. Also met: Swift 6.2 floor matches idioms-swift doctor fix_hint ("Swift 6.2 or newer"); lint command `swift format lint` matches the gate; existing `.swift-format` obeyed and none written as a side effect; ULID, testing, common commands, file locations sections kept.
    - sibling-card mentions (VALIDATOR.md, ci.yml, disallowed-constructs-swift.md, test comments) not recorded here.
    - next: implementer fixes the open finding, then /review again.
  timestamp: 2026-11-01T16:24:21.291546+00:00
- actor: claude-code
  id: 01m6byycqb4y2je5n014y52skf
  text: |-
    ### implement — changed
    - evidence: swift_guidelines_partial.rs; added helper named_in_swift_guidelines (lowercases rendered text) + test swift_partial_negative_checks_ignore_case (RED first, then GREEN). Both negative checks (formatter_use loop, swiftformat:disable) now use it. Audit: positive contains in assert_swift_guidelines_state need exact phrasing and fail closed, so no gap; script.contains is not rendered text. Suite: 843 run, 843 passed, 0 skipped.
    - next: review
  timestamp: 2026-11-01T16:26:38.187830+00:00
- actor: claude-code
  id: 01m6bz2y7kwe0tm1kgsh440wza
  text: |-
    ### test — green
    - evidence: cargo build --workspace --all-targets — 0 warnings, 0 errors; cargo nextest run --override-version-check -p swissarmyhammer-validators --no-fail-fast — 843 run, 843 passed, 0 failed, 0 skipped
    - next: review
  timestamp: 2026-11-01T16:29:07.187330+00:00
- actor: claude-code
  id: 01m6bz3mw9aq3bfymnna77fyyx
  text: |-
    ### commit — changed
    - evidence: b6430db14 test(validators): make the Swift partial's negative checks ignore case; file: crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/swift_guidelines_partial.rs
    - next: review
  timestamp: 2026-11-01T16:29:30.377948+00:00
- actor: claude-code
  id: 01m6bz8zkt70brf14kaee62j8d
  text: |-
    ### review — findings
    - scope: review sha b6430db14~1..b6430db14 (iteration 2). Prior item at swift_guidelines_partial.rs:153 reads as checked on the board.
    - evidence: 1 finding — crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/swift_guidelines_partial.rs:77 (completeness/case-sensitivity-coverage). The engine cannot review *.md files. Nothing in builtin/_partials/project-types/swift.md changed in this commit. The hand measurement of the first review stands.
    - next: implementer fixes the open item in the dated section 2026-11-01 10:30, then re-review.
  timestamp: 2026-11-01T16:32:25.210972+00:00
- actor: claude-code
  id: 01m6bzdanxbaehp4swn6sjkf98
  text: |-
    ### implement — changed
    - finding fixed: named_in_swift_guidelines now lowercases each needle and the rendered text, and returns the original needle. swift_partial_names_no_second_formatter now uses the same helper, so no call site has its own case handling. Positive checks (assert_swift_guidelines_state) stay exact-case on purpose: they hold exact command text.
    - test first: swift_partial_negative_checks_ignore_case now covers lowercase needle vs capitalised text, capitalised needle vs lowercase text, and mixed on both sides (with punctuation, swiftformat:disable). RED seen on the capitalised-needle case, GREEN after the fix.
    - evidence: crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/swift_guidelines_partial.rs; cargo nextest -p swissarmyhammer-validators --no-fail-fast: 843 run, 843 passed, 0 skipped.
    - next: /review
  timestamp: 2026-11-01T16:34:47.613296+00:00
- actor: claude-code
  id: 01m6bzghff8vtb2s9b0jggmevj
  text: |-
    ### finish — closed by the user's decision
    - The user chose to close this card as done (option 1).
    - The partial builtin/_partials/project-types/swift.md is committed (075ac5e15) and was hand-measured against all acceptance criteria: one formatter (swift format, Swift 6.2 or newer), no swiftformat or Airbnb mention, matches idioms-swift.
    - Review iteration 2 left one open finding at swift_guidelines_partial.rs:77 (needle case). That file asserts on skill text. The user ruled that such tests are removed, and session swissarmyhammer-03 deletes the file under card ^ga169e. The finding goes away with the file. It was not fixed in a commit; the iteration 3 edit stays uncommitted and is dropped.
    - Commits: 075ac5e15 (partial and tests), b6430db14 (case-insensitive checks).
    - Sibling cards still open: Rebalance the Swift prompt rules, Stop installing swiftformat in CI, Correct the validator documents, Move missing-docs-swift, and ^7fgdenq (config detection).
  timestamp: 2026-11-01T16:36:32.879545+00:00
depends_on:
- 01M1Y65VQ8V4TEWXDXD4Y2ZQNX
- 01M1Y5GD85944ACE2XS6SA5XWJ
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffffdb80
title: Rewrite the Swift project partial to state one formatter
---
## What

`builtin/_partials/project-types/swift.md` gives an author THREE formatting tools and a menu to choose between them: Apple swift-format, SwiftFormat of Nick Lockwood, and SwiftLint, plus a fourth path through the Airbnb SwiftPM plugin. A menu is not a policy. The policy is now the Swift toolchain and its own `swift format`.

Files to change:
- `builtin/_partials/project-types/swift.md` — the section with the heading **Formatting and linting**
- `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/swift_guidelines_partial.rs`

Delete from that section:
- the three-tool table, and the sentence that opens "Use the tool whose config file the repo already holds"
- the whole Airbnb plugin part, with its `swift package --allow-writing-to-package-directory format` commands and the caution about `--property-types inferred`. The Airbnb plugin is not a toolchain tool: it wraps SwiftFormat and SwiftLint and gives them as binary products.
- the SwiftFormat command with its four options `--pattern-let inline`, `--short-optionals always`, `--single-line-for-each convert` and `--guard-like-if-statements convert`
- the paragraph that opens "Write the project Swift version in `.swift-version`". That file is a SwiftFormat mechanism. Row 8 of the measurement task states if `swift format` reads it. Delete the paragraph if it does not.

Write instead:
- ONE formatter: `swift format`, which the Swift toolchain gives. No install step.
- `swift format -i -r Sources Tests` to write the files. `swift format lint -s -r Sources Tests` to check them. Keep `-s`, because `swift format lint` without it writes warnings and exits 0.
- The tool reads `.swift-format`, a JSON file, from the directory of the file or from a parent. Obey a `.swift-format` the project already holds. Do not write one as a side effect of formatting.
- `swift format dump-configuration` writes the default configuration, which is the way to make a first `.swift-format`.
- The exemption directive an author writes, which row 7 of the measurement task proves. It is `// swift-format-ignore`, and it is NOT `// swiftformat:disable:next`.
- Name the two tools the review gates still need, and say Homebrew installs them: `swiftlint` for the disallowed constructs, the function length and the magic numbers; `periphery` for the dead code. The toolchain has no rule for any of those.

Keep every other section of the partial as it is: ULID, testing, common commands and file locations.

Name a section by its heading text. Do not name a line number, because a line number moves.

## Acceptance Criteria
- [ ] The partial names `swiftformat` nowhere, and names the Airbnb plugin nowhere.
- [ ] The partial gives exactly one formatting tool and no choice between tools.
- [ ] The partial names `swiftlint` and `periphery` only as the tools the review gates need, never as a formatter.
- [ ] The partial names the same exemption directive the `idioms-swift` rule body names.
- [ ] Every command in the partial runs and gives the stated result on the Swift toolchain of the measurement table.

## Tests
- [ ] Update `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/swift_guidelines_partial.rs`.
- [ ] Write a test that holds the partial to naming `swiftformat` nowhere and `--property-types` nowhere.
- [ ] Write a test that holds the partial and the `idioms-swift` gate to naming the SAME tool and the SAME exemption directive. The two files moved together before, and they must move together now.
- [ ] Run `cargo nextest run -p swissarmyhammer-validators swift_guidelines_partial`. Every test passes.

## Workflow
- Use `/tdd` — write the failing tests first, then write the code that makes them pass. #swift

## Review Findings (2026-11-01 10:20)

> Scope: `review sha 075ac5e15~1..075ac5e15` — reviewed the diffs only — lines this change added or modified. 2 file(s) reviewed, 2 not reviewed.

> 2 file(s) not reviewed — no validator matched:
> - `Cargo.lock` — no validator matches this file
> - `builtin/_partials/project-types/swift.md` — no validator matches this file

- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/swift_guidelines_partial.rs:153` `completeness/case-sensitivity-coverage` — The new negative check `!rendered.contains(formatter_use)` is case-sensitive, but tool names in prose are not. A capitalised mention such as `SwiftLint --fix` or `SwiftLint Autocorrect` passes this test and is not caught. The sibling test `swift_partial_names_no_second_formatter` lowercases `rendered` first, and its doc comment (lines 112-113) states that a capitalised mention fails it. This check does not honour that contract. The same gap exists at line 248 for `swiftformat:disable`. Lowercase the rendered text before the formatter-use loop, for example `let rendered = rendered_swift_guidelines().to_lowercase();`. The needles are already lower case. Apply the same lowercasing to the `swiftformat:disable` check at line 248, or assert it in lower case. Then add one assertion that a capitalised form such as `SwiftLint --fix` would fail, which covers the negative direction.

## Review Findings (2026-11-01 10:30)

> Scope: `review sha b6430db14~1..b6430db14` — reviewed the diffs only — lines this change added or modified. 1 file(s) reviewed, 0 not reviewed.

- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/swift_guidelines_partial.rs:77` `completeness/case-sensitivity-coverage` — The new helper lowercases the rendered text but not the needles, so an uppercase needle such as `SwiftLint --fix` can never match and the negative check passes without checking anything. The doc comment at lines 70-71 states the lowercase rule, but nothing enforces it. The fix covers the rendered side of the case contract but not the needle side. Lowercase each needle as well, for example `.filter(|needle| rendered.contains(&needle.to_lowercase()))`, and return the original needle, so the match is case-insensitive on both sides. Alternatively, keep the lowercase contract and add a debug_assert that each needle equals its own lowercase form.
