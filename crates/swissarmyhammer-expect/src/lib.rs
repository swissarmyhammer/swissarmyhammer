//! The `expect` engine's pure domain model.
//!
//! `expect` runs human-authored behavioral expectations against a running system
//! and renders a verdict that a human must approve every change to. This crate
//! is the bottom layer: the pure, serde-driven data model (`ideas/expect.md`
//! §"The Verdict Ladder") that every higher layer builds on. It has no IO, no
//! system access, no agent, and — deliberately — no dependency on the tool layer.
//!
//! See [`types`] for the domain model ([`Observation`], [`ExpectationVerdict`],
//! and the closed enums) and [`error`] for [`ExpectError`].

pub mod error;
pub mod types;

pub use error::ExpectError;
pub use types::{
    Checkpoint, CliState, CriterionStatus, CriterionVerdict, Evidence, ExpectationVerdict,
    LedgerState, Observation, Reliability, Surface, SurfaceState, Trajectory, VerdictTier,
};
