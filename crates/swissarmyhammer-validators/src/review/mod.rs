//! The local multi-agent review pipeline's shared data model.
//!
//! This module is the home for the types that flow through the review pipeline
//! end to end: fleet agents emit [`types::Finding`]s, the verifier wraps them in
//! [`types::VerifiedFinding`]s, and synthesis renders them. [`types::parse_findings`]
//! turns a raw agent response back into a `Vec<Finding>`.
//!
//! [`probes`] is the engine-run code_context probe catalog + runner: the
//! ground-truth evidence the engine injects into review (rather than asking the
//! agent to call a tool it might skip).

pub mod drive;
pub mod fleet;
pub mod probes;
pub mod scope;
pub mod synthesize;
pub mod types;
pub mod verify;

pub use drive::run_review_over_agent;
pub use fleet::{render_fleet_prompt, run_fleet, FleetConfig, DEFAULT_BATCH_SIZE};
pub use probes::{
    probe_exists, run_probes, ChangeEntry, FileChange, ProbeCatalogEntry, ProbeKind, ProbeOp,
    ProbeResult, ProbeResults, ProbeRow, CATALOG,
};
pub use scope::{scope_review, FileWork, Scope, ScopeSpec, ValidatorWork, WorkList};
pub use synthesize::{run_review, synthesize, ReviewCounts, ReviewReport};
pub use types::{parse_findings, Finding, RefutingLayer, Severity, VerifiedFinding};
pub use verify::{
    render_verify_prompt, run_guard, verify_findings, Candidate, GuardOutcome, VerifyOutcome,
};
