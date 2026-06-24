//! The single `diagnose` entry point shared by the `diagnostics` tool and the
//! `files edit` fold-in.
//!
//! `diagnose` answers one question sharply: *what broke?* It always reports the
//! files you asked about, and of their one-hop dependents it folds in **only the
//! ones that actually broke** — never a project-wide dump of unrelated standing
//! warnings. The design is "compute freely, surface selectively": computing the
//! blast radius is cheap, so we run diagnostics across the edited file's callers,
//! but we keep only those with fresh error/warning diagnostics.
//!
//! The blast radius itself comes from `swissarmyhammer-code-context` (this is the
//! one place the `diagnostics → code-context` dependency is used). The resolver
//! is abstracted behind the [`Dependents`] trait so the core selection logic is
//! unit-testable with a stub, with no index or LSP server.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use rusqlite::Connection;

use swissarmyhammer_code_context::{get_blastradius, BlastRadiusOptions};
use swissarmyhammer_lsp::client::LspTransport;
use swissarmyhammer_lsp::{file_uri_from_path, DiagnosticSeverity, LspSession};

use crate::config::DiagnosticsConfig;
use crate::record::{DiagnosticRecord, DiagnosticsReport};
use crate::settle::{settle, SettleOutcome, Timer};

/// One-hop blast-radius source: the files whose code calls into a given file and
/// could therefore break when it changes.
///
/// Abstracted as a trait so [`diagnose`]'s broken-vs-clean selection can be
/// exercised with a stub — the production implementation,
/// [`BlastRadiusDependents`], hits the code-context index.
pub trait Dependents {
    /// Return the one-hop dependent file paths of `file_path` (its direct
    /// callers), excluding `file_path` itself. Order is unspecified; `diagnose`
    /// dedups and ranks.
    fn one_hop(&self, file_path: &str) -> Vec<String>;
}

/// The production [`Dependents`]: one-hop callers from the code-context
/// blast-radius index.
pub struct BlastRadiusDependents<'a> {
    conn: &'a Connection,
}

impl<'a> BlastRadiusDependents<'a> {
    /// Wrap a code-context index connection as a dependents source.
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }
}

impl Dependents for BlastRadiusDependents<'_> {
    fn one_hop(&self, file_path: &str) -> Vec<String> {
        let options = BlastRadiusOptions {
            file_path: file_path.to_string(),
            symbol: None,
            max_hops: 1,
        };
        // A file that is not indexed (or has no symbols) has no known
        // dependents — treat that as "nothing broke downstream", not an error.
        let radius = match get_blastradius(self.conn, &options) {
            Ok(radius) => radius,
            Err(_) => return Vec::new(),
        };

        let mut files: Vec<String> = radius
            .hops
            .iter()
            .flat_map(|hop| hop.symbols.iter().map(|symbol| symbol.file_path.clone()))
            .filter(|f| f != file_path)
            .collect();
        files.sort();
        files.dedup();
        files
    }
}

/// A [`Dependents`] backed by a precomputed map of file → its one-hop
/// dependents.
///
/// Resolving the blast radius hits a `rusqlite::Connection`, which is `!Sync`
/// and so cannot be held across an `.await`. An async caller therefore resolves
/// every path's dependents up front — via [`PrecomputedDependents::resolve`]
/// over a [`BlastRadiusDependents`] — drops the DB connection, and hands this
/// `Send` map to [`diagnose`].
#[derive(Debug, Clone, Default)]
pub struct PrecomputedDependents {
    map: HashMap<String, Vec<String>>,
}

impl PrecomputedDependents {
    /// Build from an explicit `file → dependents` map.
    pub fn new(map: HashMap<String, Vec<String>>) -> Self {
        Self { map }
    }

    /// Eagerly resolve the one-hop dependents of every path with `resolver`,
    /// capturing the result so it can outlive the resolver (and any DB handle
    /// it borrows).
    pub fn resolve(resolver: &impl Dependents, paths: &[String]) -> Self {
        let map = paths
            .iter()
            .map(|path| (path.clone(), resolver.one_hop(path)))
            .collect();
        Self { map }
    }
}

impl Dependents for PrecomputedDependents {
    fn one_hop(&self, file_path: &str) -> Vec<String> {
        self.map.get(file_path).cloned().unwrap_or_default()
    }
}

/// Produce a sharp diagnostics report for `paths`.
///
/// Ensures each queried file (and, when enabled, its broken dependents) is
/// synced into the shared `session`, waits for diagnostics to settle, then
/// builds a [`DiagnosticsReport`] that always includes the queried files and
/// folds in only the one-hop dependents that broke (have error/warning
/// diagnostics), ranked by severity and capped at `config.per_report_cap`.
///
/// Honors the config overrides: `include_dependents` gates the fold-in,
/// `severities` filters which diagnostics surface, `settle_window` /
/// `settle_hard_timeout` tune quiescence. The `timer` is injectable for
/// deterministic tests.
///
/// "Broke" means a dependent has an error- or warning-severity diagnostic.
/// Because `severities` filters before this check, a dependent whose only
/// breakage is a warning is invisible (and so treated as clean) when
/// `severities` excludes `Warning` — by design: a report never surfaces a
/// severity the caller asked to hide.
///
/// Document syncing is best-effort: if no language server is live, or a file
/// cannot be read, the corresponding sync is skipped and `diagnose` reports
/// whatever the session already knows. A never-quiescing settle (the
/// pathological backstop) yields an empty report rather than a mid-analysis
/// snapshot.
pub async fn diagnose<C, T, D>(
    session: &LspSession<C>,
    paths: &[String],
    config: &DiagnosticsConfig,
    dependents: &D,
    timer: &T,
) -> DiagnosticsReport
where
    C: LspTransport,
    T: Timer,
    D: Dependents,
{
    diagnose_with_outcome(session, paths, config, dependents, timer)
        .await
        .report
}

/// A diagnostics report together with whether the underlying analysis settled.
///
/// [`diagnose`] collapses a never-quiescing settle to an empty report, which is
/// indistinguishable from "clean". A consumer that must tell "still analyzing"
/// apart from "clean" — notably the inline-on-edit fold-in, which surfaces a
/// `pending` marker so the model knows the report is provisional — calls
/// [`diagnose_with_outcome`] instead and reads [`pending`](Self::pending).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnoseOutcome {
    /// The sharp report (queried files plus broken capped dependents).
    pub report: DiagnosticsReport,
    /// `true` when the diagnostics stream never quiesced within the hard
    /// timeout, so `report` reflects only what had been published so far.
    pub pending: bool,
}

/// Produce a sharp diagnostics report for `paths`, reporting whether the
/// analysis settled.
///
/// Identical to [`diagnose`] except it preserves the [`SettleOutcome::Pending`]
/// signal: when the diagnostics stream never quiesced within the hard timeout,
/// the returned [`DiagnoseOutcome::pending`] is `true`. [`diagnose`] is a thin
/// wrapper that discards that flag.
pub async fn diagnose_with_outcome<C, T, D>(
    session: &LspSession<C>,
    paths: &[String],
    config: &DiagnosticsConfig,
    dependents: &D,
    timer: &T,
) -> DiagnoseOutcome
where
    C: LspTransport,
    T: Timer,
    D: Dependents,
{
    let targets = dedup_preserving_order(paths);
    let target_set: HashSet<&str> = targets.iter().map(String::as_str).collect();

    // One-hop dependents (their broken subset is selected later), unless the
    // toggle is off. Exclude the targets themselves.
    let dependent_files: Vec<String> = if config.include_dependents {
        let mut deps: Vec<String> = targets
            .iter()
            .flat_map(|target| dependents.one_hop(target))
            .filter(|dep| !target_set.contains(dep.as_str()))
            .collect();
        deps.sort();
        deps.dedup();
        deps
    } else {
        Vec::new()
    };

    // Best-effort: sync every relevant document into the session so the server
    // re-analyzes against current content. Only possible with a live server.
    let all_files: Vec<&String> = targets.iter().chain(dependent_files.iter()).collect();
    if session.is_running() {
        for path in &all_files {
            if let Ok(text) = std::fs::read_to_string(path.as_str()) {
                let _ = session.sync_open(Path::new(path.as_str()), &text);
            }
        }
    }

    // Settle every watched uri at once. Use an effectively-uncapped settle so we
    // can partition by file and rank before applying the report cap.
    let watched_uris: Vec<String> = all_files
        .iter()
        .map(|path| file_uri_from_path(path.as_str()))
        .collect();
    let settle_config = DiagnosticsConfig {
        per_report_cap: usize::MAX,
        ..config.clone()
    };
    let (records, settle_pending) = match settle(session, &watched_uris, &settle_config, timer).await
    {
        SettleOutcome::Settled(records) => (records, false),
        SettleOutcome::Pending => (Vec::new(), true),
    };

    // A running server that has signalled it is still loading (a pull answered
    // with ServerCancelled / ContentModified / retrigger — see
    // [`LspSession::is_ready`]) cannot give an authoritative "clean": an empty
    // settled set then reflects "not analyzed yet", not "no problems". Report
    // `pending` so the consumer (e.g. the inline fold-in) surfaces that instead
    // of mistaking a not-yet-loaded server's silence for a clean file.
    let not_ready = session.is_running() && !session.is_ready();

    DiagnoseOutcome {
        report: build_report(records, &targets, &dependent_files, config),
        pending: settle_pending || not_ready,
    }
}

/// Assemble the final report: queried files always present, broken dependents
/// folded in by descending severity, capped.
fn build_report(
    records: Vec<DiagnosticRecord>,
    targets: &[String],
    dependent_files: &[String],
    config: &DiagnosticsConfig,
) -> DiagnosticsReport {
    let mut by_path: BTreeMap<String, Vec<DiagnosticRecord>> = BTreeMap::new();
    for record in records {
        by_path.entry(record.path.clone()).or_default().push(record);
    }

    // Queried files are always reported, in the order they were asked for.
    let mut out: Vec<DiagnosticRecord> = Vec::new();
    for target in targets {
        if let Some(recs) = by_path.get(target) {
            out.extend(recs.iter().cloned());
        }
    }

    // Of the dependents, keep only those that broke (have an error or warning),
    // ranked by error count then warning count so the worst breakage leads.
    let mut broken: Vec<BrokenDependent<'_>> = dependent_files
        .iter()
        .filter_map(|dep| {
            let recs = by_path.get(dep)?;
            let errors = count_severity(recs, DiagnosticSeverity::Error);
            let warnings = count_severity(recs, DiagnosticSeverity::Warning);
            (errors + warnings > 0).then_some(BrokenDependent {
                path: dep,
                errors,
                warnings,
            })
        })
        .collect();
    broken.sort_by(|a, b| {
        b.errors
            .cmp(&a.errors)
            .then(b.warnings.cmp(&a.warnings))
            .then(a.path.cmp(b.path))
    });
    for dependent in broken {
        if let Some(recs) = by_path.get(dependent.path) {
            out.extend(recs.iter().cloned());
        }
    }

    out.truncate(config.per_report_cap);
    DiagnosticsReport::new(out)
}

/// A dependent file that broke, with its error/warning tallies for ranking.
struct BrokenDependent<'a> {
    path: &'a String,
    errors: usize,
    warnings: usize,
}

/// Count records of a given severity.
fn count_severity(records: &[DiagnosticRecord], severity: DiagnosticSeverity) -> usize {
    records.iter().filter(|r| r.severity == severity).count()
}

/// Deduplicate while preserving first-seen order.
fn dedup_preserving_order(paths: &[String]) -> Vec<String> {
    let mut seen: HashSet<&str> = HashSet::new();
    paths
        .iter()
        .filter(|p| seen.insert(p.as_str()))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    use lsp_types::{DiagnosticSeverity as LspSeverity, Position, Range};
    use serde_json::{json, Value};

    use crate::test_support::{ManualTimer, NullTransport};

    /// Build a [`PrecomputedDependents`] from a terse literal map.
    fn stub(map: &[(&str, &[&str])]) -> PrecomputedDependents {
        PrecomputedDependents::new(
            map.iter()
                .map(|(k, v)| {
                    (
                        k.to_string(),
                        v.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                    )
                })
                .collect(),
        )
    }

    fn lsp_diag(severity: LspSeverity, message: &str) -> lsp_types::Diagnostic {
        lsp_types::Diagnostic {
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 1,
                },
            },
            severity: Some(severity),
            message: message.to_string(),
            ..lsp_types::Diagnostic::default()
        }
    }

    /// Seed a not-running session's diagnostics cache for `path` (relative paths
    /// are fine; `diagnose` keys uris as `file://<path>`).
    fn seed(
        session: &LspSession<NullTransport>,
        path: &str,
        diagnostics: Vec<lsp_types::Diagnostic>,
    ) {
        let items: Vec<Value> = diagnostics
            .iter()
            .map(|d| {
                json!({
                    "range": {
                        "start": { "line": d.range.start.line, "character": d.range.start.character },
                        "end": { "line": d.range.end.line, "character": d.range.end.character }
                    },
                    "severity": severity_number(d.severity.unwrap()),
                    "message": d.message,
                })
            })
            .collect();
        session.handle_publish_diagnostics(&json!({
            "uri": format!("file://{path}"),
            "diagnostics": items,
        }));
    }

    fn severity_number(severity: LspSeverity) -> u64 {
        match severity {
            LspSeverity::ERROR => 1,
            LspSeverity::WARNING => 2,
            LspSeverity::INFORMATION => 3,
            _ => 4,
        }
    }

    fn not_running_session() -> LspSession<NullTransport> {
        LspSession::new(Arc::new(Mutex::new(None)), "rust")
    }

    /// Spawn `diagnose` on the current-thread runtime and drive its settle window
    /// with the manual clock, mirroring the settle-engine tests.
    async fn run_diagnose(
        session: LspSession<NullTransport>,
        paths: Vec<String>,
        config: DiagnosticsConfig,
        deps: PrecomputedDependents,
        timer: ManualTimer,
    ) -> DiagnosticsReport {
        let driver = timer.clone();
        let window = config.settle_window;
        let handle =
            tokio::spawn(async move { diagnose(&session, &paths, &config, &deps, &timer).await });
        // Let diagnose run up to settle's debounce park, then advance past it.
        tokio::task::yield_now().await;
        driver.advance(window);
        handle.await.unwrap()
    }

    #[tokio::test(flavor = "current_thread")]
    async fn includes_target_and_broken_dependents_drops_clean() {
        // Target A (error). Dependents B (error -> broken) and C (hint only ->
        // clean). The report must carry A and B, never C.
        let session = not_running_session();
        seed(
            &session,
            "src/a.rs",
            vec![lsp_diag(LspSeverity::ERROR, "A broke")],
        );
        seed(
            &session,
            "src/b.rs",
            vec![lsp_diag(LspSeverity::ERROR, "B broke")],
        );
        seed(
            &session,
            "src/c.rs",
            vec![lsp_diag(LspSeverity::HINT, "c hint")],
        );

        let report = run_diagnose(
            session,
            vec!["src/a.rs".to_string()],
            DiagnosticsConfig::default(),
            stub(&[("src/a.rs", &["src/b.rs", "src/c.rs"])]),
            ManualTimer::default(),
        )
        .await;

        let messages: Vec<&str> = report
            .diagnostics
            .iter()
            .map(|r| r.message.as_str())
            .collect();
        assert!(messages.contains(&"A broke"), "target must be present");
        assert!(
            messages.contains(&"B broke"),
            "broken dependent must be folded in"
        );
        assert!(
            !messages.iter().any(|m| m.contains("c hint")),
            "clean dependent must be dropped"
        );
        assert_eq!(report.counts.errors, 2);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dependents_toggle_off_reports_only_targets() {
        let session = not_running_session();
        seed(
            &session,
            "src/a.rs",
            vec![lsp_diag(LspSeverity::ERROR, "A broke")],
        );
        seed(
            &session,
            "src/b.rs",
            vec![lsp_diag(LspSeverity::ERROR, "B broke")],
        );

        let config = DiagnosticsConfig {
            include_dependents: false,
            ..DiagnosticsConfig::default()
        };
        let report = run_diagnose(
            session,
            vec!["src/a.rs".to_string()],
            config,
            stub(&[("src/a.rs", &["src/b.rs"])]),
            ManualTimer::default(),
        )
        .await;

        let messages: Vec<&str> = report
            .diagnostics
            .iter()
            .map(|r| r.message.as_str())
            .collect();
        assert_eq!(messages, vec!["A broke"], "toggle off excludes dependents");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn report_is_capped_with_targets_kept_first() {
        // Cap of 2: the target's two errors fill the report, leaving no room for
        // the broken dependent — targets are kept first.
        let session = not_running_session();
        seed(
            &session,
            "src/a.rs",
            vec![
                lsp_diag(LspSeverity::ERROR, "A1"),
                lsp_diag(LspSeverity::ERROR, "A2"),
            ],
        );
        seed(
            &session,
            "src/b.rs",
            vec![lsp_diag(LspSeverity::ERROR, "B1")],
        );

        let config = DiagnosticsConfig {
            per_report_cap: 2,
            ..DiagnosticsConfig::default()
        };
        let report = run_diagnose(
            session,
            vec!["src/a.rs".to_string()],
            config,
            stub(&[("src/a.rs", &["src/b.rs"])]),
            ManualTimer::default(),
        )
        .await;

        let messages: Vec<&str> = report
            .diagnostics
            .iter()
            .map(|r| r.message.as_str())
            .collect();
        assert_eq!(
            messages,
            vec!["A1", "A2"],
            "cap keeps the target, drops the overflow dependent"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn broken_dependents_ranked_by_severity() {
        // Two broken dependents: B has 1 error, D has 2 errors. D must lead.
        let session = not_running_session();
        seed(
            &session,
            "src/a.rs",
            vec![lsp_diag(LspSeverity::ERROR, "A")],
        );
        seed(
            &session,
            "src/b.rs",
            vec![lsp_diag(LspSeverity::ERROR, "B1")],
        );
        seed(
            &session,
            "src/d.rs",
            vec![
                lsp_diag(LspSeverity::ERROR, "D1"),
                lsp_diag(LspSeverity::ERROR, "D2"),
            ],
        );

        let report = run_diagnose(
            session,
            vec!["src/a.rs".to_string()],
            DiagnosticsConfig::default(),
            stub(&[("src/a.rs", &["src/b.rs", "src/d.rs"])]),
            ManualTimer::default(),
        )
        .await;

        let messages: Vec<&str> = report
            .diagnostics
            .iter()
            .map(|r| r.message.as_str())
            .collect();
        // Target first, then D (2 errors) before B (1 error).
        assert_eq!(messages, vec!["A", "D1", "D2", "B1"]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dependent_with_only_info_is_not_broken() {
        // With a config that reports info too, an info-only dependent still
        // counts as clean — "broke" means error/warning, not merely "has a
        // diagnostic".
        let session = not_running_session();
        seed(
            &session,
            "src/a.rs",
            vec![lsp_diag(LspSeverity::ERROR, "A")],
        );
        seed(
            &session,
            "src/b.rs",
            vec![lsp_diag(LspSeverity::INFORMATION, "b info")],
        );

        let config = DiagnosticsConfig {
            severities: vec![
                DiagnosticSeverity::Error,
                DiagnosticSeverity::Warning,
                DiagnosticSeverity::Info,
            ],
            ..DiagnosticsConfig::default()
        };
        let report = run_diagnose(
            session,
            vec!["src/a.rs".to_string()],
            config,
            stub(&[("src/a.rs", &["src/b.rs"])]),
            ManualTimer::default(),
        )
        .await;

        let messages: Vec<&str> = report
            .diagnostics
            .iter()
            .map(|r| r.message.as_str())
            .collect();
        assert_eq!(messages, vec!["A"], "info-only dependent is not broken");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn outcome_reports_not_pending_when_settled() {
        // A normal settle (advance past the debounce window) yields the report
        // with `pending == false` — the inline fold-in relies on this flag to
        // tell "clean" apart from "still analyzing".
        let session = not_running_session();
        seed(
            &session,
            "src/a.rs",
            vec![lsp_diag(LspSeverity::ERROR, "A broke")],
        );

        let timer = ManualTimer::default();
        let driver = timer.clone();
        let config = DiagnosticsConfig::default();
        let window = config.settle_window;
        let paths = vec!["src/a.rs".to_string()];
        let deps = stub(&[]);
        let handle = tokio::spawn(async move {
            diagnose_with_outcome(&session, &paths, &config, &deps, &timer).await
        });
        tokio::task::yield_now().await;
        driver.advance(window);
        let outcome = handle.await.unwrap();

        assert!(!outcome.pending, "a settled run is not pending");
        assert_eq!(outcome.report.counts.errors, 1);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn outcome_pending_when_running_server_is_not_ready() {
        use crate::test_support::RecordingTransport;
        // A live client (is_running == true) whose `textDocument/diagnostic`
        // pull is answered with a ServerCancelled/retrigger error: the session
        // records the server as not-ready. `diagnose` must then report
        // `pending` even though the settled set is empty — a not-yet-loaded
        // server's silence is NOT an authoritative "clean".
        let client = Arc::new(Mutex::new(Some(RecordingTransport {
            diagnostic_response: Some(json!({
                "error": { "code": -32802, "data": { "retriggerRequest": true } }
            })),
            ..RecordingTransport::default()
        })));
        let session = LspSession::new(Arc::clone(&client), "rust");

        // A pull (as the watcher drives) flips the session to not-ready.
        let _ = session.pull_diagnostics(Path::new("src/a.rs"));
        assert!(!session.is_ready(), "the cancelled pull marks not-ready");

        let timer = ManualTimer::default();
        let driver = timer.clone();
        let config = DiagnosticsConfig::default();
        let window = config.settle_window;
        let paths = vec!["src/a.rs".to_string()];
        let deps = stub(&[]);
        let handle = tokio::spawn(async move {
            diagnose_with_outcome(&session, &paths, &config, &deps, &timer).await
        });
        tokio::task::yield_now().await;
        driver.advance(window);
        let outcome = handle.await.unwrap();

        assert!(
            outcome.pending,
            "a running-but-not-ready server must report pending, not clean"
        );
        assert!(
            outcome.report.diagnostics.is_empty(),
            "no diagnostics are available while the server is still loading"
        );
    }
}
