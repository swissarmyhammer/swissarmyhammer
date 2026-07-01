---
name: access-control
description: internal by default, deliberate open, no leaking lower-access types, explicit modifiers
---

# Swift Access Control

- **Library code defaults to `internal`; add `public` only for intended cross-module API.** Flag `public` sprayed on helpers no other module consumes.
- **Choose `public` vs `open` deliberately.** `open` is only for types/members designed to be subclassed or overridden from another module. `public final class` (usable, not an extension point) is the common, correct default for value-type libraries; a client being unable to subclass it is by design, not a bug.
- **Never expose a lower-access type through higher-access API.** DON'T: `public func make() -> InternalImpl` where `InternalImpl` is `internal`/`private`.
- **Spell access modifiers explicitly on library declarations** when the intent is API-shaping, rather than leaning on the implicit `internal` default.
- **Pair `@inlinable` public API with `@usableFromInline` on the internal symbols it references** — inlinable bodies are emitted into client modules and can't see plain `internal` symbols. Don't treat `@usableFromInline`/underscored symbols as stable public contract.
