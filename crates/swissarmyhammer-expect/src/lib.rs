//! The `expect` engine's pure domain model.
//!
//! `expect` runs human-authored behavioral expectations against a running system
//! and renders a verdict that a human must approve every change to. This crate
//! is the bottom layer: the pure, serde-driven data model (`ideas/expect.md`
//! §"The Verdict Ladder") that every higher layer builds on. The model itself
//! ([`types`], [`spec`], [`config`]) has no IO and no agent — and deliberately no
//! dependency on the tool layer.
//!
//! The one exception is [`drive`], the ACP delegation seam: it borrows a live
//! agent (supplied by the tool layer as a `DynConnectTo<Client>`) to *drive* the
//! system under test, while the verdict it feeds stays deterministic and inside
//! the pure model. The seam constructs no agent itself, reusing review's
//! `AgentPool` and ACP wiring rather than re-deriving them.
//!
//! See [`types`] for the domain model ([`Observation`], [`ExpectationVerdict`],
//! and the closed enums), [`error`] for [`ExpectError`], and [`drive`] for the
//! agent seam.
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

pub mod assertion;
pub mod check;
pub mod config;
pub mod create;
pub mod doctor;
pub mod drive;
pub mod error;
pub mod evaluate;
pub mod grader;
pub mod ledger;
pub mod loader;
pub mod observe;
pub mod replay;
pub mod spec;
pub mod surface;
pub mod surfaces;
pub mod types;

pub use assertion::{
    compile, A11ySelector, AssertOp, AssertionOutcome, BoundValue, CompileError, CompiledAssertion,
    Expected, Locator, Stream,
};
pub use check::{
    check, CheckEntry, CheckOptions, CheckReport, CheckStatus, CHECK_EXIT_FAILED,
    CHECK_EXIT_MALFORMED, CHECK_EXIT_OK,
};
pub use config::{
    find_expect_dir, AgentConfig, ApprovalConfig, EmbedderConfig, ExpectConfig, Granularity,
    ModelConfig, OnMissing, ProvisionConfig, ReliabilityConfig,
};
pub use create::{
    create, parse_draft, render_authoring_goal, render_schema, AuthoringRequest, CreateOutcome,
    CreateSource, DraftSpec, Provenance, RepairContext, SpecAuthor,
};
pub use doctor::{diagnose, render, DiagnosticStatus, DoctorFacts, FieldDiagnostic};
pub use drive::{
    build_driver_goal, drive_and_revalidate, observe_with_driver, run_expect_over_agent,
    AcpGoalDriver, DrivenObservation, DriverHandle, DriverTurn, ExpectScope, GoalDriver,
};
pub use error::ExpectError;
pub use evaluate::{
    evaluate, evaluate_assertion, evaluate_repeated, evaluate_spec, evaluate_tiered,
    similarity_threshold, Escalation, TextEmbedder, TieredVerdict, ToleranceAssertion,
    ToleranceBand, STRUCTURAL_DRIFT_REASON, TOLERANCE_DRIFT_REASON,
};
pub use grader::{
    Grade, GradeRequest, Grader, JudgmentAssertion, JudgmentContext, GRADER_IS_DRIVER_REASON,
    JUDGMENT_DRIFT_REASON, PANEL_DISAGREEMENT_REASON,
};
pub use ledger::{
    approval_diff, approval_status, approve, compare, decide_approval, delete_expectation,
    delete_golden, delete_observation, ledger_entry, ledger_queue, ledger_state, read_golden,
    spec_hash, write_golden, ApprovalBinding, ApprovalDecision, ApprovalStatus, ApproveError,
    ApproveMode, Artifact, CriterionComparison, DeletionSummary, Golden, GradingPins,
    LedgerComparison, LedgerEntry, RemovedArtifact, Scrubber, ScrubberSet, BINDING_ARROW,
};
pub use loader::{ExpectationLoader, RawSpec};
pub use observe::{
    cache_path, drives_mechanically, golden_path, observe, observe_repeated, received_path,
    resolved_repeat, spec_path, write_received, ObserveConfig, FINAL_CHECKPOINT,
};
pub use replay::{
    CachedAction, ReplayCache, ReplayKey, ReplaySource, ResolvedAction, MAX_REPLAY_DRIFT,
};
pub use spec::{Criterion, Expectation, Frontmatter, Isolation, ReliabilityPolicy, Setup};
pub use surface::browser::{BrowserAction, BrowserAdapter, BrowserSut};
pub use surface::cli::{CliAdapter, CliCommands, CliSut};
pub use surface::db::{DbAdapter, DbSut};
pub use surface::file::{FileAdapter, FileSut};
pub use surface::http::{HttpAdapter, HttpSut};
pub use surface::SurfaceAdapter;
pub use surfaces::SurfaceInfo;
pub use types::{
    A11yNode, Checkpoint, CliState, CriterionStatus, CriterionVerdict, DbState, Evidence,
    ExpectationVerdict, FileState, HttpState, LedgerState, Observation, Reliability, Surface,
    SurfaceState, Trajectory, VerdictTier,
};
