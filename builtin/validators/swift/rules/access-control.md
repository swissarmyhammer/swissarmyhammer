---
name: access-control
description: internal by default, deliberate open, no leaking lower-access types, explicit modifiers
---

# Swift Access Control

- **Library code defaults to `internal`; add `public` only for intended cross-module API.** Flag `public` sprayed on helpers no other module consumes.
- **Choose `public` vs `open` deliberately.** `open` is only for types/members designed to be subclassed or overridden from another module. `public final class` (usable, not an extension point) is the common, correct default for value-type libraries; a client being unable to subclass it is by design, not a bug.
- **Before you make an access level narrower, trace every call site — and remember that `@testable import` reaches `internal`, never `private`.** This applies to each narrowing direction: `public` to `internal`, `internal` to `fileprivate` or `private`, and `fileprivate` to `private`.
  - `private` reaches only the same declaration and same-file extensions of that exact type. `fileprivate` is required when a sibling type in the same file accesses another sibling's members, or when an enclosing type's own methods reach into a nested type's members (or vice versa). If any caller is a different type in the same file, `private` would not compile there; leave `fileprivate` as correct.
  - **A test target is a caller.** A Swift test target reads a library's `internal` members through `@testable import`, and `@testable` opens `internal` — it never opens `private` or `fileprivate`. So a member with no caller inside its own type may still be `internal` because a test calls it. That `internal` is load-bearing and correct as written.
  - **The diff is not the call graph.** This validator reads one changed file and runs no caller probe, so the call sites usually are not in front of you. When you cannot see them, do not flag the declaration. "I see no caller in this file" is not evidence that no caller exists.
  - A finding that would stop the build is a validator error, not a finding. When you do flag a member, name the call sites you traced.
- **Never expose a lower-access type through higher-access API.** DON'T: `public func make() -> InternalImpl` where `InternalImpl` is `internal`/`private`/`fileprivate`.
- **Spell access modifiers explicitly on library declarations** when the intent is API-shaping, rather than leaning on the implicit `internal` default.
- **Pair `@inlinable` public API with `@usableFromInline` on the internal symbols it references** — inlinable bodies are emitted into client modules and can't see plain `internal` symbols. Don't treat `@usableFromInline`/underscored symbols as stable public contract.
