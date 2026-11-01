---
name: idioms
description: Type-name repetition in static members, empty Set literals, public initializers that repeat the synthesized one
---

# Swift Idioms

Semantic idioms that read wrong in a diff. (Whitespace, indentation, and import
ordering are `swift format`'s job, not review findings. So are the shorthand type
names, the `Void` return clause, the empty `Array` and `Dictionary` literal, the
internal memberwise initializer, the `let` on each bound case variable, and
`forEach` — the `idioms-swift` tool rule decides those.)

- **Empty `Set` variables use a literal with a type annotation, not a call.** DO: `var ids: Set<String> = []`. DON'T: `var ids = Set<String>()`. The tool rule stays silent for `Set`, so this bullet is the only owner of it.
- **Don't repeat the enclosing type's name in a static member.** DON'T: `static let redColor` on `Color`. DO: `static let red`. The tool rule reads one shape only, a member whose initializer calls its own type, so every other shape is this bullet's.
- **Don't write a `public init` that only repeats the synthesized memberwise initializer on a type that is not `public`.** The compiler synthesizes an internal memberwise initializer, and a `public` one on an internal or `private` type reaches no more callers than that. DON'T: `struct Point { var x: Int; public init(x: Int) { self.x = x } }`. DO: delete the `init`. A `public init` on a `public` type stays, because the compiler never synthesizes a `public` one. The tool rule reports the internal form only.
