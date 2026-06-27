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

pub mod cli;

use std::path::Path;

use crate::error::ExpectError;
use crate::spec::Setup;
use crate::types::SurfaceState;

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
