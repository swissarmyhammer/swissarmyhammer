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
//!
//! # Examples
//!
//! Deserialize an [`Observation`] from its wire JSON, then build a
//! [`Reliability`] and check whether `pass^k` is satisfied:
//!
//! ```
//! use swissarmyhammer_expect::{Observation, Reliability};
//! use serde_json::json;
//!
//! let observation: Observation = serde_json::from_value(json!({
//!     "path": "src/checkout/coupon",
//!     "checkpoints": [{
//!         "after": "apply SAVE10",
//!         "state": {
//!             "kind": "cli",
//!             "stdout": "Total: $40\n",
//!             "stderr": "",
//!             "exit_code": 0,
//!             "files": {}
//!         },
//!         "duration_ms": 120
//!     }],
//!     "trajectory": { "steps": ["ran: checkout --apply SAVE10"] }
//! }))?;
//! assert_eq!(observation.path, "src/checkout/coupon");
//! assert_eq!(observation.checkpoints.len(), 1);
//!
//! let reliability = Reliability { required: 3, runs: vec![true, true, true] };
//! assert!(reliability.satisfied());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod error;
pub mod types;

pub use error::ExpectError;
pub use types::{
    Checkpoint, CliState, CriterionStatus, CriterionVerdict, Evidence, ExpectationVerdict,
    LedgerState, Observation, Reliability, Surface, SurfaceState, Trajectory, VerdictTier,
};
