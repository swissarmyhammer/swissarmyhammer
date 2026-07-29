---
name: access-control
description: internal by default, deliberate open, no leaking lower-access types, explicit modifiers
---

# Swift Access Control

- **Default library code to `internal`. Add `public` only for API you intend for cross-module use.** Flag `public` sprayed on helpers no other module consumes.
- **Choose `public` vs `open` deliberately.** `open` is only for types/members you design to be subclassed or overridden from another module. `public final class` (usable, not an extension point) is the common, correct default for value-type libraries; a client's inability to subclass it is by design, not a bug.
- **Do not treat `private` and `fileprivate` as interchangeable. Verify reachability before you flag one as over-broad.** `private` reaches only within the same declaration and same-file extensions of that exact type. `fileprivate` is required when a sibling type in the same file accesses another sibling's members, or when an enclosing type's own methods reach into a nested type's members (or the reverse). Before you flag `fileprivate` as "should be `private`", trace every call site of the flagged member — if any caller is a different type (sibling or enclosing/nested boundary) in the same file, `private` would not compile there; leave `fileprivate` as correct.
- **Never expose a lower-access type through higher-access API.** DON'T: `public func make() -> InternalImpl` where `InternalImpl` is `internal`/`private`/`fileprivate`.
- **Spell out access modifiers explicitly on library declarations** when the intent is API-shaping, rather than leaning on the implicit `internal` default.
- **Pair `@inlinable` public API with `@usableFromInline` on the internal symbols it references** — inlinable bodies are emitted into client modules and cannot see plain `internal` symbols. Do not treat `@usableFromInline`/underscored symbols as a stable public contract.
