---
name: fluent-usage
description: Call sites read as phrases, make-prefixed factories, mutating/non-mutating pairs, verb vs noun
---

# Swift Fluent Usage

- **Write method and function names as a grammatical phrase at the call site.** Read the call aloud. DO: `x.insert(y, at: z)` ("insert y at z"), `x.subviews(havingColor: c)`. DON'T: `x.insert(y, position: z)`, `x.subviews(color: c)`.
- **Attach the preposition to the argument label, not the base name.** DO: `x.removeBoxes(havingLength: 12)`. DON'T: `x.removeBoxesHavingLength(12)`.
- **Start a factory method name with `make`. Do not let an initializer's first argument form a phrase with the base name.** DO: `x.makeIterator()`, `Color(red: r, green: g, blue: b)`. DON'T: `x.iterator()` as a factory, `Color(havingRGBValuesRed: r, green: g, blue: b)`.
- **Omit the first argument label only for value-preserving conversions.** DO: `Int64(someUInt32)`. Label it in every other case.
- **Follow the verb/noun rule for mutating and non-mutating pairs.**
  - For verb operations, use the imperative verb for the mutating form and add `ed`/`ing` for the non-mutating form. DO: `sort()`/`sorted()`, `reverse()`/`reversed()`, `append(x)`/`appending(x)`. DON'T: `sortInPlace()`, or a `sorted()` that mutates.
  - For noun operations, use the noun for the non-mutating form and the `form` prefix for the mutating form. DO: `union(z)`/`formUnion(z)`. DON'T: `unioned(z)`, `unionInPlace(z)`.
- **Name a side-effect-free operation with a noun phrase. Name an operation with side effects with an imperative verb phrase.** DO (pure): `x.distance(to: y)`, `i.successor()`. DO (effectful): `x.sort()`, `x.append(y)`.
