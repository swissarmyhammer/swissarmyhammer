---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m6c0c7bjn15fg5wq58wsebwm
  text: |-
    ### implement — changed (partial: two parts need a person)
    - evidence: files builtin/validators/swift/rules/idioms.md, value-semantics.md; builtin/validators/code-hygiene/rules/idioms-swift.md (body only); crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/swift_judgment_rules.rs, idioms_swift.rs; src/builtin/mod.rs (comment only). cargo nextest -p swissarmyhammer-validators: 836 run, 836 passed, 0 skipped; cargo fmt --check clean.
    - done: final bullet back in value-semantics.md. idioms.md: Void and Array/Dictionary empty-literal bullets and the propertyTypes note removed; Set literal bullet kept (measured: AlwaysUseLiteralForEmptyCollectionInit is silent for Set<String>()); type-name bullet kept; where bullet kept for the print-loop shape, forEach DON'T removed (ReplaceForEachWithForLoop owns it). Owner test now holds 7 bullets (added Void omit and empty-collection rows). Gate-silent probes added for the final, type-name, Set, public-init and where bullets. supersedes stays empty (idioms.md still holds bullets).
    - card deviation: a public init on a PUBLIC type is NOT redundant (the compiler never synthesizes a public one; the old bullet exempted it). The new bullet covers a public init on a NON-public type, measured silent in the tool.
    - needs a person / sibling card: (1) where decision: UseWhereClausesInForLoops stays OFF (option 1, status quo); no choice made. (2) the three test-target requirements (force unwrap, force try, force cast in tests) have no owner. Fix needs edits to code-hygiene/VALIDATOR.md and disallowed-constructs-swift.md, which this run was barred from. No tests assert on markdown text.
  timestamp: 2026-11-01T16:51:40.018928+00:00
- actor: claude-code
  id: 01m6c2b3333qjvvq210kwnwedw
  text: |-
    ### implement — changed
    - evidence: files changed (all, with the earlier step): builtin/validators/swift/rules/idioms.md, builtin/validators/swift/rules/immutability.md, builtin/validators/swift/rules/value-semantics.md, builtin/validators/code-hygiene/rules/idioms-swift.md (body only), crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/idioms_swift.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/swift_judgment_rules.rs, crates/swissarmyhammer-validators/src/builtin/mod.rs (comment only, earlier step), .kanban/tasks/01M1Y5JDJC7CHCDVFT92HD90TH.md (card body). cargo nextest run -p swissarmyhammer-validators: 836 run, 836 passed, 0 skipped; cargo fmt clean.
    - removed, rule 1: the `where` bullet and its description mention from idioms.md; the "loop whose body DOES something" sentence from immutability.md; the "UseWhereClausesInForLoops and immutability.md disagree" section and every "choice is a person's" line from idioms-swift.md. Kept one line: UseWhereClausesInForLoops is not in the allowlist because the tool cannot tell a loop that acts from a loop that builds a collection. The ReplaceForEachWithForLoop facts stay, without the `where` bullet. Tests: the Looping where probes left the idioms refusal and answer; the nested-if test now only holds the gate silent; no test reads rule or skill text.
    - removed, rule 2: the three test-target force requirements and their acceptance line from the card body. The carve-out is untouched.
    - note: builtin/validators/code-hygiene/VALIDATOR.md still names the `where` half near its lines 495-501. A sibling card owns that file; it needs a fix there.
    - next: ready for test and review.
  timestamp: 2026-11-01T17:26:00.035129+00:00
- actor: claude-code
  id: 01m6c2c8vfqjr529271s71n5pp
  text: |-
    ### commit — changed
    - evidence: a10de55ab refactor(validators): rebalance the Swift prompt rules around the toolchain gate. 6 files staged (value-semantics.md, idioms.md, immutability.md, idioms-swift.md, idioms_swift.rs, swift_judgment_rules.rs).
    - left out: builtin/mod.rs (no diff against HEAD); .kanban files (other work).
    - next: review
  timestamp: 2026-11-01T17:26:38.703994+00:00
- actor: claude-code
  id: 01m6c2k7vvp29gtkmw1edr4fv1
  text: |-
    ### review — findings
    - evidence: review sha a10de55ab~1..a10de55ab: the engine reviewed 2 files (idioms_swift.rs, swift_judgment_rules.rs), 0 findings. The engine could NOT review the 4 markdown files (value-semantics.md, idioms.md, immutability.md, idioms-swift.md): no validator matches *.md. Hand measurement of those 4 files gave 2 findings: (1) swift/rules/immutability.md:8 still states SwiftFormat's preferForLoop rewrite claim; (2) code-hygiene/rules/idioms-swift.md:121-125 table rows point at idioms.md bullets (empty-collection, Void) that idioms.md no longer holds.
    - hand-measured, MET: value-semantics.md has the final bullet with DO and DON'T; idioms.md has no where bullet, no Void, no Array/Dictionary empty-literal, no propertyTypes text, keeps Set<String>() literal, type-name bullet, public-init bullet (non-public type only); immutability.md has no pointer to a where clause; idioms-swift.md states UseWhereClausesInForLoops is not in the allowlist with the reason, no "choice is a person's" text, no propertyTypes text; supersedes stays empty and idioms.md still holds bullets; tests in the diff assert on no markdown text.
    - toolchain probes (Apple Swift 6.4.2, swift format lint --strict, 7-rule allowlist): nested-if loop, Set<String>(), static let redColor in struct Color, public init in struct Point, non-final class: silent (exit 0). Internal init, -> Void, [Int]() reported by UseSynthesizedInitializer, NoVoidReturnOnFunctionSignature, AlwaysUseLiteralForEmptyCollectionInit.
    - not recorded (other card): code-hygiene/VALIDATOR.md where half, swiftformat/swiftlint mentions in VALIDATOR.md files and tests doc comments.
    - next: implementer fixes the 2 open items; re-review.
  timestamp: 2026-11-01T17:30:27.067819+00:00
- actor: claude-code
  id: 01m6c2qdnbz03eg8rr73mzqrk8
  text: |-
    ### implement — changed
    - evidence: 2 files — builtin/validators/swift/rules/immutability.md, builtin/validators/code-hygiene/rules/idioms-swift.md. `cargo nextest run -p swissarmyhammer-validators --no-fail-fast`: 836 run, 836 passed, 0 skipped.
    - finding 1: removed the SwiftFormat/preferForLoop/0.62.1 sentence from the accumulator bullet; DO and both DON'T shapes kept.
    - finding 2: reworded the AlwaysUseLiteralForEmptyCollectionInit, NoVoidReturnOnFunctionSignature, DontRepeatTypeInStaticProperties, UseLetInEveryBoundCaseVariable and UseShorthandTypeNames rows to say what the tool decides, without a pointer to a missing bullet. Void and shorthand claims re-measured with swift format.
    - other mentions fixed: idioms-swift.md intro paragraph that named swiftformat/Nick Lockwood's SwiftFormat; idioms-swift.md Set paragraph that said it "writes the DO of idioms.md".
    - left alone: SwiftFormatRules path in two Swift-floor table rows (a real swift-format repo directory); access-control.md:17 names swiftformat's privateStateVariables (outside the four files named; needs its own card).
    - next: /review
  timestamp: 2026-11-01T17:32:44.075843+00:00
- actor: claude-code
  id: 01m6c2s14yb8m42xwmqfnnp8jt
  text: |-
    ### commit — changed
    - evidence: 98fcc616b docs(validators): drop SwiftFormat claims from the Swift rules; files: builtin/validators/swift/rules/immutability.md, builtin/validators/code-hygiene/rules/idioms-swift.md
    - next: review
  timestamp: 2026-11-01T17:33:36.798837+00:00
- actor: claude-code
  id: 01m6c2vvhesxj48a826yprppvv
  text: |-
    ### review — findings (iteration 2, range 98fcc616b~1..98fcc616b)
    - engine: `review sha` reviewed 0 files, 0 findings. It could NOT review `builtin/validators/swift/rules/immutability.md` and `builtin/validators/code-hygiene/rules/idioms-swift.md` (no validator matches *.md). The engine result proves nothing, so the hand measurement decides.
    - board: both findings of the first review read `[x]` on the board (confirmed with get task).
    - hand: `rg -i "swiftformat|preferForLoop|propertyTypes|0\.62\.1"` over immutability.md, idioms.md, value-semantics.md, idioms-swift.md: 2 hits, both real paths `Sources/SwiftFormatRules/...` (idioms-swift.md lines 763-764). Fixed.
    - hand: every pointer in idioms-swift.md to idioms.md names the Set bullet, the type-name bullet or the public-init bullet. idioms.md holds exactly those three. Fixed. immutability.md keeps the DO and both DON'T shapes.
    - hand: `swift format lint --strict` probe (Apple Swift 6.4.2), rules ON: `[Int]()` and `[String: Int]()` reported, `Set<String>()` silent (AlwaysUseLiteralForEmptyCollectionInit row correct). `-> Void` and `-> ()` both reported (NoVoidReturnOnFunctionSignature row correct). `Array<Int>`, `Dictionary<String, Int>`, `Optional<Int>` reported (UseShorthandTypeNames row correct).
    - FINDING: idioms-swift.md:128, the `UseLetInEveryBoundCaseVariable` row says "one `let` on the pattern, not a `let` on each bound case variable". The probe shows the OPPOSITE: `case let .p(m, n)` is reported ("move this 'let' keyword inside the 'case' pattern, before each of the bound variables"); `case .p(let m, let n)` is silent.
    - next: implementer rewords the row, checks the whole file for the same reversed direction, then re-review.

    step: review
    outcome: findings
    evidence: 1 finding, builtin/validators/code-hygiene/rules/idioms-swift.md:128; engine 0 findings with 2 md files skipped; hand-measured rg (2 allowed path hits), idioms.md pointers, 3 swift format lint probes
    task: ^2hd90th
  timestamp: 2026-11-01T17:35:09.358127+00:00
- actor: claude-code
  id: 01m6c30cn2eecg3m94pkwvzhez
  text: |-
    ### implement — changed
    - evidence: 1 file, builtin/validators/code-hygiene/rules/idioms-swift.md (allowlist rows 128 and 127). Re-measured every row with swift format lint --strict, one rule ON per probe (Apple Swift 6.4.x).
    - UseLetInEveryBoundCaseVariable: WRONG direction, fixed. `case let .p(m, n)` is reported; `case .p(let m, let n)` is silent. Row now says a let on each bound variable.
    - ReplaceForEachWithForLoop: row too narrow, fixed. Tool reports every `.forEach { ... }` closure call (with if, with return, plain print, filter chain); `forEach(print)` is silent.
    - AlwaysUseLiteralForEmptyCollectionInit: OK ([Int]() and [String: Int]() reported, Set<String>() silent, OFF in default config).
    - DontRepeatTypeInStaticProperties: OK (static let redColor = Color() in Color reported; greenColor = 2 and a differently named type silent).
    - NoVoidReturnOnFunctionSignature: OK (-> Void and -> () reported).
    - UseShorthandTypeNames: OK (Array<Int>, Dictionary<K,V>, Optional<Int> reported).
    - UseSynthesizedInitializer: OK (internal init reported; public init on public type and on internal type silent).
    - Searched idioms-swift.md, swift/rules/idioms.md, immutability.md, value-semantics.md: other statements of the let direction (idioms.md line 11, idioms-swift.md lines 21 and 861) are correct. Finding flipped to [x].
    - tests: cargo nextest run -p swissarmyhammer-validators --no-fail-fast: 836 run, 836 passed, 0 skipped.
  timestamp: 2026-11-01T17:37:37.954370+00:00
- actor: claude-code
  id: 01m6c318p4mbfyjjn4wevab94y
  text: |-
    ### commit — changed
    - evidence: b9ad16196 docs(validators): correct two allowlist rows of the idioms-swift rule (builtin/validators/code-hygiene/rules/idioms-swift.md)
    - next: review
  timestamp: 2026-11-01T17:38:06.660983+00:00
- actor: claude-code
  id: 01m6c374qppsp5pdc1pgsvejzz
  text: |-
    ### review — clean
    - scope: review sha b9ad16196~1..b9ad16196, iteration 3. Engine: 0 findings, 0 files reviewed. It could NOT review builtin/validators/code-hygiene/rules/idioms-swift.md (no validator matches *.md), so its zero proves nothing. Hand measurement below.
    - prior items: `get task` shows all 3 prior findings as `- [x]`.
    - hand-measured with `swift format lint --strict` (Apple Swift 6.4.2), only one rule ON per probe (config from dump-configuration, all other rules false), one reported shape and one silent shape per row:
      - AlwaysUseLiteralForEmptyCollectionInit: `[Int]()` and `[String: Int]()` reported; `Set<String>()` silent; literal forms silent. Default config has it false. Row matches.
      - DontRepeatTypeInStaticProperties: `static let redColor = Color()` in `struct Color` reported; annotated member, inferred-from-other-type member, computed member and `red` silent. Row matches.
      - NoVoidReturnOnFunctionSignature: `-> Void` and `-> ()` on a func both reported; no return clause and `() -> Void` closure type silent. Row matches.
      - ReplaceForEachWithForLoop: filter chain + forEach, `forEach { if ... }` and `forEach { print($0) }` all reported; `forEach(print)` silent. Row matches (every closure call).
      - UseLetInEveryBoundCaseVariable: `case let .p(m, n)` reported; `case .p(let m, let n)` silent. Row matches.
      - UseShorthandTypeNames: `Array<Int>`, `Dictionary<String, Int>`, `Optional<Int>` reported; shorthand silent. Row matches.
      - UseSynthesizedInitializer: internal init on internal struct reported; public init on public struct and public init on internal struct silent. Row matches.
    - rg -i "swiftformat|preferForLoop|propertyTypes|0\.62\.1" over swift/rules/*.md and idioms-swift.md: only idioms-swift.md:763-764 (real paths Sources/SwiftFormatRules/...) and the known access-control.md:17 (other card).
    - pointers to idioms.md bullets (idioms-swift.md lines 652, 834, 852-863): Set literal, type-name, public-init. idioms.md holds all three.
    - no other sentence states the direction of UseLetInEveryBoundCaseVariable (rg: lines 21, 127, 128, 861 only).
    - next: task moved to done.
  timestamp: 2026-11-01T17:41:19.222263+00:00
depends_on:
- 01M1Y65VQ8V4TEWXDXD4Y2ZQNX
- 01M1Y5GD85944ACE2XS6SA5XWJ
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffffe180
title: Rebalance the Swift prompt rules for the toolchain gate
---
## What

The move to `swift format` changes which bullets a tool owns. One requirement takes one owner, thus each bullet must move to the correct side. Read the table of the task "Measure the Swift toolchain rules the gate needs" before you move a bullet.

Files to change:
- `builtin/validators/swift/rules/value-semantics.md`
- `builtin/validators/swift/rules/idioms.md`
- `builtin/validators/swift/rules/immutability.md`
- `builtin/validators/swift/rules/optionals.md`
- `builtin/validators/code-hygiene/rules/idioms-swift.md` — the `supersedes` key and the body section about it
- `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/swift_judgment_rules.rs`
- `crates/swissarmyhammer-validators/src/builtin/mod.rs`

### Bullets that come BACK to the prompt

| the bullet | why it returns |
|---|---|
| `value-semantics.md` mark classes `final` | the toolchain has no rule like `preferFinalClasses` |
| `idioms.md` no memberwise initializer identical to the synthesized one, for a PUBLIC initializer | measured, `UseSynthesizedInitializer` reports the internal form and is silent for the public form. Row 4 holds the evidence |

### Bullets that STAY in the prompt

| the bullet | why it stays |
|---|---|
| `idioms.md` do not repeat the type name in a static member | measured, `DontRepeatTypeInStaticProperties` reports nothing for the shape the bullet names. Row 3 holds the evidence |

### Bullets that LEAVE the prompt

| the bullet of `idioms.md` | the toolchain rule that takes it |
|---|---|
| empty-collection variables use a literal | `AlwaysUseLiteralForEmptyCollectionInit` |
| omit the return clause when it is `Void` | `NoVoidReturnOnFunctionSignature`, BOTH halves |

The empty-collection bullet holds a long note about SwiftFormat's `propertyTypes` rule and `--property-types inferred`. Delete that note with the bullet. The toolchain decides the bullet in the correct direction, thus the warning is not true.

### The `where` bullet waits for a decision

`UseWhereClausesInForLoops` contradicts `immutability.md`, which names this shape as a DON'T whose fix is `map` or `filter`, NOT a `where` clause:

```swift
for user in users { if user.isActive { names.append(user.name) } }
```

Row 5 of the measurement task states the two answers and chooses neither. Bring the evidence to a person and let them choose. Then write the answer into `idioms.md` and `immutability.md` so the two rules cannot disagree. `swift_judgment_rules.rs` holds the carve-out sentence as a test; update it for the answer.

Measured, `ReplaceForEachWithForLoop` reports `things.filter { $0 > 2 }.forEach { thing in print(thing) }` and asks for a for-in loop, which is not the `where` clause the bullet demands. Ask "does ANY enabled rule report this line, and what shape does its fix land on", and state both in the rule body.

**Delete `idioms.md` only if it then states no bullet.** The type-name bullet, the public memberwise-initializer bullet and perhaps the `where` half all stay, thus the file is probably not empty. `idioms-swift` states `supersedes: idioms` only when the file is gone.

## Acceptance Criteria
- [ ] `value-semantics.md` states the `final` bullet again, with a DO and a DON'T.
- [ ] `idioms.md` states the type-name bullet and the public memberwise-initializer bullet.
- [ ] `idioms.md` states no bullet that a rule of the gate allowlist decides.
- [ ] `idioms.md` states nothing about `propertyTypes` or `--property-types`.
- [ ] `idioms.md` and `immutability.md` cannot disagree about a filtering loop.
- [ ] The `supersedes` key of `idioms-swift` agrees with what `idioms.md` holds.
- [ ] No bullet has two owners. No bullet has no owner.

## Tests
- [ ] Update `the_shipped_swift_idioms_tool_rule_owns_each_bullet_it_took` in `crates/swissarmyhammer-validators/src/builtin/mod.rs` for the new set of taken bullets.
- [ ] Write a test that holds `value-semantics.md` to stating the `final` bullet, and holds the gate silent for a class that is not `final`.
- [ ] Write a test that holds `idioms.md` to stating the type-name bullet, and holds the gate silent for `static let redColor` in `struct Color`.
- [ ] Write a test that holds `idioms.md` to naming `propertyTypes` nowhere.
- [ ] Update the carve-out test in `swift_judgment_rules.rs` for the `where` answer.
- [ ] Run `cargo nextest run -p swissarmyhammer-validators swift`. Every test passes.

## Workflow
- Use `/tdd` — write the failing tests first, then write the code that makes them pass. #swift

## Review Findings (2026-11-01 11:40)

> Scope: `review sha a10de55ab~1..a10de55ab`. The engine reviewed 2 files (`idioms_swift.rs`, `swift_judgment_rules.rs`): 0 findings. The engine could NOT review the 4 markdown files (`value-semantics.md`, `idioms.md`, `immutability.md`, `idioms-swift.md`): no validator matches `*.md`. A hand measurement of those 4 files produced the 2 findings below. All other acceptance criteria are met (see the step comment).

- [x] `builtin/validators/swift/rules/immutability.md:8` `swift/immutability` — The accumulator bullet still says "SwiftFormat's `preferForLoop` REWRITES `users.forEach { if $0.isActive { names.append($0.name) } }` into that second DON'T — measured on 0.62.1, the fix writes ... — so the accumulator loop is where the tool's own correction lands". The project does not use SwiftFormat. `swift format` is the one formatter, and its `ReplaceForEachWithForLoop` reports that `forEach` and corrects nothing (measured: `--in-place` leaves the file byte for byte as it was). The claim is not true for the tool in use. Remove the SwiftFormat/`preferForLoop` sentence from the bullet. Keep the DO and both DON'T shapes.
- [x] `builtin/validators/code-hygiene/rules/idioms-swift.md:121-125` `code-hygiene/idioms-swift` — The allowlist table rows say `AlwaysUseLiteralForEmptyCollectionInit` decides "the empty-collection bullet of `idioms.md`, in the direction that rule asks for" and `NoVoidReturnOnFunctionSignature` decides "the `Void` return clause, BOTH halves of the bullet `idioms.md` split". `idioms.md` no longer holds a `Void` bullet, and its only empty-collection bullet is the `Set` bullet, which this rule does NOT decide (measured: silent for `Set<String>()`). The rows name a bullet that the file does not have and imply two owners for the `Set` bullet. Reword both rows so each names what the tool decides (the empty `Array` and `Dictionary` literal; the `Void` return clause) and does not point at a bullet of `idioms.md`. Check the other rows of the table (`UseLetInEveryBoundCaseVariable`, `UseShorthandTypeNames`) for the same pointer to a bullet that no prompt rule holds.

## Review Findings (2026-11-01 11:50)

> Scope: `review sha 98fcc616b~1..98fcc616b`. The engine reviewed 0 files. It could NOT review the 2 changed files (`immutability.md`, `idioms-swift.md`): no validator matches `*.md`. It reports 0 findings, which proves nothing. A hand measurement produced the 1 finding below. Finding 1 and finding 2 of the first review are fixed: `rg -i "swiftformat|preferForLoop|propertyTypes|0\.62\.1"` over `immutability.md`, `idioms.md`, `value-semantics.md` and `idioms-swift.md` hits only the real paths `Sources/SwiftFormatRules/...` (idioms-swift.md lines 763-764). Every pointer in `idioms-swift.md` to an `idioms.md` bullet names the Set bullet, the type-name bullet or the public-init bullet, and `idioms.md` holds those three.

- [x] `builtin/validators/code-hygiene/rules/idioms-swift.md:128` `code-hygiene/idioms-swift` — The allowlist row for `UseLetInEveryBoundCaseVariable` says "one `let` on the pattern, not a `let` on each bound case variable". The tool decides the OPPOSITE. Measured with `swift format lint --strict` (Apple Swift 6.4.2): `case let .p(m, n)` is reported with `move this 'let' keyword inside the 'case' pattern, before each of the bound variables`, and `case .p(let m, let n)` is silent. The row states the wrong direction. Reword the row to say the tool decides a `let` on each bound case variable, and not one `let` on the pattern. Then check the whole file for any other sentence that states the direction of this rule.
