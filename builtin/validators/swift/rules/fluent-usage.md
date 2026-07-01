---
name: fluent-usage
description: Call sites read as phrases, make-prefixed factories, mutating/non-mutating pairs, verb vs noun
---

# Swift Fluent Usage

- **Method and function names form a grammatical phrase at the call site.** Read the call aloud. DO: `x.insert(y, at: z)` ("insert y at z"), `x.subviews(havingColor: c)`. DON'T: `x.insert(y, position: z)`, `x.subviews(color: c)`.
- **Attach the preposition to the argument label, not the base name.** DO: `x.removeBoxes(havingLength: 12)`. DON'T: `x.removeBoxesHavingLength(12)`.
- **Factory methods begin with `make`; don't let an initializer's first argument form a phrase with the base name.** DO: `x.makeIterator()`, `Color(red: r, green: g, blue: b)`. DON'T: `x.iterator()` as a factory, `Color(havingRGBValuesRed: r, green: g, blue: b)`.
- **Omit the first argument label only for value-preserving conversions.** DO: `Int64(someUInt32)`. Otherwise, label it.
- **Mutating/non-mutating pairs follow the verb/noun rule.**
  - Verb operations: mutating is the imperative verb; non-mutating adds `ed`/`ing`. DO: `sort()`/`sorted()`, `reverse()`/`reversed()`, `append(x)`/`appending(x)`. DON'T: `sortInPlace()`, or a `sorted()` that mutates.
  - Noun operations: non-mutating is the noun; mutating gets the `form` prefix. DO: `union(z)`/`formUnion(z)`. DON'T: `unioned(z)`, `unionInPlace(z)`.
- **Side-effect-free operations are noun phrases; operations with side effects are imperative verb phrases.** DO (pure): `x.distance(to: y)`, `i.successor()`. DO (effectful): `x.sort()`, `x.append(y)`.
