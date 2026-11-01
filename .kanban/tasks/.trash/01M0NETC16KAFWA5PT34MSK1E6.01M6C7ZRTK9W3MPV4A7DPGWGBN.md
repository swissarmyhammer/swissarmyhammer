---
assignees:
- claude-code
position_column: todo
position_ordinal: fff480
project: swift-validator
title: 'swift validators: make the rule-scope partition a shipped artifact, not a guess'
---
Found while driving ^052w80d. Two review iterations each found NEW pairs of Swift prompt rules where one rule's DO is another rule's DON'T. The third iteration found one in a pair the previous sweep had explicitly declared CLEAR.

That is not a prose defect. It is a verification defect, and patching pair-by-pair will not end it.

## The arithmetic

There are 14 Swift prompt rules. That is **91 pairs**.

- **3** pairs are frozen in `SWIFT_SHARED_SHAPES` in `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/swift_judgment_rules.rs`, where a test reads them.
- **88** pairs live in a kanban comment that no test reads.

So every iteration re-guesses those 88 by hand, and reaches a different answer. Each round adds one row to a table of three. The next round will find another. This is the shape of work that never finishes.

## What is actually wrong

A rule's SCOPE is currently implicit — it exists only in the prose of its bullets and in whatever the last reviewer happened to hold in their head. Nothing declares it, so nothing can check it.

## The fix

Make the partition the shipped artifact:

1. **Every Swift prompt rule declares its scope machine-readably.** The exact form is yours to design — frontmatter, a structured section, whatever the loader can read. It must say what the rule decides and, where it borders another rule, which side of the border is whose.
2. **One test enumerates ALL pairs** and fails when two rules claim the same shape without a stated boundary on both sides.
3. The three known collisions and the five declared-clear pairs become rows in that enumeration, not prose in a comment.

Then a new rule cannot be added without declaring where it sits, and a collision fails a test instead of waiting for a reviewer with enough context to notice.

## Known collisions to seed the table

- `idioms.md` vs `immutability.md` — the accumulator loop. Boundary sentences exist but are NOT complements: `idioms.md` carves out "only appends into a collection"; `immutability.md` says "DOES something for each element it keeps". `names.append(user.name)` satisfies both readings, so word for word each rule hands the shape to the other.
- `preconditions.md` vs `error-handling.md` — the `""` return. Fixed in one direction only; `error-handling.md` still lists `""` as a flat DON'T and was never touched.
- `initialization.md` vs `state-modeling.md` — `var result: Value?` is in scope for both with incompatible fixes: an `enum` versus a `private let` that cannot be written for a value that does not exist until a load returns.

## Scope note

This is a design change to how the validator bundle states and proves rule ownership. It is deliberately NOT more prose patching. If it turns out to need an engine change to read the declarations, say so and card that separately rather than widening this one.

## Acceptance

- Every Swift prompt rule declares its scope, and the loader reads it.
- One test enumerates all 91 pairs and fails on an undeclared overlap.
- The three collisions above are resolved with complementary boundaries — verified by the test, not by reading.
- Adding a 15th rule without a scope declaration fails a test.