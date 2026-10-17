//! Test-support utilities shared by this crate's unit tests **and** its
//! integration tests (which compile as a separate crate and so cannot see
//! `#[cfg(test)]`-only items).
//!
//! [`KillOnDrop`] is the single kill-on-drop guard for spawned child
//! processes. It exists so that no test — unit or integration — ever leaks a
//! spawned LSP server (mock or real `rust-analyzer`) as a `PPID=1` orphan, and
//! so there is exactly one such guard in the crate rather than a copy per test
//! module.
//!
//! [`symlinked_root`] makes a workspace root whose canonical form is different
//! from the root. The tests of this crate and the tests of the file watchers in
//! other crates use it, so all of them make the same fixture.

use std::path::{Path, PathBuf};

/// Make a workspace root that is a symlink to a real directory in `outer`.
///
/// The canonical form of the returned root is different from the root, the
/// same as `/var/...` and `/private/var/...` on macOS. The function returns
/// the root and its canonical form.
///
/// # Panics
///
/// Panics when the directory or the symlink cannot be made, or when the
/// canonical form of the root is the same as the root. This is a test fixture,
/// so a failure is a broken precondition of the test.
pub fn symlinked_root(outer: &Path) -> (PathBuf, PathBuf) {
    let real = outer.join("real");
    std::fs::create_dir_all(&real).unwrap();
    let link = outer.join("link");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&real, &link).unwrap();
    #[cfg(not(unix))]
    std::os::windows::fs::symlink_dir(&real, &link).unwrap();
    let canonical = link.canonicalize().unwrap();
    assert_ne!(
        canonical, link,
        "precondition: the root must not be canonical"
    );
    (link, canonical)
}

/// A spawned child process that is killed and reaped when it goes out of scope.
///
/// LSP test servers block on `stdin.readline()` until they have read exactly as
/// many messages as they were scripted with (mock servers), or run indefinitely
/// (a real `rust-analyzer`). If the code under test sends fewer messages than a
/// mock expects — which happens whenever the live-LSP wire protocol changes
/// (e.g. dropping a per-request `didClose`) — the child parks on that read
/// forever. A test that then *blocks* on the child (the old `child.wait()`
/// pattern) deadlocks, and because libtest waits for every spawned test thread
/// to report completion, one parked test hangs the entire `cargo test` run
/// indefinitely. A real `rust-analyzer` that a test forgets to kill — or that a
/// panicking assertion skips the cleanup for — reparents to launchd as an
/// orphan.
///
/// This guard removes both failure classes: tests never wait on the child, and
/// the child is always reaped. On drop it sends `SIGKILL` (which cannot block)
/// and then reaps the zombie, so a message-count mismatch surfaces as a normal
/// test assertion instead of a hang, a panic before manual cleanup cannot leak,
/// and no spawned process is ever orphaned.
pub struct KillOnDrop(pub std::process::Child);

impl KillOnDrop {
    /// Wrap an already-spawned child in the kill-on-drop guard.
    pub fn new(child: std::process::Child) -> Self {
        KillOnDrop(child)
    }
}

impl std::ops::Deref for KillOnDrop {
    type Target = std::process::Child;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for KillOnDrop {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        // kill() is non-blocking; the following wait() only reaps the
        // already-terminating process, so neither call can deadlock the way a
        // bare wait() on a parked mock would.
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
