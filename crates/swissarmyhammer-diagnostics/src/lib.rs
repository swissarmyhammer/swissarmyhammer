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

pub mod config;
pub mod language;
pub mod record;
pub mod settle;

pub use config::{
    DiagnosticsConfig, DEFAULT_PER_REPORT_CAP, DEFAULT_SETTLE_HARD_TIMEOUT, DEFAULT_SETTLE_WINDOW,
};
pub use language::is_diagnosable;
pub use record::{map, Counts, DiagnosticRecord, DiagnosticsReport, Range};
pub use settle::{settle, settle_stream, SettleOutcome, Timer, TokioTimer};
pub use swissarmyhammer_lsp::DiagnosticSeverity;
