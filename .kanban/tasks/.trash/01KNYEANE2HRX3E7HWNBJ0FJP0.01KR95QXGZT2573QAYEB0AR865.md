---
assignees:
- claude-code
position_column: todo
position_ordinal: '8580'
title: Introduce ChildHandle trait to unlock LspDaemon error-path coverage
---
swissarmyhammer-lsp/src/daemon.rs

**Background:** Commit 9f35d2acc raised daemon.rs test coverage but left several error branches uncovered because they all hinge on OS syscall failures from `tokio::process::Child`:

| Line(s) | Branch | Why unreachable |
|---|---|---|
| 183-186 | `Command::spawn` Err | Real commands that pass `which::which` will always spawn |
| 247-258 | `ChildStdin::into_owned_fd` Err | Requires OS-level dup/fcntl failure |
| 315-323 | `Child::try_wait` Err | Only errors on OS wait4 syscall failure |
| 395-396 | `graceful_teardown` Ok(Err(e)) | `Child::wait` only errors on OS wait4 failure |
| 540 | `graceful_teardown` wait() Err → ShutdownFailed | Same as above |

Behaviorally these are identical to branches that ARE covered (they log and transition to the same state), so observable behavior is verified. But line coverage is permanently stuck without a test seam.

**Approach:** introduce a minimal trait-bounded child handle that production code uses in place of `tokio::process::Child` directly.

```rust
/// Minimal abstraction over tokio::process::Child for testability.
pub(crate) trait ChildHandle: Send + Sync + 'static {
    fn try_wait(&mut self) -> std::io::Result<Option<std::process::ExitStatus>>;
    async fn wait(&mut self) -> std::io::Result<std::process::ExitStatus>;
    async fn kill(&mut self) -> std::io::Result<()>;
    fn take_stdin(&mut self) -> Option<tokio::process::ChildStdin>;
    fn take_stdout(&mut self) -> Option<tokio::process::ChildStdout>;
    fn take_stderr(&mut self) -> Option<tokio::process::ChildStderr>;
    fn id(&self) -> Option<u32>;
}

impl ChildHandle for tokio::process::Child { /* delegate */ }
```

In tests:
```rust
struct MockChild {
    try_wait_result: Option<std::io::Result<Option<ExitStatus>>>,
    wait_result: Option<std::io::Result<ExitStatus>>,
    ...
}
impl ChildHandle for MockChild { /* configurable */ }
```

Then `LspDaemon::child` becomes `Box<dyn ChildHandle>` (or keeps its concrete type with a test-only constructor that accepts a mock).

**Scope considerations:**
- This is a non-trivial refactor — changes every place daemon.rs touches `self.child`.
- Adds a small runtime cost (dynamic dispatch on every child interaction), though probably unmeasurable.
- The `async fn` in trait requires `async-trait` crate or GAT support.
- The `Command::spawn` Err branch also needs a similar abstraction over `tokio::process::Command` — consider whether to extract a `ProcessLauncher` trait too, or whether that path should stay uncovered.

**Alternative (lighter):** instead of a full trait, add a `cfg(test)` hook that lets tests inject a pre-configured `Child` state. For example, `LspDaemon::new_with_child(...)` available only under test. This is less invasive but still requires production code changes.

**Coverage impact:** unlocks ~10-15 uncovered lines in daemon.rs error paths.

**Recommendation:** Only take this on if coverage metrics or a specific bug motivate it. The skipped branches are all behaviorally validated via the happy-path counterparts. Consider marking these lines with `#[cfg_attr(coverage, coverage(off))]` or an `#[allow(dead_code)]`-style annotation as a lightweight alternative to full mocking.

#coverage-gap