---
assignees:
- claude-code
position_column: todo
position_ordinal: fff280
project: swift-validator
title: 'swift: give the SwiftUI private-state half a tool owner (privateStateVariables / private_swiftui_state)'
---
`builtin/validators/swift/rules/access-control.md` states one bullet with two halves:

> **A SwiftUI view keeps its input properties `internal` and its dynamic properties `private`.**

The SECOND half is tool-decidable, and `builtin/validators/swift/VALIDATOR.md` states the standing order: **if a tool can decide it, the tool owns it.** Two tools already decide it, and neither stands in a shipped roster.

## The measurement

Measured over one probe holding `@State var counter: Int = 0` and `@StateObject var model: Model = Model()` beside the same two declarations written `private`:

| the tool | the run | reported |
|---|---|---|
| swiftformat 0.62.1 | `--rules privateStateVariables` | both non-private lines; silent on the private forms |
| swiftlint 0.65.0 | `only_rules: [private_swiftui_state]` | both non-private lines, `SwiftUI state properties should be private`; silent on the private forms |

swiftformat's message: `Adds private access control to @State properties without existing access control modifiers.`

## The work

- Add `privateStateVariables` to the roster of `builtin/validators/code-hygiene/rules/idioms-swift.md`. The roster is a `printf` argument list, and `the_shipped_swift_idioms_tool_rule_names_only_rules_swiftformat_knows` reads the names off the shipped script — the roster size constant `SWIFT_IDIOMS_ROSTER_SIZE` moves from 29 to 30.
- The rule AUTOCORRECTS. Read what the fix writes before deleting anything: measured, it writes `@State private var counter: Int = 0`. That is the DO the prompt bullet already states, so the two agree.
- Add the declaration to the `idioms-swift` fixture pair — `builtin/validators/code-hygiene/fixtures/idioms-swift.fail.swift.tmpl` holds the non-private form, `.pass.swift.tmpl` the private one — and move the finding counts the rule body records (**42 findings**, 21 of 27 enabled rules).
- Take the dynamic half OUT of the `access-control.md` bullet, leaving the memberwise-init half whole, and record the deletion in the `idioms-swift` rule body table beside the six bullets already there.
- Add the row to `SupersededSwiftBullet` in `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs`, so `the_shipped_swift_idioms_tool_rule_owns_each_bullet_it_took` holds BOTH halves: the tool reports the shape, and no prompt rule states it any more.
- Drop the `@State var isExpanded = false` form and its probe from `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/swift_judgment_rules.rs`, which currently holds every shipped gate to reporting NOTHING over that shape. That test WILL fail the moment the roster takes the rule, and that is the point — it fails loudly rather than letting two owners stand.
- Update `builtin/validators/swift/VALIDATOR.md`, which states in as many words that this half waits on a roster.

`private_swiftui_state` is the swiftlint alternative if `disallowed-constructs-swift` is the better home. Only ONE of the two may be enabled — enabling both puts two tools on one requirement. #tool-validators