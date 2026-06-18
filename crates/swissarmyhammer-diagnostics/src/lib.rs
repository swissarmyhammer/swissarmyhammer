//! Model-free diagnostics core for SwissArmyHammer.
//!
//! This crate holds the shared, model-free pieces of the diagnostics feature:
//! the report/record/config types, the pure mapping from
//! [`lsp_types::Diagnostic`] to a [`DiagnosticRecord`], the single
//! diagnosable-language predicate, and the [`settle`](settle::settle) engine
//! that debounces a server's diagnostic re-flows into one settled set. It owns
//! **no** LSP client — it sits on top of the shared session/supervisor in
//! [`swissarmyhammer_lsp`], subscribing to that session's diagnostics fan-out.
//!
//! It is a crate (not a module of a consumer) because it has two consumers — the
//! `diagnostics` MCP tool and the inline-on-edit fold-in — and belongs to
//! neither. Nothing here is persisted to disk: config and any report are derived
//! state.
//!
//! ## Severity
//!
//! The canonical [`DiagnosticSeverity`] is defined once in
//! [`swissarmyhammer_lsp`] and re-exported here so this crate and
//! `swissarmyhammer-code-context` share a single severity type.

/// Cross-process diagnostics fan-out over the leader-election pub/sub bus.
pub mod bus;
pub mod config;
pub mod diagnose;
pub mod language;
pub mod record;
pub mod request_api;
pub mod settle;
/// The leader-owned diagnostics file watcher (one per workdir).
pub mod watcher;

#[cfg(test)]
pub(crate) mod test_support;

pub use bus::{
    fan_out_over_bus, fan_out_to_bus, message_from_update, subscribe_diagnostics_over_bus,
    DiagnosticsBusMessage, DIAGNOSTICS_TOPIC,
};
pub use config::{
    DiagnosticsConfig, DEFAULT_PER_REPORT_CAP, DEFAULT_SETTLE_HARD_TIMEOUT, DEFAULT_SETTLE_WINDOW,
};
pub use diagnose::{
    diagnose, diagnose_with_outcome, BlastRadiusDependents, Dependents, DiagnoseOutcome,
    PrecomputedDependents,
};
pub use language::is_diagnosable;
pub use record::{map, Counts, DiagnosticRecord, DiagnosticsReport, Range};
pub use request_api::{
    dispatch, serve_session_requests, SessionRequestClient, METHOD_DIAGNOSE, METHOD_LSP_REQUEST,
};
pub use settle::{settle, settle_stream, SettleOutcome, Timer, TokioTimer};
pub use swissarmyhammer_lsp::DiagnosticSeverity;
pub use watcher::{
    refresh_changed_files, refresh_file, start_diagnostics_watcher, SessionRoute,
    DIAGNOSTICS_WATCH_DEBOUNCE,
};
