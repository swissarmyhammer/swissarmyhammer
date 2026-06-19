//! Subscribable diagnostics MCP resource.
//!
//! Exposes the workspace's diagnostics as a single MCP resource
//! ([`DIAGNOSTICS_RESOURCE_URI`]) that emits `notifications/resources/updated`
//! whenever diagnostics change, so a subscribing host gets diagnostics *without*
//! issuing a tool call. This is the second of the two best-effort courtesy
//! channels for the hardest case — a foreign host doing a native edit where the
//! tool never runs: the leader's file watcher detects the change, the session's
//! in-process fan-out delivers the new per-uri diagnostics, and this resource
//! re-publishes that change to subscribers as a resource update.
//!
//! ## Reuse, not a second mechanism
//!
//! The data source is the existing per-uri diagnostics cache: every
//! `DiagnosticUpdate` the session broadcasts is folded in here via
//! [`publish_diagnostics_update`], the same fan-out the cross-process bus tee
//! (`spawn_diagnostics_fan_out`) already consumes. The per-uri records reuse
//! [`swissarmyhammer_diagnostics::map`] — the same conversion the bus and the
//! pull-side `diagnostics` tool use — so there is one diagnostic record shape,
//! not a parallel one.
//!
//! The out-of-band push mirrors the prompt-list-changed watcher
//! (`McpFileWatcherCallback`): a `Peer<RoleServer>` captured at `initialize` is
//! held in a shared,
//! late-populated slot, and the resource fires `peer.notify_resource_updated`
//! when the view changes. Best-effort: a missing peer (no client connected) or a
//! transport error is logged and swallowed, never propagated to block the edit or
//! analysis path.

use std::collections::BTreeMap;
use std::sync::Arc;

use once_cell::sync::OnceCell;
use rmcp::model::{
    Annotated, ListResourcesResult, LoggingLevel, LoggingMessageNotificationParam, RawResource,
    ReadResourceResult, Resource, ResourceContents, ResourceUpdatedNotificationParam,
};
use rmcp::{Peer, RoleServer};
use swissarmyhammer_diagnostics::{map, DiagnosticRecord, DiagnosticsReport};
use swissarmyhammer_lsp::file_path_from_uri;
use tokio::sync::RwLock;

/// The single, stable URI of the subscribable diagnostics resource.
///
/// One aggregate resource (not one per file) keeps the subscription surface
/// simple: a host subscribes once and is told — via
/// `notifications/resources/updated` carrying this URI — whenever *any* file's
/// diagnostics change, then re-reads the resource for the full current set.
pub const DIAGNOSTICS_RESOURCE_URI: &str = "diagnostics://workspace";

/// Human-facing name of the diagnostics resource.
const DIAGNOSTICS_RESOURCE_NAME: &str = "Workspace Diagnostics";

/// Process-wide diagnostics resource, installed once and shared by the MCP
/// server's resource handlers and the in-process diagnostics fan-out.
///
/// Mirrors the process-wide [`LSP_SUPERVISOR`](crate::mcp::tools::code_context)
/// handle: the diagnostics view and the per-uri fan-out are per-workspace, not
/// per-request, so a single shared handle lets both the `ServerHandler` resource
/// methods and the fan-out tee reach the same view and peer.
static DIAGNOSTICS_RESOURCES: OnceCell<DiagnosticsResources> = OnceCell::new();

/// The diagnostics resource's shared state: the per-uri view and the late-bound
/// ACP/MCP peer used to push `notifications/resources/updated`.
#[derive(Clone, Default)]
pub struct DiagnosticsResources {
    /// Latest-per-uri diagnostics, keyed by `file://` URI. A `BTreeMap` keeps the
    /// serialized resource read deterministic (stable file order).
    view: Arc<RwLock<BTreeMap<String, Vec<DiagnosticRecord>>>>,
    /// The server peer, populated at `initialize`. `None` until a client
    /// connects; a push with no peer is a no-op (best-effort).
    peer: Arc<RwLock<Option<Peer<RoleServer>>>>,
}

impl DiagnosticsResources {
    /// Build an empty diagnostics resource.
    pub fn new() -> Self {
        Self::default()
    }

    /// Install the server peer captured at `initialize`, enabling pushes.
    ///
    /// Idempotent: a later connect replaces the peer. Best-effort — until a peer
    /// is set, view updates still accumulate; they simply are not pushed.
    pub async fn set_peer(&self, peer: Peer<RoleServer>) {
        *self.peer.write().await = Some(peer);
    }

    /// Fold a per-uri diagnostics update into the view and push a best-effort
    /// `notifications/resources/updated` to subscribers.
    ///
    /// `diagnostics` is the full latest set for `uri` (a replacement, matching
    /// the session cache's per-uri semantics); an empty set means "now clean".
    /// The records reuse [`swissarmyhammer_diagnostics::map`]. The notify is
    /// fire-and-forget: a missing peer or a transport error is logged and
    /// swallowed so a diagnostics change never blocks the edit path.
    pub async fn publish(&self, uri: &str, diagnostics: Vec<lsp_types::Diagnostic>) {
        let path = file_path_from_uri(uri);
        let records: Vec<DiagnosticRecord> =
            diagnostics.iter().map(|d| map(d, path.clone())).collect();
        self.view.write().await.insert(uri.to_string(), records);

        let peer = self.peer.read().await.clone();
        let Some(peer) = peer else {
            tracing::debug!(
                uri = %uri,
                "diagnostics resource updated but no peer connected; not pushed"
            );
            return;
        };
        if let Err(e) = peer
            .notify_resource_updated(ResourceUpdatedNotificationParam::new(
                DIAGNOSTICS_RESOURCE_URI,
            ))
            .await
        {
            tracing::debug!(error = %e, "diagnostics resources/updated push failed (best-effort)");
        }
    }

    /// Push a best-effort host-facing MCP `notifications/message` (logging) to
    /// the connected peer.
    ///
    /// This is the watcher-push courtesy channel: when the leader's file watcher
    /// detects a native edit, it tells the host a change was seen. For a foreign
    /// host it is a plain MCP `notifications/message`; in llama-agent the same
    /// notification relays through `NotifyingClientHandler::relay_logging_message`
    /// into an ACP `SessionUpdate`. Fire-and-forget: no peer or a transport error
    /// is logged and swallowed so the watcher's re-diagnose path is never blocked.
    pub async fn notify_host_log(&self, message: String) {
        let peer = self.peer.read().await.clone();
        let Some(peer) = peer else {
            tracing::debug!(%message, "watcher push but no peer connected; not sent");
            return;
        };
        let param = LoggingMessageNotificationParam {
            level: LoggingLevel::Info,
            logger: Some("diagnostics".to_string()),
            data: serde_json::json!({ "message": message }),
        };
        if let Err(e) = peer.notify_logging_message(param).await {
            tracing::debug!(error = %e, "watcher push notifications/message failed (best-effort)");
        }
    }

    /// The resources this server advertises (just the one diagnostics resource).
    pub fn list(&self) -> ListResourcesResult {
        ListResourcesResult {
            resources: vec![diagnostics_resource()],
            next_cursor: None,
            meta: None,
        }
    }

    /// Read the diagnostics resource: a JSON [`DiagnosticsReport`] over every
    /// per-uri record currently in the view, or `None` for any other URI.
    pub async fn read(&self, uri: &str) -> Option<ReadResourceResult> {
        if uri != DIAGNOSTICS_RESOURCE_URI {
            return None;
        }
        let view = self.view.read().await;
        let records: Vec<DiagnosticRecord> = view.values().flatten().cloned().collect();
        let report = DiagnosticsReport::new(records);
        let text = serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".to_string());
        Some(ReadResourceResult::new(vec![ResourceContents::text(
            text,
            DIAGNOSTICS_RESOURCE_URI,
        )]))
    }
}

/// The process-wide diagnostics resource, created on first access.
pub fn diagnostics_resources() -> &'static DiagnosticsResources {
    DIAGNOSTICS_RESOURCES.get_or_init(DiagnosticsResources::new)
}

/// Fold a per-uri diagnostics update into the process-wide diagnostics resource
/// and push `notifications/resources/updated` to subscribers (best-effort).
///
/// This is the single entry point the in-process diagnostics fan-out (and tests)
/// call to feed the resource. It exists as a free function so callers that hold a
/// `DiagnosticUpdate` need not reach into the server's internals.
pub fn publish_diagnostics_update(uri: &str, diagnostics: Vec<lsp_types::Diagnostic>) {
    let uri = uri.to_string();
    let resources = diagnostics_resources().clone();
    // The publish is async (it awaits the peer notify); spawn it so a synchronous
    // caller (the fan-out drain, a test) is never blocked on the transport.
    tokio::spawn(async move {
        resources.publish(&uri, diagnostics).await;
    });
}

/// Push a best-effort watcher-push `notifications/message` naming a file the
/// leader's watcher saw change on disk.
///
/// The watcher (in the rmcp-free `swissarmyhammer-diagnostics` crate) invokes a
/// [`WatcherNotifier`](swissarmyhammer_diagnostics::WatcherNotifier) closure that
/// routes here. Spawned so the watcher's synchronous refresh loop is never
/// blocked on the transport.
pub fn watcher_push_log(path: &std::path::Path) {
    let message = format!(
        "Diagnostics refreshed after a native edit to {}",
        path.display()
    );
    let resources = diagnostics_resources().clone();
    tokio::spawn(async move {
        resources.notify_host_log(message).await;
    });
}

/// Build the advertised diagnostics [`Resource`] descriptor.
fn diagnostics_resource() -> Resource {
    Annotated::new(
        RawResource::new(DIAGNOSTICS_RESOURCE_URI, DIAGNOSTICS_RESOURCE_NAME)
            .with_title("Workspace Diagnostics")
            .with_description(
                "Live LSP diagnostics for the workspace, as a settled per-file report. \
                 Subscribe to receive notifications/resources/updated on every change.",
            )
            .with_mime_type("application/json"),
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn warning_at(line: u32, message: &str) -> lsp_types::Diagnostic {
        lsp_types::Diagnostic {
            range: lsp_types::Range {
                start: lsp_types::Position { line, character: 0 },
                end: lsp_types::Position { line, character: 1 },
            },
            severity: Some(lsp_types::DiagnosticSeverity::WARNING),
            message: message.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn list_advertises_the_single_diagnostics_resource() {
        let resources = DiagnosticsResources::new();
        let listed = resources.list();
        assert_eq!(listed.resources.len(), 1);
        assert_eq!(listed.resources[0].uri, DIAGNOSTICS_RESOURCE_URI);
        assert_eq!(
            listed.resources[0].mime_type.as_deref(),
            Some("application/json")
        );
    }

    #[tokio::test]
    async fn read_reflects_published_diagnostics() {
        let resources = DiagnosticsResources::new();
        resources
            .publish(
                "file:///workspace/src/main.rs",
                vec![warning_at(1, "unused variable `x`")],
            )
            .await;

        let read = resources
            .read(DIAGNOSTICS_RESOURCE_URI)
            .await
            .expect("diagnostics uri reads");
        let text = match &read.contents[0] {
            ResourceContents::TextResourceContents { text, .. } => text,
            other => panic!("expected text contents, got {other:?}"),
        };
        assert!(text.contains("src/main.rs"), "read text: {text}");
        assert!(text.contains("unused variable `x`"), "read text: {text}");
    }

    #[tokio::test]
    async fn read_of_unknown_uri_is_none() {
        let resources = DiagnosticsResources::new();
        assert!(resources.read("diagnostics://nope").await.is_none());
    }

    #[tokio::test]
    async fn publish_replaces_latest_per_uri() {
        let resources = DiagnosticsResources::new();
        let uri = "file:///workspace/src/lib.rs";
        resources
            .publish(uri, vec![warning_at(1, "first"), warning_at(2, "second")])
            .await;
        // A later publish with an empty set means "now clean" for that uri.
        resources.publish(uri, vec![]).await;

        let read = resources.read(DIAGNOSTICS_RESOURCE_URI).await.unwrap();
        let text = match &read.contents[0] {
            ResourceContents::TextResourceContents { text, .. } => text,
            other => panic!("expected text contents, got {other:?}"),
        };
        assert!(
            !text.contains("first"),
            "stale diagnostics not replaced: {text}"
        );
        assert!(
            !text.contains("second"),
            "stale diagnostics not replaced: {text}"
        );
    }

    #[tokio::test]
    async fn publish_without_peer_is_a_noop_not_an_error() {
        // No peer set: publishing must still fold the view and must not panic or
        // error (best-effort courtesy channel).
        let resources = DiagnosticsResources::new();
        resources
            .publish("file:///workspace/a.rs", vec![warning_at(0, "x")])
            .await;
        let read = resources.read(DIAGNOSTICS_RESOURCE_URI).await.unwrap();
        let text = match &read.contents[0] {
            ResourceContents::TextResourceContents { text, .. } => text,
            other => panic!("expected text contents, got {other:?}"),
        };
        assert!(text.contains("a.rs"));
    }
}
