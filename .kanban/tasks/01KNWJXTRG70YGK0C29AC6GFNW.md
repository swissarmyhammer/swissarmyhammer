---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffa180
title: Add tests for shelltool-cli main::FileWriterGuard and dispatch_command
---
shelltool-cli/src/main.rs

Coverage: 0/93 (0.0%)

The entire `main.rs` is uncovered — no tests exist in this file. There are two clearly testable items and one that is not.

**Actionable 1 — `FileWriterGuard` (lines 31-55):**
```rust
struct FileWriterGuard {
    file: Arc<Mutex<std::fs::File>>,
}

impl FileWriterGuard {
    fn new(file: Arc<Mutex<std::fs::File>>) -> Self { Self { file } }
}

impl std::io::Write for FileWriterGuard {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> { ... }
    fn flush(&mut self) -> std::io::Result<()> { ... }
}
```
Add a `#[cfg(test)] mod tests` block to `main.rs`. Create a tempfile, wrap it in `Arc::new(Mutex::new(f))`, construct a `FileWriterGuard::new(...)`, call `.write(b"hello")` and `.flush()`, then re-read the file and assert contents. Both `write` and `flush` hit the `sync_all` path.

Note: `FileWriterGuard` is currently private. Either make it `pub(crate)` with a doc-comment, or test it via an in-file `#[cfg(test)] mod tests` which can access private items.

**Actionable 2 — `dispatch_command` (lines 119-168):**
```rust
async fn dispatch_command(cli: Cli) -> i32 { match cli.command { ... } }
```
Hits each `Commands` arm:
- `Commands::Init { target: InstallTarget::Local }` — avoid `Project`/`User` scope to keep test hermetic (`Local` writes to a project-level file). Run in a tempdir as cwd.
- `Commands::Deinit { target: InstallTarget::Local }` — same tempdir trick.
- `Commands::Doctor { verbose: false }` — calls through to `doctor::run_doctor` which is already tested; this adds dispatch-arm coverage.
- `Commands::Serve` — **skip**, `run_serve` blocks on stdio and cannot be unit-tested without a transport harness.

Each arm test should construct a `Cli { debug: false, command: ... }` directly (bypass `Cli::parse`) and `.await` the result, asserting the exit code.

**Not actionable:** `async fn main()` itself (lines 57-114) — tracing init, file logging setup, `std::process::exit`. These only run once per process and cannot be meaningfully unit-tested.

Also `dispatch_command` is currently private and not an item of a module — move it into a small `mod dispatch` or just make it `pub(crate)` and call via `#[cfg(test)] mod tests`. #coverage-gap