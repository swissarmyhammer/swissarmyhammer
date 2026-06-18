//! Inline-on-edit diagnostics fold-in.
//!
//! When a file-mutating MCP tool (`files` `write`/`edit`, or any future mutator)
//! changes a file, its **own** tool result should carry the diagnostics for what
//! it changed — no hook, no separate `diagnostics` call. A tool's return value
//! always reaches the model, so folding diagnostics into that result is the most
//! direct path to "the model sees what broke the moment it edits".
//!
//! The mechanism is a typed side-channel, not content parsing: a mutator records
//! the absolute paths it changed via
//! [`ToolContext::record_mutated_path`](crate::mcp::tool_registry::ToolContext::record_mutated_path),
//! and the single dispatch chokepoint in [`server`](crate::mcp::server) calls
//! [`fold_in_diagnostics`] after `execute`. The chokepoint never inspects the
//! tool's `CallToolResult` content to learn what changed.
//!
//! Severity/scope is delegated to the shared diagnostics core
//! ([`swissarmyhammer_diagnostics`]): the edited file is always reported, plus
//! only the one-hop dependents that *broke*, capped — never a project-wide dump.
//! Gating on diagnosable language uses the one shared
//! [`is_diagnosable`](swissarmyhammer_diagnostics::is_diagnosable) predicate, so
//! `.md`/`.txt` edits attach nothing. A non-quiescent analysis yields a
//! `pending` marker rather than blocking.

use std::collections::HashSet;
use std::path::PathBuf;

use rmcp::model::{CallToolResult, Content};
use rmcp::ErrorData as McpError;
use swissarmyhammer_common::utils::find_git_repository_root_from;
use swissarmyhammer_diagnostics::{is_diagnosable, DiagnoseOutcome, DiagnosticsConfig};

use crate::mcp::tool_registry::ToolContext;
use crate::mcp::tools::diagnostics::produce_outcome;

/// Produces a diagnostics outcome for a set of mutated paths.
///
/// Abstracted as a trait so the fold-in's gating and JSON-folding logic can be
/// unit-tested with a stub — no live LSP server, no index. The production
/// implementation, [`LiveDiagnoser`], drives the shared diagnostics core.
#[async_trait::async_trait]
pub trait MutationDiagnoser: Send + Sync {
    /// Diagnose the already-filtered, absolute, diagnosable `paths`, returning
    /// the sharp report plus whether the analysis settled.
    async fn diagnose(&self, paths: &[String], context: &ToolContext) -> DiagnoseOutcome;
}

/// The production [`MutationDiagnoser`]: resolves the live session and
/// blast-radius dependents and drives `diagnose_with_outcome`, reusing the exact
/// report-producing core the `diagnostics` MCP tool uses.
pub struct LiveDiagnoser;

#[async_trait::async_trait]
impl MutationDiagnoser for LiveDiagnoser {
    async fn diagnose(&self, paths: &[String], context: &ToolContext) -> DiagnoseOutcome {
        let repo = repo_root(context);
        produce_outcome(paths, &repo, context, &DiagnosticsConfig::default()).await
    }
}

/// Resolve the repository root from the session work-dir (never a stray
/// `current_dir()` when a work-dir is set), matching the `diagnostics` tool.
fn repo_root(context: &ToolContext) -> PathBuf {
    let working_dir = context.session_root();
    find_git_repository_root_from(&working_dir).unwrap_or(working_dir)
}

/// Fold inline diagnostics into a mutating tool's own result, using the
/// production [`LiveDiagnoser`].
///
/// This is the single entry point both dispatch paths in
/// [`server`](crate::mcp::server) call after `tool.execute`. It drains the typed
/// `mutated_paths` channel from `context`, and — for the diagnosable subset —
/// folds a diagnostics report into the result. A non-mutating call (empty
/// channel) or a non-diagnosable mutation returns `result` unchanged.
pub async fn fold_in_diagnostics(
    result: Result<CallToolResult, McpError>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    fold_in_diagnostics_with(result, context, &LiveDiagnoser).await
}

/// The diagnoser-injectable core of [`fold_in_diagnostics`].
///
/// Drains the mutated-path channel, gates on `is_diagnosable`, runs `diagnoser`
/// over the diagnosable subset, and folds the outcome into the result's JSON.
/// Errors and non-mutating / non-diagnosable calls pass `result` through
/// untouched.
pub async fn fold_in_diagnostics_with(
    result: Result<CallToolResult, McpError>,
    context: &ToolContext,
    diagnoser: &dyn MutationDiagnoser,
) -> Result<CallToolResult, McpError> {
    // Always drain the channel so a recorded path never bleeds into a later
    // call sharing the same context, even on the error / non-diagnosable paths.
    let mutated = context.take_mutated_paths();

    // A failed tool call has no successful mutation to diagnose.
    let call_result = match result {
        Ok(call_result) => call_result,
        Err(e) => return Err(e),
    };

    let diagnosable = diagnosable_paths(&mutated);
    if diagnosable.is_empty() {
        return Ok(call_result);
    }

    let outcome = diagnoser.diagnose(&diagnosable, context).await;
    Ok(fold_outcome_into_result(call_result, &outcome))
}

/// The deduplicated, diagnosable subset of `mutated`, as absolute path strings.
///
/// Non-diagnosable files (`.md`, `.txt`, extension-less) are dropped via the one
/// shared [`is_diagnosable`] predicate, so this is the sole gate.
fn diagnosable_paths(mutated: &[PathBuf]) -> Vec<String> {
    let mut seen = HashSet::new();
    mutated
        .iter()
        .filter(|p| is_diagnosable(p))
        .map(|p| p.to_string_lossy().into_owned())
        .filter(|p| seen.insert(p.clone()))
        .collect()
}

/// Fold a diagnostics outcome into a tool result.
///
/// Attaches the report only when there is something to say — a non-empty report
/// or a `pending` analysis — so a clean edit adds no noise. The diagnostics ride
/// on both `structured_content` (for structured-aware hosts) and an appended
/// text block (for hosts that splice the result text into the conversation), so
/// the model sees them regardless of how the host surfaces tool results.
fn fold_outcome_into_result(
    mut result: CallToolResult,
    outcome: &DiagnoseOutcome,
) -> CallToolResult {
    if outcome.report.diagnostics.is_empty() && !outcome.pending {
        return result;
    }

    let folded = serde_json::json!({
        "diagnostics": outcome.report,
        "pending": outcome.pending,
    });

    // Structured surface: merge under a `diagnostics`/`pending` key so any
    // pre-existing structured content is preserved.
    let mut structured = match result.structured_content.take() {
        Some(serde_json::Value::Object(map)) => map,
        _ => serde_json::Map::new(),
    };
    structured.insert("diagnostics".to_string(), folded["diagnostics"].clone());
    structured.insert("pending".to_string(), folded["pending"].clone());
    result.structured_content = Some(serde_json::Value::Object(structured));

    // Text surface: append a JSON block so hosts that only forward result text
    // still deliver the diagnostics to the model.
    let text = serde_json::to_string_pretty(&folded).unwrap_or_else(|_| folded.to_string());
    result.content.push(Content::text(text));

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use swissarmyhammer_diagnostics::{
        DiagnosticRecord, DiagnosticSeverity, DiagnosticsReport, Range,
    };

    use crate::mcp::tool_registry::ToolContext;
    use crate::test_utils::create_test_context;

    /// A stub diagnoser returning a canned outcome and recording the paths it
    /// was asked to diagnose, so a test can assert the gating behaviour.
    struct StubDiagnoser {
        outcome: DiagnoseOutcome,
        seen: Arc<std::sync::Mutex<Vec<String>>>,
    }

    #[async_trait::async_trait]
    impl MutationDiagnoser for StubDiagnoser {
        async fn diagnose(&self, paths: &[String], _context: &ToolContext) -> DiagnoseOutcome {
            self.seen.lock().unwrap().extend(paths.iter().cloned());
            self.outcome.clone()
        }
    }

    /// Reuse the canonical shared test-context builder
    /// ([`create_test_context`](crate::test_utils::create_test_context)) rather
    /// than hand-rolling a `ToolContext`. The default context already carries a
    /// usable per-call `mutated_paths` sink, which is all these tests need.
    async fn context() -> ToolContext {
        create_test_context().await
    }

    fn error_report(message: &str) -> DiagnosticsReport {
        DiagnosticsReport::new(vec![DiagnosticRecord {
            path: "src/lib.rs".to_string(),
            range: Range {
                start_line: 0,
                start_character: 0,
                end_line: 0,
                end_character: 1,
            },
            severity: DiagnosticSeverity::Error,
            message: message.to_string(),
            code: None,
            source: Some("rustc".to_string()),
            containing_symbol: None,
        }])
    }

    fn ok_result() -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![Content::text("OK")]))
    }

    fn result_text(result: &CallToolResult) -> String {
        result
            .content
            .iter()
            .filter_map(|c| match &c.raw {
                rmcp::model::RawContent::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// An edit of a diagnosable `.rs` file with an injected error folds the
    /// diagnostics into the tool result.
    #[tokio::test]
    async fn rust_edit_folds_diagnostics_into_result() {
        let context = context().await;
        context.record_mutated_path("/repo/src/lib.rs");

        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let diagnoser = StubDiagnoser {
            outcome: DiagnoseOutcome {
                report: error_report("mismatched types"),
                pending: false,
            },
            seen: seen.clone(),
        };

        let folded = fold_in_diagnostics_with(ok_result(), &context, &diagnoser)
            .await
            .expect("fold-in succeeds");

        // The diagnoser was asked about the diagnosable path.
        assert_eq!(seen.lock().unwrap().as_slice(), &["/repo/src/lib.rs"]);
        // The diagnostics reached both surfaces.
        let text = result_text(&folded);
        assert!(
            text.contains("mismatched types"),
            "text carries the diagnostic: {text}"
        );
        let structured = folded.structured_content.expect("structured content set");
        assert_eq!(structured["pending"], serde_json::json!(false));
        assert_eq!(
            structured["diagnostics"]["counts"]["errors"],
            serde_json::json!(1)
        );
        // The original result is preserved.
        assert!(text.contains("OK"), "original result preserved: {text}");
    }

    /// An edit of a non-diagnosable `.md` file attaches nothing and never calls
    /// the diagnoser.
    #[tokio::test]
    async fn markdown_edit_attaches_nothing() {
        let context = context().await;
        context.record_mutated_path("/repo/README.md");

        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let diagnoser = StubDiagnoser {
            outcome: DiagnoseOutcome {
                report: error_report("should never appear"),
                pending: false,
            },
            seen: seen.clone(),
        };

        let folded = fold_in_diagnostics_with(ok_result(), &context, &diagnoser)
            .await
            .expect("fold-in succeeds");

        assert!(
            seen.lock().unwrap().is_empty(),
            "diagnoser must not be called for .md"
        );
        assert_eq!(result_text(&folded), "OK", "no diagnostics folded in");
        assert!(folded.structured_content.is_none());
    }

    /// A non-quiescent analysis yields a `pending` marker rather than blocking.
    #[tokio::test]
    async fn pending_analysis_yields_pending_marker() {
        let context = context().await;
        context.record_mutated_path("/repo/src/lib.rs");

        let diagnoser = StubDiagnoser {
            outcome: DiagnoseOutcome {
                report: DiagnosticsReport::new(Vec::new()),
                pending: true,
            },
            seen: Arc::new(std::sync::Mutex::new(Vec::new())),
        };

        let folded = fold_in_diagnostics_with(ok_result(), &context, &diagnoser)
            .await
            .expect("fold-in succeeds");

        let text = result_text(&folded);
        let structured = folded.structured_content.expect("structured content set");
        assert_eq!(structured["pending"], serde_json::json!(true));
        assert!(text.contains("pending"), "pending surfaced in text");
    }

    /// A tool that mutated nothing (empty channel) is passed through untouched.
    #[tokio::test]
    async fn non_mutating_call_is_passed_through() {
        let context = context().await;
        let diagnoser = StubDiagnoser {
            outcome: DiagnoseOutcome {
                report: error_report("never"),
                pending: false,
            },
            seen: Arc::new(std::sync::Mutex::new(Vec::new())),
        };

        let folded = fold_in_diagnostics_with(ok_result(), &context, &diagnoser)
            .await
            .expect("fold-in succeeds");
        assert_eq!(result_text(&folded), "OK");
        assert!(folded.structured_content.is_none());
    }

    /// A failed tool call is returned as-is, with the channel still drained.
    #[tokio::test]
    async fn error_result_is_passed_through_and_channel_drained() {
        let context = context().await;
        context.record_mutated_path("/repo/src/lib.rs");

        let diagnoser = StubDiagnoser {
            outcome: DiagnoseOutcome {
                report: error_report("never"),
                pending: false,
            },
            seen: Arc::new(std::sync::Mutex::new(Vec::new())),
        };

        let err: Result<CallToolResult, McpError> = Err(McpError::internal_error("boom", None));
        let folded = fold_in_diagnostics_with(err, &context, &diagnoser).await;
        assert!(folded.is_err());
        // Channel was drained even on the error path.
        assert!(context.take_mutated_paths().is_empty());
    }

    /// A stub "non-file" mutator that reports a diagnosable path still gets
    /// diagnostics — the channel is the contract, not the tool's identity.
    #[tokio::test]
    async fn stub_mutator_reporting_a_path_gets_diagnostics() {
        let context = context().await;
        // A mutator that is not the `files` tool, recording a synthetic path.
        context.record_mutated_path("/generated/output.rs");

        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let diagnoser = StubDiagnoser {
            outcome: DiagnoseOutcome {
                report: error_report("synthesized error"),
                pending: false,
            },
            seen: seen.clone(),
        };

        let folded = fold_in_diagnostics_with(ok_result(), &context, &diagnoser)
            .await
            .expect("fold-in succeeds");
        assert_eq!(seen.lock().unwrap().as_slice(), &["/generated/output.rs"]);
        assert!(result_text(&folded).contains("synthesized error"));
    }

    /// End-to-end through the real `files` mutator: an `edit file` of a `.rs`
    /// file records its absolute path on the typed channel, and the shared
    /// fold-in helper (the exact one both server dispatch paths call) folds the
    /// diagnostics into the result. This exercises the full
    /// mutator → channel → helper chain without a live LSP server.
    #[tokio::test]
    async fn files_edit_records_path_and_helper_folds() {
        use crate::mcp::tool_registry::McpTool;
        use crate::mcp::tools::files::FilesTool;

        let temp = tempfile::TempDir::new().unwrap();
        let file = temp.path().join("lib.rs");
        std::fs::write(&file, "fn main() {}").unwrap();

        // A per-call sink, exactly as the chokepoint installs.
        let context = context().await.with_fresh_mutated_paths();

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("edit file"));
        args.insert(
            "file_path".to_string(),
            serde_json::json!(file.to_string_lossy()),
        );
        args.insert("old_string".to_string(), serde_json::json!("main"));
        args.insert("new_string".to_string(), serde_json::json!("run"));

        let raw = FilesTool::new().execute(args, &context).await;
        assert!(raw.is_ok(), "edit should succeed: {raw:?}");

        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let diagnoser = StubDiagnoser {
            outcome: DiagnoseOutcome {
                report: error_report("type error"),
                pending: false,
            },
            seen: seen.clone(),
        };
        let folded = fold_in_diagnostics_with(raw, &context, &diagnoser)
            .await
            .expect("fold-in succeeds");

        // The mutator recorded the edited file (absolute, canonicalized).
        let recorded = seen.lock().unwrap();
        assert_eq!(recorded.len(), 1, "exactly one path recorded");
        assert!(
            recorded[0].ends_with("lib.rs"),
            "recorded path: {}",
            recorded[0]
        );
        // The diagnostics were folded into the result.
        assert!(result_text(&folded).contains("type error"));
    }

    /// A tool-to-tool `call_tool` to a mutator must NOT pollute the *outer*
    /// call's mutated-paths sink: inline diagnostics are a property of top-level
    /// dispatch, not internal tool composition. `call_tool` isolates the inner
    /// call with a fresh sink, so the outer context sees nothing.
    #[tokio::test]
    async fn tool_to_tool_mutation_does_not_pollute_outer_sink() {
        use std::sync::Arc;
        use tokio::sync::RwLock;

        use crate::mcp::tool_registry::ToolRegistry;
        use crate::mcp::tools::files::register_file_tools;

        let temp = tempfile::TempDir::new().unwrap();
        let file = temp.path().join("lib.rs");
        std::fs::write(&file, "fn main() {}").unwrap();

        let mut registry = ToolRegistry::new();
        register_file_tools(&mut registry);
        let outer = context()
            .await
            .with_fresh_mutated_paths()
            .with_tool_registry(Arc::new(RwLock::new(registry)));

        // The outer tool delegates an edit through call_tool.
        let result = outer
            .call_tool(
                "files",
                serde_json::json!({
                    "op": "edit file",
                    "file_path": file.to_string_lossy(),
                    "old_string": "main",
                    "new_string": "run",
                }),
            )
            .await;
        assert!(result.is_ok(), "delegated edit should succeed: {result:?}");

        // The inner mutation was isolated — the outer sink is empty.
        assert!(
            outer.take_mutated_paths().is_empty(),
            "tool-to-tool mutation must not pollute the outer sink"
        );
        // And the edit really happened.
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "fn run() {}");
    }

    /// A clean diagnosable edit (empty report, settled) adds no noise.
    #[tokio::test]
    async fn clean_edit_attaches_nothing() {
        let context = context().await;
        context.record_mutated_path("/repo/src/lib.rs");

        let diagnoser = StubDiagnoser {
            outcome: DiagnoseOutcome {
                report: DiagnosticsReport::new(Vec::new()),
                pending: false,
            },
            seen: Arc::new(std::sync::Mutex::new(Vec::new())),
        };

        let folded = fold_in_diagnostics_with(ok_result(), &context, &diagnoser)
            .await
            .expect("fold-in succeeds");
        assert_eq!(result_text(&folded), "OK", "clean edit attaches nothing");
        assert!(folded.structured_content.is_none());
    }
}
