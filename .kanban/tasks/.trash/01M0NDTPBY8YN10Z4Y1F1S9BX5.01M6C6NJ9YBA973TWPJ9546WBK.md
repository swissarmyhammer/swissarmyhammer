---
assignees:
- claude-code
position_column: todo
position_ordinal: fff380
project: swift-validator
title: 'swift: give the type-name-repetition requirement one owner (naming-clarity vs idioms)'
---
Found by the DO-versus-DON'T sweep of ^052w80d. Two prompt rules of the `swift` set state the same requirement.

- `naming-clarity.md` — "**Omit needless words.**" DON'T: `Color.colorRed`. DO: `Color.red`.
- `idioms.md` — "**Don't repeat the enclosing type's name in a static member.**" DON'T: `static let redColor` on `Color`. DO: `static let red`.

One declaration, `static let redColor` on `Color`, draws a finding from both. The two prescribe the SAME fix, so this is duplication, not a contradiction — an author who obeys one satisfies the other, and no review round churns. That is why the sweep did not treat it as a blocker.

It is still two owners for one requirement, which `builtin/validators/swift/VALIDATOR.md` refuses: "One requirement takes one owner". The reviewer reads two findings on one line.

## What to do

Decide which rule owns type-name repetition in a member, and narrow the other so the two cannot both fire.

- `idioms.md` is the narrower and more specific statement: the enclosing type's name inside a static member.
- `naming-clarity.md`'s bullet also carries `allViews.removeElement(button)` and `user.userName`, which `idioms.md` does not read. So the `Color.colorRed` example is the overlapping part, not the whole bullet.

Then add the pair to `SWIFT_SOLE_OWNERS` in
`crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/swift_judgment_rules.rs`,
so the boundary is held by `every_swift_requirement_two_prompt_rules_could_state_stands_in_one_of_them`
rather than by memory.

Measure the survivor against the five shipped file-scope Swift gates before the edit lands, over a probe staging a `.swift-version` of `6.3`. #tool-validators