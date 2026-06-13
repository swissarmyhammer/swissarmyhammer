//! Shared test fixtures for code that exercises the
//! [`SessionStore`](crate::SessionStore).
//!
//! The store resolves its directory under `$XDG_STATE_HOME`, so every test
//! that persists or lists session records must isolate that directory. This
//! module hosts the one canonical [`StateDirGuard`] for the whole workspace —
//! downstream agent crates (claude-agent, llama-agent) re-export it instead
//! of carrying per-crate copies that would drift.
//!
//! Compiled for this crate's own unit tests, and exported to downstream
//! crates' tests via the `test-support` cargo feature (the same pattern
//! acp-conformance uses for its mock-agent harness).

/// RAII guard that points `XDG_STATE_HOME` at a fresh temp directory for the
/// lifetime of the guard, restoring the previous value on drop.
///
/// Tests that persist [`SessionRecord`](crate::SessionRecord)s must isolate
/// the state directory so they neither pollute the developer's real state
/// tree nor observe records left by other tests. Hold the guard for the
/// whole test body.
///
/// Mutating a process-wide environment variable is racy across threads, so
/// every test holding a `StateDirGuard` must also be `#[serial]`.
#[derive(Debug)]
pub struct StateDirGuard {
    _temp: tempfile::TempDir,
    previous: Option<std::ffi::OsString>,
}

impl StateDirGuard {
    /// Create a fresh temp directory and point `XDG_STATE_HOME` at it.
    pub fn new() -> Self {
        let temp = tempfile::TempDir::new().expect("temp dir for XDG_STATE_HOME");
        let previous = std::env::var_os("XDG_STATE_HOME");
        // SAFETY: callers are `#[serial]`, so no other thread reads or writes
        // the env var concurrently; the previous value is restored in `Drop`.
        std::env::set_var("XDG_STATE_HOME", temp.path());
        Self {
            _temp: temp,
            previous,
        }
    }
}

impl Default for StateDirGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for StateDirGuard {
    fn drop(&mut self) {
        // SAFETY: see `StateDirGuard::new` — callers are `#[serial]`.
        match self.previous.take() {
            Some(value) => std::env::set_var("XDG_STATE_HOME", value),
            None => std::env::remove_var("XDG_STATE_HOME"),
        }
    }
}
