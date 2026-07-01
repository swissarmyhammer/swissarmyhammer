---
name: optionals
description: No force unwrap or IUO in non-test code, guard-let early exits, no Optional where a default fits
---

# Swift Optionals

- **No force unwrap (`!`) in non-test code.** DON'T: `let name = user.name!`. DO: `guard let name = user.name else { … }` or `user.name ?? "Anonymous"`.
- **Use `guard let … else { return/throw }` for early exit** so the happy path stays unindented. DON'T nest the whole success branch inside `if let` with a trailing `else { return }`.
- **No implicitly unwrapped optionals (`Type!`).** DON'T: `var session: URLSession!`. DO: a non-optional initialized in `init`, or a real `URLSession?`. Sanctioned exceptions: `@IBOutlet`, and test fixtures set in `setUp()`.
- **Don't use `Optional` where a sensible default exists.** `nil` should mean genuine absence, not error or sentinel. DON'T: `func timeout() -> Int?` that callers always `?? 30`. DO: `func timeout() -> Int { 30 }`. Keep `firstIndex(of:) -> Int?` — `nil` there is true absence, never a `-1` sentinel.
