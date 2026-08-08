//! Shared fixtures for this crate's unit tests.
//!
//! Compiled only under `#[cfg(test)]` (see the module declaration in
//! `lib.rs`); integration tests under `tests/` are a separate crate target
//! and import the same fixtures from `agent_client_protocol_extras`
//! directly.

// The canonical XDG_STATE_HOME isolation guard lives next to the
// `SessionStore` it isolates; re-export it rather than carrying a per-crate
// copy. Callers must be `#[serial]` — see its docs.
pub(crate) use agent_client_protocol_extras::test_support::StateDirGuard;

/// Collect a spawned command's arguments as owned strings for assertions.
///
/// Shared by `claude_process`'s and `session_fork`'s test modules so both can
/// assert on the literal argv `ClaudeProcess::build_base_command` assembles
/// without duplicating the extraction logic.
pub(crate) fn command_args(command: &tokio::process::Command) -> Vec<String> {
    command
        .as_std()
        .get_args()
        .map(|s| s.to_string_lossy().to_string())
        .collect()
}

/// Restores `PATH` on drop.
///
/// `ClaudeProcess` spawns the CLI as the bare program name `claude`, so the
/// only seam a test has for substituting a scripted stand-in is `PATH`. This
/// guard prepends a directory holding that stand-in and puts `PATH` back
/// afterwards.
///
/// `PATH` is process-wide, so every test holding a `PathGuard` must also be
/// `#[serial]` — the same default group the [`StateDirGuard`] callers use, so
/// a `PATH` mutator and an `XDG_STATE_HOME` mutator can never interleave.
#[derive(Debug)]
pub(crate) struct PathGuard {
    original: Option<std::ffi::OsString>,
}

impl PathGuard {
    /// Prepend `dir` to `PATH` for the guard's lifetime, so a program name
    /// with no path separator (e.g. `"claude"`) resolves to a binary placed
    /// in `dir`.
    pub(crate) fn prepend(dir: &std::path::Path) -> Self {
        let original = std::env::var_os("PATH");
        let mut new_path = std::ffi::OsString::from(dir);
        if let Some(existing) = &original {
            new_path.push(":");
            new_path.push(existing);
        }
        std::env::set_var("PATH", &new_path);
        Self { original }
    }
}

impl Drop for PathGuard {
    fn drop(&mut self) {
        match &self.original {
            Some(p) => std::env::set_var("PATH", p),
            None => std::env::remove_var("PATH"),
        }
    }
}
