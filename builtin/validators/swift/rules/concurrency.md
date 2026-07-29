---
name: concurrency
description: async/await over completion handlers, Sendable correctness, structured tasks, actor isolation
---

# Swift Concurrency

- **Use `async`/`await` instead of completion-handler callbacks in new code.** Flag a newly added `@escaping (Result<…>) -> Void` / `completion:` for inherently async work at a public boundary. DO: `func loadUser(id: User.ID) async throws -> User`.
- **Make types `Sendable` when they cross an actor/task boundary.** Do not pass a mutable non-`Sendable` class across an `await` or task boundary.
- **Document a synchronization invariant when you use `@unchecked Sendable`.** The problem is the *absence* of a lock/isolation mechanism and a comment, not the keyword itself. DON'T: `final class Counter: @unchecked Sendable { var n = 0 }` with no guard. DO: back it with a lock or actor and state the invariant in a comment.
- **Model new shared mutable state as an `actor`, not a hand-rolled `DispatchQueue`/`NSLock`.**
- **Do not leak an unstructured `Task { }`. Use structured concurrency instead.** DO: `async let a = fetchA(); async let b = fetchB()`, or `withTaskGroup` for a dynamic set. Store long-lived `Task` handles, cancel them at teardown, and check `Task.isCancelled`/`Task.checkCancellation()` in loops — `cancel()` is a no-op if nothing checks it. DON'T: `Task { while true { await tick() } }`.
- **Annotate UI-facing state and types with `@MainActor`** instead of manual `DispatchQueue.main.async` hops.
- **Do not capture non-`Sendable` state in a `@Sendable` closure** (those passed to `Task`, `Task.detached`, `addTask`). Capture a `Sendable` copy instead.
