---
name: state-modeling
description: Enums over boolean/optional soup, exhaustive switch, type-safe identifiers
---

# Swift State Modeling

- **Model mutually-exclusive state as an `enum`, not several `Bool`s or optionals that can't all be valid at once.** Multiple related flags, or several optionals where only one should ever be set, permit impossible states.
  - DON'T: `var isLoading = false; var result: Value?; var error: Error?`
  - DO: `enum LoadState { case idle, loading, loaded(Value), failed(Error) }`
- **Switches over domain enums are exhaustive — no `default:` that silently swallows future cases** when each case deserves deliberate handling. A `default` throws away the compiler's coverage check, so a newly added case is silently unhandled.
- **Give distinct domain identifiers distinct types, not interchangeable `String`/`Int`/`UUID`.** Passing a user id where an order id is expected should be a compile error, not a silent wrong-row fetch. Prefer a tagged/wrapper type (`Tagged<User, Int>` where the project uses swift-tagged, or a small `struct` id) over raw scalars threaded through APIs.
