---
name: state-modeling
description: Enums over boolean/optional soup, exhaustive switch, type-safe identifiers
---

# Swift State Modeling

- **Model mutually-exclusive state as an `enum`.** Do not use several `Bool` values or optionals that cannot all be valid at once. Multiple related flags, or several optionals where only one should ever be set, permit impossible states.
  - DON'T: `var isLoading = false; var result: Value?; var error: Error?`
  - DO: `enum LoadState { case idle, loading, loaded(Value), failed(Error) }`
- **Write exhaustive switches over domain enums.** Do not add a `default:` case that silently swallows future cases, when each case deserves deliberate handling. A `default` case throws away the compiler's coverage check, so a newly added case goes silently unhandled.
- **Give distinct domain identifiers distinct types.** Do not use interchangeable `String`/`Int`/`UUID` types for them. Passing a user id where an order id is expected must be a compile error, not a silent wrong-row fetch. Use a tagged/wrapper type (`Tagged<User, Int>` where the project uses swift-tagged, or a small `struct` id) instead of a raw scalar threaded through APIs.
