//! The local multi-agent review pipeline's shared data model.
//!
//! This module is the home for the types that flow through the review pipeline
//! end to end: fleet agents emit [`types::Finding`](crate::review::types::Finding)s,
//! the verifier wraps them in
//! [`types::VerifiedFinding`](crate::review::types::VerifiedFinding)s, and
//! synthesis renders them.
//! [`types::parse_findings`](crate::review::types::parse_findings)
//! turns a raw agent response back into a `Vec<Finding>`.
//!
//! [`probes`](crate::review::probes) is the engine-run code_context probe catalog + runner: the
//! ground-truth evidence the engine injects into review (rather than asking the
//! agent to call a tool it might skip).

pub mod drive;
pub mod fleet;
pub mod ignore;
pub mod probes;
pub mod scope;
pub mod synthesize;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
pub mod tool_output;
pub mod types;
pub mod verify;

pub use drive::run_review_over_agent;
pub use fleet::{
    prompt_framing_bytes, render_file_payload, render_fleet_prompt, render_run_prime,
    render_validator_suffix, rendered_file_block_bytes, run_fleet, unpin_prefix_session,
    FleetConfig, FleetOutcome, ReviewProgressEvent, ReviewProgressSender, AGENT_PROMPT_CAP,
    DEFAULT_BATCH_SIZE,
};
pub use probes::{
    probe_exists, run_probes, ChangeEntry, FileChange, ProbeCatalogEntry, ProbeKind, ProbeOp,
    ProbeResult, ProbeResults, ProbeRow, CATALOG,
};
pub use scope::{
    batch_work_list, scope_review, FileWork, LineAnnotation, ProbeNames, RuleNames, Scope,
    ScopeSpec, SkippedFile, ValidatorWork, WorkList,
};
pub use synthesize::{
    run_review, synthesize, FleetTally, ReviewCounts, ReviewReport, TasksAttempted, TasksFailed,
};
pub use tool_output::parse_tool_stdout;
pub use types::{parse_findings, Finding, RefutingLayer, VerifiedFinding};
pub use verify::{
    render_verify_prompt, run_guard, verify_findings, Candidate, GuardOutcome, VerifyOutcome,
};
