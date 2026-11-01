---
assignees:
- claude-code
position_column: todo
position_ordinal: ffef80
project: swift-validator
title: 'swift: decide whether implicitly_unwrapped_optional joins the test-target carve-out'
---
Found while implementing `disallowed-constructs-swift` (task ^4s2n0rx). That card names THREE rules that stay off inside a test target — `force_unwrapping`, `force_try` and `force_cast` — and the shipped rule does exactly that. A fourth carve-out the prompt rule states is left unexpressed.

## What the prompt rule states

`builtin/validators/swift/rules/optionals.md`, the IUO bullet, word for word:

> **No implicitly unwrapped optionals (`Type!`).** DON'T: `var session: URLSession!`. DO: a non-optional initialized in `init`, or a real `URLSession?`. Sanctioned exceptions: `@IBOutlet`, and test fixtures set in `setUp()`.

Two sanctioned exceptions. `disallowed-constructs-swift` expresses the FIRST and not the second.

## What was measured

swiftlint 0.65.0. `swiftlint rules implicitly_unwrapped_optional` names the whole option set the rule accepts: `severity` and `mode`. There is no third key.

Measured with the shipped script over one file:

| the declaration | reported |
|---|---|
| `@IBOutlet var titleLabel: UILabel!` | no |
| `var plain: UILabel!` | yes |
| `var subject: Screen!` a `setUp()` method assigns | yes |

`mode: all_except_iboutlets`, which the shipped child config states, is what silences the first row. The mode reads the ATTRIBUTE and never the assignment, so no option reaches the third row.

`var sut: SUT!` set in `setUp()` is a very common XCTest fixture shape, so a Swift package with XCTest suites gets one finding for each such fixture.

## The decision to make

Three answers, and this card is to pick one:

1. **Leave it.** The author writes `// swiftlint:disable:next implicitly_unwrapped_optional` with the reason, which is the escape hatch the whole set hands an author. The rule already documents this in the section "Two prompt carve-outs no option expresses".
2. **Add `implicitly_unwrapped_optional` to the test-target partition**, beside the three force rules. The partition already exists in the script — the roster it reads is written into `$work/product-only`, so this is one line. It is WIDER than the prompt bullet, which sanctions only a fixture `setUp()` assigns, so it would silence a plain IUO in test code too.
3. **Change the prompt bullet** so the two halves agree with whichever of the two the gate does.

The same question stands for `error-handling.md`'s `try!` bullet, whose sanctioned exception is "a literal that can fail solely through programmer error (e.g. a compile-time-constant regex)". `swiftlint rules force_try` names `severity` and nothing else, so that one has no path but the directive.

## Acceptance

- One of the three answers is taken, and the rule body, the prompt rule, and the acceptance tests all state the same thing.
- If answer 2 is taken: an acceptance test in `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/disallowed_constructs_swift.rs` holds an IUO under `Tests/` to silent and the same bytes under `Sources/` to reporting, beside the existing force-unwrap test.
#tool-validators