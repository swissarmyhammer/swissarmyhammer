---
name: idioms
description: Shorthand types, Void returns, literal empty collections, synthesized initializers, for over forEach
---

# Swift Idioms

This file lists semantic idioms that look wrong in a diff. Whitespace,
indentation, and import order are `swift-format`'s job — they are not review
findings here.

- **Use shorthand type sugar.** DO: `[Int]`, `[Key: Value]`, `String?`. DON'T: `Array<Int>`, `Dictionary<Key, Value>`, `Optional<String>`.
- **Declare an empty collection with a literal and a type annotation. Do not declare it with a call.** DO: `var items: [Int] = []`. DON'T: `var items = [Int]()`. This rule applies to `Set` and to every other `ExpressibleByArrayLiteral`/`ExpressibleByDictionaryLiteral` type: `var ids: Set<String> = []` is the idiomatic form. Do not flag it toward `Set<String>()`; the annotated literal wins, and flip-flopping between the two forms across review rounds is always a validator error.
- **Return `Void`, not `()`.** Omit the return clause entirely when the return type is `Void`. DON'T: `func f() -> ()`, `func f() -> Void {}`. DO: `func f() {}`.
- **Do not write a memberwise initializer identical to the synthesized one.** Delete it and let the compiler synthesize it. An exception applies to public initializers.
- **Do not repeat the enclosing type's name in a static member.** DON'T: `static let redColor` on `Color`. DO: `static let red`.
- **Use a `for` loop (with a `where` clause when filtering) instead of `forEach` + `if`** when you need control flow — `forEach` cannot `break`/`continue`/`return` out of the caller.
- **Bind each case variable with its own `let` inside the pattern.** DO: `case .point(let x, let y)`. DON'T: `case let .point(x, y)`.
