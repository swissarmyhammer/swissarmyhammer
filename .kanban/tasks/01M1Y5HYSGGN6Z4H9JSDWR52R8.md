---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m6c0cjan9ccabx35azqpmbp7
  text: |-
    ### implement — changed
    - evidence: files changed: builtin/validators/code-hygiene/rules/missing-docs-swift.md (script now runs `swift format lint --strict` with AllPublicDeclarationsHaveDocumentation as the only tag kept; doctor tests toolchain, floor Swift 6.2; body records measurements and the Swift 5.6 first-ship), builtin/validators/code-hygiene/rules/missing-docs.md (Swift getter and obvious-implementation facts re-measured with swift format, no swiftlint fact left), crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs (Swift section rewritten: 14 behavior tests; 4 were RED on the old script), crates/swissarmyhammer-validators/src/review/tool_rules/tests.rs (doc comment of SWIFT_MISSING_DOCS_RULE). Fixture templates unchanged: pass = 0 findings (raw run draws 2 [Indentation]), fail = 3 findings.
    - tests: `cargo nextest run --override-version-check -p swissarmyhammer-validators --no-fail-fast` = 836 run, 836 passed, 0 failed, 0 skipped, 0 warnings.
    - discoveries: the toolchain rule reads `public` only. `open` declarations are NOT reported (header says "public or open", code reads .public). `public override` is skipped. Inherited types and getters are reported (swiftlint skipped them). Project excludes list is gone; `// swift-format-ignore-file` is the generated-code exemption.
    - needs sibling work: tests/shipped.rs doc comments still say swiftlint holds `missing-docs-swift` (comment on SwiftProjectDeclineProbe, and the ORDER note in ShippedEmptyRun); builtin/validators/swift/VALIDATOR.md (line naming missing-docs-swift beside swiftlint gates) and code-hygiene VALIDATOR.md may still list it as swiftlint. Not touched, per scope guard.
    - next: /review
  timestamp: 2026-11-01T16:51:51.253365+00:00
- actor: claude-code
  id: 01m6c0j29q5vx892gmqagdr4pa
  text: |-
    ### test — green
    - evidence: cargo build --workspace --all-targets: exit 0, 0 warnings. cargo nextest run -p swissarmyhammer-validators --no-fail-fast: 836 run, 836 passed, 0 failed, 0 skipped. All 14 Swift missing-docs behavior tests ran and passed. Swift 6.4.2 (swiftlang-6.4.2.1.9), swift format 6.4.4.
    - next: review
  timestamp: 2026-11-01T16:54:51.447652+00:00
- actor: claude-code
  id: 01m6c0k2frhq2d4ey1tsc76wqp
  text: |-
    ### commit — changed
    - evidence: 7a89b57b4 feat(validators): run missing-docs-swift on the toolchain swift format. Staged: builtin/validators/code-hygiene/rules/missing-docs-swift.md, builtin/validators/code-hygiene/rules/missing-docs.md.
    - left out: tool_rules/tests.rs and tests/shipped/missing_docs.rs had no diff against HEAD. They already hold the change.
    - next: review
  timestamp: 2026-11-01T16:55:24.408652+00:00
- actor: claude-code
  id: 01m6c1jv5py0679sas5y03wd2r
  text: |-
    ### review — findings
    - evidence: 3 findings, all in crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs/swift.rs: line 227 test-integrity/test-partitioning, line 349 reuse/reuse, line 513 reuse/reuse. Engine: review sha 7a89b57b4~1..7a89b57b4 = 0 files reviewed (no validator matches missing-docs-swift.md or missing-docs.md; NOT reviewed by the engine). review file on swift.rs = 6 findings, 3 kept, 3 dropped (lines 467, 480, 565 ask to refactor tests that existed before this card). The earlier prompt-cap finding on missing_docs.rs is moot: commit 3490265fb split the file. Swift tests live in commit 59d4aedfb "test: delete tests that assert on skill, validator or source text", moved by 3490265fb "refactor(validators): split missing_docs shipped tests by language".
    - hand-measured (Swift 6.4.2) with the run block of missing-docs-swift.md: fail fixture = 3 findings (lines 10, 11, 14), exit 0; pass fixture = 0, exit 0; raw pass run = 2 [Indentation] lines, filtered out; getter in `public struct Wide: Equatable` and `description` in a conformance extension = reported; `open class` = not reported; `public override` = not reported; unparsable file and missing file = one sah-diagnostic line, exit 0; doctor check_command = exit 0. The script names no swiftlint (only the body line 84 names it as history). missing-docs.md names no swiftlint. Filter and status handling match idioms-swift. Floor Swift 6.2 in fix_hint. All markdown criteria met. No test reads markdown text.
    - next: fix the 3 open findings, then /review
  timestamp: 2026-11-01T17:12:45.494519+00:00
- actor: claude-code
  id: 01m6c1td7r5zgj6jrvtnn8e8xv
  text: |-
    ### implement — changed
    - evidence: crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs/swift.rs; nextest -p swissarmyhammer-validators: 836 run, 836 passed, 0 skipped; 0 warnings.
    - finding 2 fixed: new helper swift_missing_docs_script_findings(staged, judged) holds loader, require_tool_installed, shipped_script_findings and expect. Both swift_missing_docs_pass_fixture_findings and the multi-file test call it.
    - finding 3 fixed: new helper swift_missing_docs_judged_rows() holds the two expected_row calls. swift_missing_docs_decline_probe and the parser-cannot-read test call it.
    - same cause removed elsewhere in the file: the two-path list (probe path, second path) was written twice in the multi-file test. It is now const SWIFT_MISSING_DOCS_TWO_PATHS.
    - finding 1 NOT moved: no separate integration target exists for this crate. There is no tests/ directory, no feature gate, no integration module, and no workspace default-members split. Every real-tool test (Swift, Rust, Go, Dart, Python, TypeScript) lives in the unit target under tests/shipped, and all of them share private helpers from shipped.rs. Convention for real-tool tests here is that location, so the test stays. A new harness would be an invention; a person must decide if one should exist.
    - next: review. Finding 1 needs a human decision on a new integration target.
  timestamp: 2026-11-01T17:16:53.368094+00:00
- actor: claude-code
  id: 01m6c1xrfrm1gna1jb98npd37e
  text: 'released: the user decided the test-partitioning rule misfires here (the crate has no separate integration target); the rule gets fixed in its own card'
  timestamp: 2026-11-01T17:18:43.192366+00:00
- actor: claude-code
  id: 01m6c1xs8gme6213zrvkxrsw6h
  text: |-
    ### commit — changed
    - evidence: 50ab7ac18 test(validators): share the missing-docs-swift test setup and expected rows; file crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs/swift.rs
    - next: review
  timestamp: 2026-11-01T17:18:43.984941+00:00
- actor: claude-code
  id: 01m6c23zndrxwyhxnwh811xse4
  text: |-
    ### review — clean
    - scope: review sha 50ab7ac18~1..50ab7ac18 (task-mode, iteration 2). 1 file reviewed, 0 not reviewed.
    - counts: findings 0, confirmed 0, refuted 0, attempted 7, failed 0, skipped 0.
    - board: all 3 items of the 2026-11-01 11:10 section read [x]. The release note on finding 1 (test-partitioning) is present. Finding 1 was not raised again.
    - fixed by this commit: findings 2 and 3 (helpers swift_missing_docs_script_findings and swift_missing_docs_judged_rows, constant SWIFT_MISSING_DOCS_TWO_PATHS).
    - tests: validators suite 836 passed, 0 skipped (implementer, after cargo fmt).
    - markdown: no markdown file changed in this commit. The engine cannot review *.md. The first review's hand measurement stands.
    - next: task moved to done.
  timestamp: 2026-11-01T17:22:07.149174+00:00
depends_on:
- 01M1Y65VQ8V4TEWXDXD4Y2ZQNX
- 01M1Y5GD85944ACE2XS6SA5XWJ
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffffdf80
title: Move missing-docs-swift to the toolchain swift format
---
## What

`builtin/validators/code-hygiene/rules/missing-docs-swift.md` runs `swiftlint`. The Swift toolchain decides the same bullet with its own rule `AllPublicDeclarationsHaveDocumentation`. Move this rule to `swift format lint`, and remove one more Homebrew dependency.

Files to change:
- `builtin/validators/code-hygiene/rules/missing-docs-swift.md`
- `builtin/validators/code-hygiene/rules/missing-docs.md` — it states swiftlint-measured facts about `missing-docs-swift` that this task makes false
- `builtin/validators/code-hygiene/fixtures/missing-docs-swift.fail.swift.tmpl`
- `builtin/validators/code-hygiene/fixtures/missing-docs-swift.pass.swift.tmpl`
- the matching test file under `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/`

Measured on the Swift 6.4 toolchain, over a public struct with no documentation:

```
Dirty.swift:3:1: error: [AllPublicDeclarationsHaveDocumentation] add a documentation comment for 'Probe'
```

`AllPublicDeclarationsHaveDocumentation` is OFF in the default configuration. The written configuration must make it ON.

### The configuration alone is not the gate

`swift format lint` always writes layout findings — `[Indentation]`, `[Spacing]`, `[LineLength]` and `[AddLines]`. No key of the configuration makes them OFF. **Measured over the SHIPPED passing fixture of this rule**, with a configuration that makes ON only `AllPublicDeclarationsHaveDocumentation`:

```
Pass.swift:9:1: error: [Indentation] unindent by 2 spaces
Pass.swift:10:1: error: [Indentation] unindent by 2 spaces
```

Thus the script must read each output line, take the `[<RuleName>]` tag, and KEEP only `AllPublicDeclarationsHaveDocumentation`. Use the same filter and the same status handling the task "Move idioms-swift to the toolchain swift format" writes. Do that task first.

`swift format lint --strict` exits 1 when it reports. The run must capture the status and must not let `set -e` end the run.

Do NOT make ON `ValidateDocumentationComments` or `BeginDocumentationCommentWithOneLineSummary`. Those two decide the bullets of `builtin/validators/swift/rules/doc-parameter-naming.md`, which is a different requirement and a different owner.

## Acceptance Criteria
- [ ] The script names `swift format` and names `swiftlint` nowhere.
- [ ] The script keeps only the `AllPublicDeclarationsHaveDocumentation` tag, and drops every other tag.
- [ ] `doctor.check_command` tests the toolchain. The rule needs Homebrew no longer.
- [ ] The passing fixture gives 0 findings through the filter, at exit 0. Layout findings from the pretty-printer do not reach the report.
- [ ] The failing fixture gives a STATED number of findings, at exit 0.
- [ ] `missing-docs.md` states no swiftlint fact about `missing-docs-swift`.
- [ ] The rule body records the measurement, and states which Swift version gives the rule.

## Tests
- [ ] Update the acceptance tests of this rule for the new tool and the new message shape.
- [ ] Write a test that holds the shipped passing fixture to 0 findings through the filter. A script that drops the filter fails it by name, because the fixture draws two `[Indentation]` lines.
- [ ] Write a test that holds the configuration and the allowlist to naming only `AllPublicDeclarationsHaveDocumentation`.
- [ ] Run `cargo nextest run -p swissarmyhammer-validators missing_docs`. Every test passes.
- [ ] Run `sah doctor`. The rule reports as healthy, and its fixture pair passes.

## Workflow
- Use `/tdd` — write the failing tests first, then write the code that makes them pass. #swift

## Review Findings (2026-11-01 11:10)

> Scope: `review sha 7a89b57b4~1..7a89b57b4` (two markdown files, no validator matches them, so the engine reviewed 0 files; measured by hand) and `review file` on the Swift tests. The tests were swept into commit 59d4aedfb "test: delete tests that assert on skill, validator or source text", then moved by 3490265fb "refactor(validators): split missing_docs shipped tests by language" to `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs/swift.rs`. Findings on tests that existed before this card (the decline, hollow-directory and only-given-files tests) are dropped by the written rule that drops findings which ask to refactor tests that already existed.

- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs/swift.rs:227` `test-integrity/test-partitioning` — Integration test inside the unit target: runs a project `.swift-format` through the real tool via `swift_gate_reporting_rules`. Move this test to the separate integration target that runs the real Swift toolchain. (released: the user decided the test-partitioning rule misfires here (the crate has no separate integration target); the rule gets fixed in its own card)
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs/swift.rs:349` `reuse/reuse` — The multi-file test repeats the setup of `swift_missing_docs_pass_fixture_findings`: it builds the loader, calls `require_tool_installed` for the same project types and rule, then runs `shipped_script_findings` and calls `.expect`. The shared steps are copied, not reused. Extract a helper that takes the staged files and the expected-success message, such as `swift_missing_docs_script_findings(staged: &[(&str, &str)], judged: &[&str]) -> Vec<String>`. Use it in both `swift_missing_docs_pass_fixture_findings` and `the_shipped_swift_missing_docs_tool_rule_reports_every_file_that_reports`.
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs/swift.rs:513` `reuse/reuse` — The test rebuilds the same `structure` and `property` expected rows that `swift_missing_docs_decline_probe` already builds at lines 436-445. The existing function is a near-match that is not reused. Two copies of the same expected-row logic can drift apart. Call `swift_missing_docs_decline_probe().expected` to get the two rows, or move the two `expected_row` calls into one shared helper that both sites call.
