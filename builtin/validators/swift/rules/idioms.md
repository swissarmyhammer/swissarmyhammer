---
name: idioms
description: Shorthand types, Void returns, literal empty collections, synthesized initializers, for over forEach
---

# Swift Idioms

Semantic idioms that read wrong in a diff. (Whitespace, indentation, and import
ordering are `swift-format`'s job, not review findings.)

- **Use shorthand type sugar.** DO: `[Int]`, `[Key: Value]`, `String?`. DON'T: `Array<Int>`, `Dictionary<Key, Value>`, `Optional<String>`.
- **Empty-collection variables use a literal with a type annotation, not a call.** DO: `var items: [Int] = []`. DON'T: `var items = [Int]()`. This includes `Set` and every other `ExpressibleByArrayLiteral`/`ExpressibleByDictionaryLiteral` type: `var ids: Set<String> = []` is the idiomatic form — do NOT flag it toward `Set<String>()`; the annotated literal wins, and flip-flopping between the two forms across review rounds is always a validator error.
- **Return `Void`, not `()`, and omit the return clause entirely when it's `Void`.** DON'T: `func f() -> ()`, `func f() -> Void {}`. DO: `func f() {}`.
- **Don't write a memberwise initializer identical to the synthesized one** — delete it and let the compiler synthesize it. Exceptions allowed for public initializes.
- **Don't repeat the enclosing type's name in a static member.** DON'T: `static let redColor` on `Color`. DO: `static let red`.
- **Prefer a `for` loop (with a `where` clause when filtering) over `forEach` + `if`** when you need control flow — `forEach` can't `break`/`continue`/`return` out of the caller.
- **Bind each case variable with its own `let` inside the pattern.** DO: `case .point(let x, let y)`. DON'T: `case let .point(x, y)`.
