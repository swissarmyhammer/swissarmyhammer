---
name: error-handling
description: Typed Error enums, throws over sentinels, no force try/cast, fatalError for programmer error only
---

# Swift Error Handling

- **Define typed errors as an `enum`/`struct` that conforms to `Error`. Namespace each error by type.** DO: `enum ChannelError: Error { case alreadyClosed; case connectTimeout(TimeAmount) }`. DON'T: `throw NSError(domain: "x", code: -1)` or throwing bare strings.
- **Use `throws` instead of an optional/sentinel return when the caller must learn *why* something failed.** DO: `func validate() throws`. DON'T: `func validate() -> Bool` that hides the reason, or `-1`/`""` sentinels.
- **Do not use `try!` in code that is not test code.** DON'T: `let data = try! Data(contentsOf: url)`. DO: `try` with `do`/`catch`, or `try?`. The only sanctioned `try!` is a literal that can fail only through programmer error (e.g. a compile-time-constant regex).
- **Do not use `as!` force-cast in code that is not test code.** DON'T: `segue.destination as! DetailVC`. DO: `guard let vc = segue.destination as? DetailVC else { … }`.
- **Use `fatalError`/`preconditionFailure` only for programmer error, never on a recoverable path.** Bad input, missing files, and network failures must return typed `throws` errors. `precondition`/`assert` remain legitimate for API-contract violations (e.g. an out-of-range index).
