//! The leader-owned diagnostics file watcher (one per workdir).
//!
//! A follower process spawns no LSP server (see `^7a5h2bj`), and the closed
//! `files edit` write surface only covers edits made through the tool. A
//! follower's *direct* `files edit` write, plus leaks no closed surface can
//! cover — a subprocess, a formatter, a `git checkout` — all land on disk
//! without the leader's session hearing about them. This watcher closes that
//! gap: it watches the workspace, debounces the change burst, and for each
//! changed *diagnosable* file feeds the new content into the leader's one
//! [`LspSession`] as a `didChange` and pulls fresh diagnostics. Because the
//! watcher issues the `didChange` itself, a follower need not sync the analyzer
//! — it just writes the file.
//!
//! The fresh diagnostics flow into the session's in-process fan-out, which the
//! leader re-publishes across the process boundary via
//! [`fan_out_to_bus`](crate::bus::fan_out_to_bus) — so a follower sees the
//! result even though it never touched the analyzer.
//!
//! ## Reuse
//!
//! The debounce machinery is `async-watcher`'s [`AsyncDebouncer`] — the same
//! debounced-`notify` wrapper the code-context watcher uses — not a second
//! watch implementation. The diagnosable-file gate is the shared
//! [`is_diagnosable`](crate::language::is_diagnosable). What differs from the
//! code-context watcher is only the *action* (LSP `didChange` + pull, vs marking
//! DB rows dirty), so this is a distinct action over the shared library, not a
//! duplicate watcher.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_watcher::{notify::RecursiveMode, AsyncDebouncer};

use swissarmyhammer_lsp::client::LspTransport;
use swissarmyhammer_lsp::LspSession;

use crate::language::is_diagnosable;

/// Best-effort, transport-agnostic callback the watcher invokes once per
/// refreshed file so a host can be told a native edit was seen.
///
/// This is the watcher-push courtesy channel for the hardest case — a foreign
/// host doing a native edit where the tool's closed write surface never ran. The
/// watcher still detects the change but can only reach the host, not wake an idle
/// model, so this is explicitly *not* load-bearing: a `None` notifier (or a panic
/// inside one) must never change whether the file is re-diagnosed.
///
/// The diagnostics crate stays free of any MCP/rmcp dependency by taking this
/// abstract callback; the `swissarmyhammer-tools` server wires it to a plain MCP
/// `notifications/message` (`peer.notify_logging_message`), which in llama-agent
/// relays through the existing
/// `NotifyingClientHandler::relay_logging_message` into an ACP `SessionUpdate`.
pub type WatcherNotifier = Arc<dyn Fn(&Path) + Send + Sync>;

/// One language server's session plus the file extensions it handles.
///
/// The watcher routes each changed file to the session whose `extensions` claim
/// the file's extension, so a `.rs` edit only re-diagnoses against the rust
/// session and a `.py` edit only against the python one — never every session.
/// Extensions are matched case-insensitively and carry no leading dot.
pub struct SessionRoute<C: LspTransport = swissarmyhammer_lsp::client::LspJsonRpcClient> {
    /// File extensions (without the dot) this session's server handles.
    extensions: Vec<String>,
    /// The session to feed `didChange` + pull into for a matching file.
    session: LspSession<C>,
}

impl<C: LspTransport> SessionRoute<C> {
    /// Build a route for `session` over the given `extensions`.
    pub fn new(extensions: Vec<String>, session: LspSession<C>) -> Self {
        Self {
            extensions,
            session,
        }
    }

    /// The file extensions (without the dot) this route's server handles.
    pub fn extensions(&self) -> &[String] {
        &self.extensions
    }

    /// The session this route feeds `didChange` + pull into.
    pub fn session(&self) -> &LspSession<C> {
        &self.session
    }

    /// Whether this route's server claims `path`'s extension.
    fn handles(&self, path: &Path) -> bool {
        match path.extension().and_then(|e| e.to_str()) {
            Some(ext) => self.extensions.iter().any(|e| e.eq_ignore_ascii_case(ext)),
            None => false,
        }
    }
}

/// Resolve the session that handles `path`, or `None` if no route claims it.
fn route_for<'a, C: LspTransport>(
    routes: &'a [SessionRoute<C>],
    path: &Path,
) -> Option<&'a LspSession<C>> {
    routes.iter().find(|r| r.handles(path)).map(|r| &r.session)
}

/// Debounce window for collapsing a change burst before re-diagnosing.
///
/// Matches the code-context watcher's 1-second window: a save often lands as
/// several `notify` events, and an external tool (a formatter, `git checkout`)
/// can rewrite many files at once. Re-diagnosing once after the burst settles
/// is far cheaper than once per raw event.
pub const DIAGNOSTICS_WATCH_DEBOUNCE: Duration = Duration::from_secs(1);

/// Re-diagnose a single changed file against the leader's session.
///
/// Reads `path` from disk, syncs the new content into the session (a `didOpen`
/// if the document was never opened, a `didChange` if it was — both no-ops when
/// the text is unchanged), then pulls diagnostics so the result lands in the
/// session's per-uri cache and in-process fan-out. Best-effort: a file that
/// cannot be read (deleted between the event and the read) or a session with no
/// live client is silently skipped — losing one file's refresh must not tear
/// down the watcher.
///
/// Returns `true` when the file was diagnosable and a refresh was attempted,
/// `false` when it was skipped (non-diagnosable or unreadable).
pub fn refresh_file<C: LspTransport>(session: &LspSession<C>, path: &Path) -> bool {
    if !is_diagnosable(path) {
        return false;
    }
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    if session.sync_open(path, &text).is_err() {
        // No live client (or a transient notify failure) — nothing to pull.
        return false;
    }
    // Pull so the result feeds the cache + fan-out. The push model is not wired
    // into the daemon read loop, so the pull is what makes the change observable.
    let _ = session.pull_diagnostics(path);
    true
}

/// Re-diagnose every changed file in a debounced batch, routing each to the
/// session whose server handles it.
///
/// The single seam shared by the watch loop and its unit test: given the paths
/// from one debounced batch and the per-server routing table, it resolves each
/// path to its handling session ([`route_for`]) and calls [`refresh_file`],
/// returning how many were refreshed. A path no route claims (or that
/// [`refresh_file`] skips) is not counted. Deduping is the caller's concern; a
/// batch may list the same file twice and a second `sync_open` is a no-op when
/// the text is unchanged.
pub fn refresh_changed_files<C: LspTransport>(
    routes: &[SessionRoute<C>],
    paths: &[PathBuf],
) -> usize {
    refresh_changed_files_notified(routes, paths, None)
}

/// [`refresh_changed_files`] plus a best-effort push: when `notifier` is set, it
/// is invoked once for each file that was actually refreshed, after the refresh.
///
/// The push is the watcher-push courtesy channel (see [`WatcherNotifier`]). It is
/// driven off the refresh result, not the raw event, so an unrouted or skipped
/// file is never pushed. The notifier fires *after* the refresh so the session's
/// per-uri cache already holds the fresh diagnostics by the time the host is
/// told. A `None` notifier yields exactly [`refresh_changed_files`]'s behavior.
pub fn refresh_changed_files_notified<C: LspTransport>(
    routes: &[SessionRoute<C>],
    paths: &[PathBuf],
    notifier: Option<&WatcherNotifier>,
) -> usize {
    paths
        .iter()
        .filter(|p| match route_for(routes, p) {
            Some(session) if refresh_file(session, p) => {
                if let Some(notify) = notifier {
                    notify(p);
                }
                true
            }
            _ => false,
        })
        .count()
}

/// Start the leader-owned diagnostics watcher for `workspace_root`.
///
/// Spawns ONE background task per workdir that watches the workspace
/// recursively, debounces change bursts by [`DIAGNOSTICS_WATCH_DEBOUNCE`], and
/// routes each changed file to the session whose server handles its extension
/// ([`SessionRoute`]), feeding it a `didChange` + pull. Taking the full routing
/// table (rather than a single session) is what keeps it one watcher per
/// workdir even when several language servers run, and stops a `.py` edit from
/// being fed into the rust session.
pub fn start_diagnostics_watcher<C>(
    workspace_root: PathBuf,
    routes: Vec<SessionRoute<C>>,
) -> tokio::task::JoinHandle<()>
where
    C: LspTransport + Send + 'static,
{
    start_diagnostics_watcher_with_notifier(workspace_root, routes, None)
}

/// [`start_diagnostics_watcher`] plus a best-effort watcher-push notifier.
///
/// The `notifier` (see [`WatcherNotifier`]) is invoked once per refreshed file as
/// each debounced batch is processed. Passing `None` is exactly
/// [`start_diagnostics_watcher`]. The push is a courtesy channel and never gates
/// the re-diagnose path.
pub fn start_diagnostics_watcher_with_notifier<C>(
    workspace_root: PathBuf,
    routes: Vec<SessionRoute<C>>,
    notifier: Option<WatcherNotifier>,
) -> tokio::task::JoinHandle<()>
where
    C: LspTransport + Send + 'static,
{
    tokio::spawn(async move {
        if let Err(e) = run_diagnostics_watcher(&workspace_root, &routes, notifier.as_ref()).await {
            tracing::error!("diagnostics watcher failed: {}", e);
        }
    })
}

/// The watch loop: collect debounced batches and re-diagnose changed files.
async fn run_diagnostics_watcher<C: LspTransport>(
    workspace_root: &Path,
    routes: &[SessionRoute<C>],
    notifier: Option<&WatcherNotifier>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (mut debouncer, mut event_rx) =
        AsyncDebouncer::new_with_channel(DIAGNOSTICS_WATCH_DEBOUNCE, None).await?;

    debouncer
        .watcher()
        .watch(workspace_root, RecursiveMode::Recursive)?;

    tracing::info!(
        "diagnostics: file watcher started for {}",
        workspace_root.display()
    );

    while let Some(events_result) = event_rx.recv().await {
        match events_result {
            Ok(debounced_events) => {
                let paths = changed_paths(&debounced_events);
                if paths.is_empty() {
                    continue;
                }
                let refreshed = refresh_changed_files_notified(routes, &paths, notifier);
                if refreshed > 0 {
                    tracing::info!("diagnostics: re-diagnosed {} changed file(s)", refreshed);
                }
            }
            Err(errors) => {
                for error in errors {
                    tracing::warn!("diagnostics watcher error: {}", error);
                }
            }
        }
    }

    tracing::info!("diagnostics: file watcher stopped");
    Ok(())
}

/// Flatten a debounced batch into the deduped set of changed file paths.
///
/// Each [`DebouncedEvent`](async_watcher::DebouncedEvent) is keyed by a single
/// `path` (the debouncer collapses per-path); the underlying notify event's
/// `paths` can be empty for some backends, so the per-event `path` is the
/// reliable source.
fn changed_paths(events: &[async_watcher::DebouncedEvent]) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = events.iter().map(|e| e.path.clone()).collect();
    paths.sort();
    paths.dedup();
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    use swissarmyhammer_lsp::LspSession;

    use crate::test_support::{NullTransport, RecordingTransport};

    /// Build a session over a shared recording transport.
    fn recording_session() -> (
        LspSession<RecordingTransport>,
        Arc<Mutex<Option<RecordingTransport>>>,
    ) {
        let client = Arc::new(Mutex::new(Some(RecordingTransport::default())));
        let session = LspSession::new(Arc::clone(&client), "rust");
        (session, client)
    }

    #[test]
    fn refresh_file_skips_non_diagnosable() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("README.md");
        std::fs::write(&md, "# hi").unwrap();

        let (session, client) = recording_session();
        assert!(!refresh_file(&session, &md));
        // Nothing was sent for a non-diagnosable file.
        let guard = client.lock().unwrap();
        let t = guard.as_ref().unwrap();
        assert_eq!(t.notification_count("textDocument/didOpen"), 0);
        assert_eq!(t.request_count("textDocument/diagnostic"), 0);
    }

    #[test]
    fn refresh_file_opens_and_pulls_for_a_diagnosable_file() {
        let dir = tempfile::tempdir().unwrap();
        let rs = dir.path().join("main.rs");
        std::fs::write(&rs, "fn main() {}\n").unwrap();

        let (session, client) = recording_session();
        assert!(refresh_file(&session, &rs));

        let guard = client.lock().unwrap();
        let t = guard.as_ref().unwrap();
        // First touch opens the document, then pulls diagnostics.
        assert_eq!(t.notification_count("textDocument/didOpen"), 1);
        assert_eq!(t.request_count("textDocument/diagnostic"), 1);
        // The pulled diagnostics landed in the session cache + fan-out.
        let uri = swissarmyhammer_lsp::file_uri_from_path(&rs.to_string_lossy());
        assert_eq!(session.diagnostics_for(&uri).len(), 1);
    }

    #[test]
    fn refresh_file_on_changed_content_issues_didchange() {
        let dir = tempfile::tempdir().unwrap();
        let rs = dir.path().join("main.rs");
        std::fs::write(&rs, "fn main() {}\n").unwrap();

        let (session, client) = recording_session();
        // First refresh opens it.
        assert!(refresh_file(&session, &rs));
        // A real on-disk edit (a follower's direct write / external tool).
        std::fs::write(&rs, "fn main() { let x = 1; }\n").unwrap();
        assert!(refresh_file(&session, &rs));

        let guard = client.lock().unwrap();
        let t = guard.as_ref().unwrap();
        assert_eq!(t.notification_count("textDocument/didOpen"), 1);
        // The second refresh, with changed text, is a didChange — this is the
        // path that catches a write the closed edit surface never saw.
        assert_eq!(t.notification_count("textDocument/didChange"), 1);
        assert_eq!(t.request_count("textDocument/diagnostic"), 2);
    }

    #[test]
    fn refresh_changed_files_counts_only_routed_files() {
        let dir = tempfile::tempdir().unwrap();
        let rs = dir.path().join("a.rs");
        let md = dir.path().join("b.md");
        std::fs::write(&rs, "fn a() {}\n").unwrap();
        std::fs::write(&md, "# b").unwrap();

        let (session, _client) = recording_session();
        let routes = vec![SessionRoute::new(vec!["rs".to_string()], session)];
        // Only the .rs file routes to a session; .md matches no route.
        let n = refresh_changed_files(&routes, &[rs.clone(), md.clone()]);
        assert_eq!(n, 1, "only the .rs file routes to a session");
    }

    #[test]
    fn routing_sends_each_file_only_to_its_servers_session() {
        let dir = tempfile::tempdir().unwrap();
        let rs = dir.path().join("a.rs");
        let py = dir.path().join("b.py");
        std::fs::write(&rs, "fn a() {}\n").unwrap();
        std::fs::write(&py, "x = 1\n").unwrap();

        let (rust_session, rust_client) = recording_session();
        let (py_session, py_client) = recording_session();
        let routes = vec![
            SessionRoute::new(vec!["rs".to_string()], rust_session),
            SessionRoute::new(vec!["py".to_string()], py_session),
        ];

        let n = refresh_changed_files(&routes, &[rs.clone(), py.clone()]);
        assert_eq!(n, 2, "both files route to a session");

        // The rust session only ever saw the .rs file, the python session only
        // the .py file — no cross-language didChange.
        let rust = rust_client.lock().unwrap();
        let rt = rust.as_ref().unwrap();
        assert_eq!(rt.notification_count("textDocument/didOpen"), 1);
        let rs_uri = swissarmyhammer_lsp::file_uri_from_path(&rs.to_string_lossy());
        assert!(!rt.notifications.is_empty());
        assert!(rt.notifications.iter().all(|(_, p)| p
            .to_string()
            .contains(rs_uri.trim_start_matches("file://"))
            || p.to_string().contains("a.rs")));

        let py_guard = py_client.lock().unwrap();
        let pt = py_guard.as_ref().unwrap();
        assert_eq!(pt.notification_count("textDocument/didOpen"), 1);
        assert!(pt
            .notifications
            .iter()
            .all(|(_, p)| p.to_string().contains("b.py")));
    }

    #[test]
    fn refresh_file_with_no_live_client_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let rs = dir.path().join("main.rs");
        std::fs::write(&rs, "fn main() {}\n").unwrap();

        // A session with no live client (None) — diagnosable, but cannot sync.
        let client: Arc<Mutex<Option<NullTransport>>> = Arc::new(Mutex::new(None));
        let session = LspSession::new(client, "rust");
        assert!(!refresh_file(&session, &rs));
    }

    /// Collect the paths a notifier is invoked for, so a test can assert the
    /// best-effort push fires once per refreshed file.
    fn recording_notifier() -> (WatcherNotifier, Arc<Mutex<Vec<PathBuf>>>) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_clone = Arc::clone(&seen);
        let notifier: WatcherNotifier =
            Arc::new(move |path: &Path| seen_clone.lock().unwrap().push(path.to_path_buf()));
        (notifier, seen)
    }

    #[test]
    fn refresh_changed_files_notified_pushes_once_per_refreshed_file() {
        let dir = tempfile::tempdir().unwrap();
        let rs = dir.path().join("a.rs");
        let md = dir.path().join("b.md");
        std::fs::write(&rs, "fn a() {}\n").unwrap();
        std::fs::write(&md, "# b").unwrap();

        let (session, _client) = recording_session();
        let routes = vec![SessionRoute::new(vec!["rs".to_string()], session)];
        let (notifier, seen) = recording_notifier();

        let n = refresh_changed_files_notified(&routes, &[rs.clone(), md.clone()], Some(&notifier));

        assert_eq!(n, 1, "only the .rs file routes to a session");
        // The push fires for the refreshed .rs file only — the unrouted .md is
        // never pushed because it was never refreshed.
        let seen = seen.lock().unwrap();
        assert_eq!(seen.as_slice(), &[rs], "push once per refreshed file");
    }

    #[test]
    fn refresh_changed_files_with_no_notifier_still_refreshes() {
        let dir = tempfile::tempdir().unwrap();
        let rs = dir.path().join("a.rs");
        std::fs::write(&rs, "fn a() {}\n").unwrap();

        let (session, _client) = recording_session();
        let routes = vec![SessionRoute::new(vec!["rs".to_string()], session)];

        // No notifier (foreign host with no relay) must not change the refresh
        // outcome — the push is a courtesy, never load-bearing.
        let n = refresh_changed_files_notified(&routes, std::slice::from_ref(&rs), None);
        assert_eq!(n, 1);
    }
}
