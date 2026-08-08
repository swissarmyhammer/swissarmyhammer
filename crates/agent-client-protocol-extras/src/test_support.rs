//! Shared test fixtures for code that exercises the
//! [`SessionStore`](crate::SessionStore).
//!
//! The store resolves its directory under `$XDG_STATE_HOME`, so every test
//! that persists or lists session records must isolate that directory. This
//! module hosts the one canonical [`StateDirGuard`] for the whole workspace —
//! downstream agent crates (claude-agent) re-export it instead of carrying
//! per-crate copies that would drift.
//!
//! Compiled for this crate's own unit tests, and exported to downstream
//! crates' tests via the `test-support` cargo feature (the same pattern
//! acp-conformance uses for its mock-agent harness).

use swissarmyhammer_common::test_utils::EnvVarGuard;

/// RAII guard that points `XDG_STATE_HOME` at a fresh temp directory for the
/// lifetime of the guard, restoring the previous value on drop.
///
/// Tests that persist [`SessionRecord`](crate::SessionRecord)s must isolate
/// the state directory so they neither pollute the developer's real state
/// tree nor observe records left by other tests. Hold the guard for the
/// whole test body.
///
/// Mutating a process-wide environment variable is racy across threads, so
/// every test holding a `StateDirGuard` must also be `#[serial]`. The restore
/// itself is the shared [`EnvVarGuard`], so this guard only owns the temp
/// directory and the choice of variable.
#[derive(Debug)]
pub struct StateDirGuard {
    /// Restores `XDG_STATE_HOME` on drop, before `_temp` deletes the
    /// directory it pointed at. Held for that effect alone, never read.
    _state_dir: EnvVarGuard,
    _temp: tempfile::TempDir,
}

impl StateDirGuard {
    /// Create a fresh temp directory and point `XDG_STATE_HOME` at it.
    pub fn new() -> Self {
        let temp = tempfile::TempDir::new().expect("temp dir for XDG_STATE_HOME");
        Self {
            _state_dir: EnvVarGuard::set("XDG_STATE_HOME", temp.path()),
            _temp: temp,
        }
    }
}

impl Default for StateDirGuard {
    fn default() -> Self {
        Self::new()
    }
}
