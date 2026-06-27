//! Surface adapters: how `expect` provisions, drives, observes, and tears down
//! the system under test.
//!
//! `ideas/expect.md` §"Surface adapters" defines a surface as a built-in engine
//! that both **drives** (causes the `When` transition) and **observes** (reads
//! the authoritative checkpoint state), in-process and mechanical — no Node, no
//! Python, no external test harness. §"Provisioning and Isolation" adds that
//! `expect` **owns the SUT lifecycle**: it provisions a fresh instance, drives
//! it, and tears it down, so a `check` gates *this code, built now*.
//!
//! [`SurfaceAdapter`] is the contract every surface implements. The lifecycle is
//! always the same four steps — provision → drive → observe → teardown — but the
//! *handle* each adapter carries between them differs (a built binary for cli, a
//! launched server for http, a scratch database for db), so the handle is an
//! [associated type](SurfaceAdapter::ProvisionedSut) the adapter owns rather than
//! a shared enum. New adapters slot in by implementing the trait with their own
//! handle type; nothing in the trait is cli-specific.
//!
//! The first adapter is [`cli`](crate::surface::cli) — the deterministic,
//! no-agent path: build the binary, run argv, read stdout/stderr/exit/files.

pub mod a11y;
pub mod browser;
pub mod cli;
pub mod db;
pub mod file;
pub mod gui;
pub mod http;

use std::path::{Path, PathBuf};

use crate::error::ExpectError;
use crate::spec::Setup;
use crate::types::SurfaceState;

/// Safe-join a relative `name` under `base`, rejecting any name that could escape
/// `base` via an absolute path or a `..` component.
///
/// Surface adapters join spec-supplied names (cli output files, file-surface
/// write targets) onto a sandbox root; an absolute path or a parent-directory
/// component would let a spec read or write outside it (e.g. `../../etc/passwd`).
/// A name is accepted only when it is relative and contains no parent-directory
/// component, which guarantees the join stays under `base`. This is the single
/// source of truth for the traversal guard shared by every path-bearing adapter.
///
/// # Errors
///
/// Returns [`ExpectError::Surface`] when `name` is absolute or contains a `..`
/// component.
pub(crate) fn safe_join(base: &Path, name: &str) -> Result<PathBuf, ExpectError> {
    let candidate = Path::new(name);
    if candidate.is_absolute() {
        return Err(ExpectError::Surface(format!(
            "path `{name}` must be relative to the sandbox root"
        )));
    }
    if candidate
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(ExpectError::Surface(format!(
            "path `{name}` must not escape the sandbox root with `..`"
        )));
    }
    Ok(base.join(candidate))
}

/// The ordered provisioning commands of a [`Setup`] declaration as a slice — a
/// single command becomes a one-element slice, a list passes through.
///
/// Shared by the db (fixture SQL) and file (fixture writes) adapters so a spec's
/// `setup:` is read uniformly across surfaces.
pub(crate) fn setup_commands(setup: &Setup) -> &[String] {
    match setup {
        Setup::Command(command) => std::slice::from_ref(command),
        Setup::Commands(commands) => commands.as_slice(),
    }
}

/// The contract a surface adapter implements to run the `expect` lifecycle
/// against one surface (cli, http, db, …).
///
/// The four methods are the lifecycle from `ideas/expect.md`
/// §"provision → arrange → act → observe → teardown": [`provision`] stands the
/// SUT up, [`drive`] causes each `When` transition, [`observe`] reads the
/// authoritative state for a checkpoint, and [`teardown`] releases what
/// `provision` created. The handle that threads through them is the
/// adapter-owned [`ProvisionedSut`], so each surface keeps exactly the state it
/// needs (and no more) without a shared, cli-shaped enum.
///
/// [`provision`]: SurfaceAdapter::provision
/// [`drive`]: SurfaceAdapter::drive
/// [`observe`]: SurfaceAdapter::observe
/// [`teardown`]: SurfaceAdapter::teardown
/// [`ProvisionedSut`]: SurfaceAdapter::ProvisionedSut
pub trait SurfaceAdapter {
    /// The provisioned system-under-test handle this adapter owns between
    /// [`provision`](SurfaceAdapter::provision) and
    /// [`teardown`](SurfaceAdapter::teardown).
    type ProvisionedSut;

    /// Stand the SUT up so it is ready to be driven.
    ///
    /// `setup` is the spec's optional [`Setup`] declaration: when present it
    /// overrides how the SUT is built and launched; when absent the adapter
    /// falls back to commands it derives from the detected project type at
    /// `repo_root`.
    ///
    /// # Errors
    ///
    /// Returns [`ExpectError`] when provisioning fails — a build step exits
    /// non-zero, the project type cannot be detected, or a command cannot be
    /// spawned.
    fn provision(
        &self,
        setup: Option<&Setup>,
        repo_root: &Path,
    ) -> Result<Self::ProvisionedSut, ExpectError>;

    /// Cause one `When` transition against the provisioned SUT.
    ///
    /// `when_step` is the action to perform, in the surface's own dialect (for
    /// cli, the argv to run).
    ///
    /// # Errors
    ///
    /// Returns [`ExpectError`] when the action cannot be performed or exceeds
    /// the adapter's timeout ([`ExpectError::Timeout`]).
    fn drive(&self, sut: &mut Self::ProvisionedSut, when_step: &str) -> Result<(), ExpectError>;

    /// Read the SUT's authoritative state into a [`SurfaceState`] checkpoint.
    ///
    /// # Errors
    ///
    /// Returns [`ExpectError`] when the state cannot be read.
    fn observe(&self, sut: &Self::ProvisionedSut) -> Result<SurfaceState, ExpectError>;

    /// Release everything [`provision`](SurfaceAdapter::provision) created,
    /// consuming the handle.
    ///
    /// # Errors
    ///
    /// Returns [`ExpectError`] when scratch state cannot be cleaned up.
    fn teardown(&self, sut: Self::ProvisionedSut) -> Result<(), ExpectError>;

    /// Whether the adapter can resolve `when_step` into a concrete action and
    /// drive it mechanically, with no agent interpretation.
    ///
    /// This is the gate the agent-fallback path
    /// ([`observe_with_driver`](crate::drive::observe_with_driver)) consults
    /// before delegating to a scoped subagent. Deterministic surfaces resolve
    /// every concrete step — a cli run is always an argv, an http call always a
    /// request — so the default is `true` and such a run never reaches the
    /// agent. A surface whose locator or action fails to bind (for example a
    /// renamed browser/gui accessibility control) returns `false` for that step,
    /// which routes it through the subagent as the documented runtime fallback
    /// (`ideas/expect.md` §"The Check Loop").
    fn resolves_mechanically(&self, _when_step: &str) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_join_accepts_relative_names_and_rejects_traversal() {
        let base = Path::new("/sandbox");
        assert_eq!(safe_join(base, "a.txt").unwrap(), base.join("a.txt"));
        assert_eq!(
            safe_join(base, "nested/dir/a.txt").unwrap(),
            base.join("nested/dir/a.txt")
        );
        assert!(matches!(
            safe_join(base, "../escape.txt"),
            Err(ExpectError::Surface(_))
        ));
        assert!(matches!(
            safe_join(base, "nested/../../escape.txt"),
            Err(ExpectError::Surface(_))
        ));
        assert!(matches!(
            safe_join(base, "/etc/passwd"),
            Err(ExpectError::Surface(_))
        ));
    }

    #[test]
    fn setup_commands_flattens_single_and_list_forms() {
        assert_eq!(
            setup_commands(&Setup::Command("one".to_string())),
            &["one".to_string()]
        );
        assert_eq!(
            setup_commands(&Setup::Commands(vec!["a".to_string(), "b".to_string()])),
            &["a".to_string(), "b".to_string()]
        );
    }
}
