---
name: immutability
description: map over a var accumulator, static let or a computed static var over stored static var, no global functions
---

# Swift Immutability

- **Build a collection with `map`/`compactMap`, not a `var` accumulator.** The accumulator is mutable for the whole loop, so a reader must walk every line of the body to learn the final value. DON'T: `var names: [String] = []` beside `for user in users where user.isActive { names.append(user.name) }`, and DON'T the same loop written with a nested `if`: `for user in users { if user.isActive { names.append(user.name) } }`. DO: `let names = users.filter(\.isActive).map(\.name)`. SwiftFormat's `preferForLoop` REWRITES `users.forEach { if $0.isActive { names.append($0.name) } }` into that second DON'T — measured on 0.62.1, the fix writes `for user in users { if user.isActive { names.append(user.name) } }` — so the accumulator loop is where the tool's own correction lands, and this bullet names it in both shapes.
- **A `static` member is a `let`, or a computed `var`.** A stored `static var` is shared mutable state: every caller reads it, any caller writes it, and no caller owns it. DON'T: `static var attemptLimit = 3`. DO: `static let attemptLimit = 3`, and `static var totalBudgetSeconds: Int { attemptLimit * backoffSeconds }` where the value is derived from other members.
- **A function belongs to a type.** A top-level `func` carries no namespace, so its name must state the whole context and every file that imports the module sees it. DON'T: `func formatDuration(_ seconds: Int) -> String`. DO: `extension Int { var formattedDuration: String { … } }`, or a `static func` on an `enum` namespace.
