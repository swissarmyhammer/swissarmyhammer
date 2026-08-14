---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzw4tp4jrz93k2x2qkkhabfy
  text: |-
    Measurement done. swiftlint 0.65.0. Corpus: Alamofire 0455bfb (98 files), swift-nio 08b497c (554), vapor c6818be (242) = 894 `.swift` files, none carrying a `.swiftlint.yml`. Same method as the `ignores_case_statements` measurement: a child configuration naming `only_rules: [closure_body_length]`, one `warning`/`error` level at a time, over every file.

    Threshold sweep, findings over the 894 files:

    | warning | findings |
    |---|---|
    | 20 | 316 |
    | 30 (swiftlint's own default warning) | 148 |
    | 40 | 76 |
    | 50 | 41 |
    | 100 (swiftlint's own default error) | 3 |
    | 150 | 2 |
    | 200 | 2 |
    | 250 | 1 |
    | 300 | 0 |

    At 250 the whole corpus reports ONE closure: `swift-nio/Benchmarks/Benchmarks/NIOCoreBenchmarks/Benchmarks.swift:45`, a `let benchmarks: @Sendable () -> Void = { ... }` registration block of 259 lines. The shipped gate as it stands today reports 3 findings over the same 894 files (2 `cyclomatic_complexity`, 1 `function_body_length`), so the rule adds 1 finding to 3.

    Trailing-closure shapes read first, at the gate of 250:

    | the shape | `closure_body_length` |
    |---|---|
    | SwiftUI `body`, 300 `Text` rows in one `VStack` | reports, at the `VStack {` line |
    | SwiftUI `body`, 200 `Text` rows in one `VStack` | silent |
    | SwiftUI `body`, 3 `VStack` of 100 rows inside a `Group` | reports the outer `Group`, 306 lines |
    | `func testEndToEnd()` holding one `measure { }` of 300 lines | reports, beside `function_body_length` |
    | `let suite: () -> Void = { }` of 300 lines | reports |
    | a computed `var` of 300 straight lines, no closure | silent |

    So the gate does not fire on an idiomatic trailing closure. It fires at 250 lines, which is where `function-length` fires. DECISION: add `closure_body_length: 250` to the child configuration. The number that decided it is 1 finding over 894 files at 250.

    Two consequences to carry into the change:
    - The finding anchors on the closure's own opening line, so `// swiftlint:disable:next closure_body_length` goes directly above `VStack {` and NOT above `var body`. Measured both ways.
    - The existing acceptance test `..._reads_no_computed_property_body` uses a 300-row `VStack` inside a `body`, which now reports. The gap that actually remains is a computed variable whose body holds no closure, so that probe changes to one.
  timestamp: 2026-08-12T23:27:26.354048+00:00
- actor: claude-code
  id: 01kzw5exgdzpn57hat956q5cte
  text: |-
    Implementation landed. The measurement said ADD, so the child configuration now names `closure_body_length` at `warning: 250` / `error: 250`, and the `jq` filter selects its rule id beside the other two.

    TDD order, with the RED watched:
    1. Added `the_shipped_swift_complexity_tool_rule_reports_a_long_trailing_closure`. It failed with `left: []`, `right: ["Sources/Panel.swift:5"]` — the shipped rule reported nothing, and row 5 is the `VStack {` line the measurement named.
    2. Added the rule to the child configuration and to the filter. The new test passed.
    3. `..._reads_no_computed_property_body` then failed, as the research comment predicted: its 300-row `VStack` inside a SwiftUI `body` now reports. Its probe changed to a computed `var` of 300 straight statement lines beside the long function, which is the gap that actually remains. New helper `long_swift_computed_variable`. `SWIFT_VIEW_TAIL_LINES` went dead with the old row constant and was deleted.

    Rule body corrected. "What each gate reaches, and what neither reaches" now carries a third column measured over all ten declaration shapes, and a new section "Why the closure gate stands at 250" states the sweep table, the one corpus finding, and the trailing-closure shape table. Two further measurements that section records:
    - `cyclomatic_complexity` reads no closure either, and swiftlint holds no closure complexity rule, so the closure gate is LENGTH alone. Measured: a closure of cyclomatic complexity 16 in a `let` and the same inside a computed `var` each report nothing. Superseding drops the complexity gate for a closure, and the rule now states that trade.
    - A nested closure aggregates. Three `VStack` of 100 rows inside a `Group` report the outer `Group` at 306.

    Also corrected, because each said "two rules" or counted five options: the head list, "The length gate counts what the prompt rule counts", "Each rule has one gate", "How the run is shaped", "The rule owns its own gates" (five options becomes seven), "The carve-outs the two prompt rules state", and the `complexity-swift` line of `builtin/validators/code-hygiene/VALIDATOR.md`. The annotation section gained the closure directive's placement, measured both ways: above the closure's own opening line silences it, above the `var body` line does not.

    Verification: `cargo nextest run -p swissarmyhammer-validators` 697 passed 0 failed; `cargo nextest run -p mirdan` 503 passed 0 failed; `cargo fmt --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean.

    ### implement — changed
    - evidence: 3 files — builtin/validators/code-hygiene/rules/complexity-swift.md, builtin/validators/code-hygiene/VALIDATOR.md, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/complexity_swift.rs. 697 validators tests pass, 503 mirdan tests pass, fmt and clippy clean.
    - next: /review
  timestamp: 2026-08-12T23:38:29.261120+00:00
- actor: claude-code
  id: 01kzw5pcj95rbc9jfrwnj14p6k
  text: |-
    ### review — clean
    - evidence: `review sha d8f813b84~1..d8f813b84` — 0 findings, 0 confirmed, 0 refuted, 8 validators attempted, 0 failed, 0 skipped. No prior Review Findings section holds an unchecked item. No finding was dropped for a false premise, because the engine reported none.
    - next: none — task moves to done.
  timestamp: 2026-08-12T23:42:34.057170+00:00
- actor: claude-code
  id: 01kzw5q2h4c0r3tewvxw5s8e7e
  text: |-
    ### finish iteration 1 — clean
    - implement: changed — 3 files. The measurement said ADD, and the number that decided it is 1 finding over 894 files at a gate of 250. Corpus is the one the ignores_case_statements measurement used: Alamofire 0455bfb (98 files), swift-nio 08b497c (554), vapor c6818be (242), none carrying a .swiftlint.yml. Sweep: 20 → 316 findings, 30 (swiftlint's own default) → 148, 50 → 41, 100 → 3, 250 → 1, 300 → 0. The one finding is a 259-line @Sendable registration block at swift-nio Benchmarks/.../Benchmarks.swift:45.
    - The trade the card warned about does not materialise at 250. A SwiftUI body of 200 Text rows in a VStack is silent, and reports at 300, which is where function-length fires. At swiftlint's default of 30 it would materialise: 148 findings.
    - Two measurements that came out of the work and are now stated in the rule rather than left to be re-found: `cyclomatic_complexity` reads no closure either and swiftlint ships no closure-complexity rule, so superseding drops the COMPLEXITY gate for a closure — length is all the closure gate gives. And a nested closure aggregates: three VStack of 100 rows inside a Group report the outer Group at 306 lines.
    - The existing test `..._reads_no_computed_property_body` broke once the rule shipped, because its probe's 300-row VStack now reports. Its probe moved to a computed var of 300 straight statement lines, which is the gap that genuinely remains.
    - test: green — 697 validators tests, 503 mirdan tests, fmt and clippy clean. RED watched first: `left: []`, `right: ["Sources/Panel.swift:5"]`.
    - commit: d8f813b84
    - review: clean — 0 findings over d8f813b84~1..d8f813b84, 8 validators attempted, 0 failed. Task moved to done.
  timestamp: 2026-08-12T23:42:56.548512+00:00
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffffffff580
title: complexity-swift drops the closure, which function-length measures
---
`function-length` states "All Function Types: Methods, closures, lambdas, standalone functions". `complexity-swift` supersedes that prompt rule, and its child configuration names `cyclomatic_complexity` and `function_body_length` alone.

Measured with swiftlint 0.65.0 over one closure of 300 body lines held in a `let`: the run reports nothing. The same 300 lines in a `func` report `Function body should span 250 lines or less`.

swiftlint holds `closure_body_length` for a closure. It is an opt-in rule, and its default gate is `warning: 20` and `error: 100`, which is not the 250 of the `function-length` prompt gate.

Measure `closure_body_length` at 250 over a body of real Swift — Alamofire, swift-nio and vapor are the corpus the `ignores_case_statements` measurement of `complexity-swift` used. Then decide from the measurement whether the child configuration names the rule. A trailing closure of a SwiftUI `body` or of a test builder is the shape to read first: a rule that reports every long trailing closure makes a suppression mandatory on code the prompt rule calls correct, which is the trade `complexity-swift` refuses elsewhere.

The rule body states the gap today, under "What each gate reaches, and what neither reaches". Correct that section with whatever the measurement decides.

Found while measuring the carve-outs on ^h2ezbs7. #tool-validators #objectivity