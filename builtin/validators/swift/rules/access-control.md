---
name: access-control
description: internal by default, deliberate open, no leaking lower-access types, explicit modifiers
---

# Swift Access Control

- **Library code defaults to `internal`; add `public` only for intended cross-module API.** Flag `public` sprayed on helpers no other module consumes.
- **Choose `public` vs `open` deliberately.** `open` is only for types/members designed to be subclassed or overridden from another module. `public final class` (usable, not an extension point) is the common, correct default for value-type libraries; a client being unable to subclass it is by design, not a bug.
- **`private` and `fileprivate` are not interchangeable — verify reachability before flagging one as over-broad.** `private` only reaches within the same declaration and same-file extensions of that exact type. `fileprivate` is required when a sibling type in the same file accesses another sibling's members, or when an enclosing type's own methods reach into a nested type's members (or vice versa). Before flagging `fileprivate` as "should be `private`", trace every call site of the flagged member — if any caller is a different type (sibling or enclosing/nested boundary) in the same file, `private` would not compile there; leave `fileprivate` as correct.
- **Never expose a lower-access type through higher-access API.** DON'T: `public func make() -> InternalImpl` where `InternalImpl` is `internal`/`private`/`fileprivate`.
- **Spell access modifiers explicitly on library declarations** when the intent is API-shaping, rather than leaning on the implicit `internal` default.
- **Pair `@inlinable` public API with `@usableFromInline` on the internal symbols it references** — inlinable bodies are emitted into client modules and can't see plain `internal` symbols. Don't treat `@usableFromInline`/underscored symbols as stable public contract.
