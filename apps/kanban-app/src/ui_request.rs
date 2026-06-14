//! Host→UI request/reply channel: a generic primitive that lets host-side
//! code ASK the webview a question and AWAIT the answer.
//!
//! Host→UI traffic is otherwise fire-and-forget `emit_to` (see
//! [`crate::commands::spawn_window_forwarder`] and
//! [`crate::command_services`]'s focus event sink); the host registers no
//! listener for UI-emitted events, so there is no reply path. This module
//! adds one, correlated by a per-request id:
//!
//! 1. The host calls [`request_from_ui`], which registers a [`oneshot`]
//!    sender under a fresh id, emits a `ui/request` Tauri event to the named
//!    window carrying `{ request_id, kind, params }`, then awaits the
//!    receiver with a timeout.
//! 2. The webview's `ui/request` listener dispatches on `kind`, computes a
//!    result, and `invoke("ui_request_reply", { request_id, result })`.
//! 3. The [`ui_request_reply`] Tauri command looks the sender up by id and
//!    fires it, resolving the host's `await`.
//!
//! ## Deadlock discipline (load-bearing)
//!
//! The reply travels back through the Tauri command thread, so the awaiting
//! host task MUST NOT hold any [`crate::state::AppState`] / spatial `Mutex`
//! across the `.await` — otherwise `ui_request_reply` (which may need those
//! locks, directly or transitively) could not run and the request would
//! deadlock until timeout. The API enforces the *registry's* own discipline:
//! its internal `Mutex` is released before the `await` (only the
//! [`oneshot::Receiver`] is held). Callers are responsible for dropping their
//! application locks before calling [`request_from_ui`] / awaiting its future.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::oneshot;

use crate::state::AppState;

/// The Tauri event name the host raises to ask the webview a question.
///
/// The webview's `ui-request-responder` listens on this event, dispatches by
/// `kind`, and replies via the `ui_request_reply` command.
pub const UI_REQUEST_EVENT: &str = "ui/request";

/// The default deadline for a host→UI request before it is abandoned.
///
/// Generous enough for a webview round-trip (listener dispatch + a layout
/// read), short enough that a closed window or missing responder fails fast
/// rather than hanging a host task indefinitely.
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Why a host→UI request did not produce a value.
#[derive(Debug)]
pub enum UiRequestError {
    /// The webview never replied within the deadline (window closed, no
    /// responder registered for the `kind`, or the responder hung).
    Timeout,
    /// The reply channel closed before a value arrived — the registry entry
    /// was cancelled or dropped without being fulfilled.
    Closed,
    /// Emitting the `ui/request` event to the target window failed.
    Emit(String),
}

impl std::fmt::Display for UiRequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Timeout => write!(f, "ui request timed out waiting for a reply"),
            Self::Closed => write!(f, "ui request channel closed before a reply arrived"),
            Self::Emit(e) => write!(f, "failed to emit ui request to window: {e}"),
        }
    }
}

impl std::error::Error for UiRequestError {}

/// The seam over "emit a `ui/request` event to a specific window".
///
/// Production uses the Tauri [`AppHandle`] (via the blanket impl over
/// [`Emitter`]); tests substitute a recording double so the
/// correlation/timeout logic is exercised without a real webview.
pub trait UiRequestEmitter {
    /// Emit `payload` to `window_label` under the Tauri event `event`.
    fn emit_request(
        &self,
        window_label: &str,
        event: &str,
        payload: &Value,
    ) -> Result<(), UiRequestError>;
}

impl<E: Emitter<tauri::Wry>> UiRequestEmitter for E {
    fn emit_request(
        &self,
        window_label: &str,
        event: &str,
        payload: &Value,
    ) -> Result<(), UiRequestError> {
        self.emit_to(window_label, event, payload)
            .map_err(|e| UiRequestError::Emit(e.to_string()))
    }
}

/// Correlates in-flight host→UI requests to their replies.
///
/// A request inserts a [`oneshot::Sender`] under a fresh id; the matching
/// reply (routed through [`ui_request_reply`]) removes and fires it. The
/// internal `Mutex` guards only the id→sender map and is never held across an
/// `.await` — see the module-level deadlock discipline.
#[derive(Default)]
pub struct UiRequestRegistry {
    pending: Mutex<HashMap<String, oneshot::Sender<Value>>>,
}

impl UiRequestRegistry {
    /// Register a new in-flight request, returning its id and the receiver the
    /// caller awaits. The id is a fresh ULID (monotonic, collision-free —
    /// matches the project's id convention; never `Math.random`/`Date::now`).
    pub fn register(&self) -> (String, oneshot::Receiver<Value>) {
        let id = ulid::Ulid::new().to_string();
        let (tx, rx) = oneshot::channel();
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id.clone(), tx);
        (id, rx)
    }

    /// Deliver `value` to the request with `id`. Returns `true` if a waiting
    /// request matched (and was resolved), `false` for an unknown id — e.g. a
    /// reply that races in after its request already timed out. Either way no
    /// sender is leaked.
    pub fn fulfill(&self, id: &str, value: Value) -> bool {
        let sender = self
            .pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(id);
        match sender {
            // `send` errors only if the receiver was dropped (caller gave up);
            // the entry is removed regardless, so nothing leaks.
            Some(tx) => tx.send(value).is_ok(),
            None => false,
        }
    }

    /// Drop the pending entry for `id` without delivering a value — used to
    /// clean up after a timeout so a never-arriving reply leaks no sender.
    pub fn cancel(&self, id: &str) {
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(id);
    }

    /// Number of requests currently awaiting a reply. Test/diagnostic aid for
    /// asserting no sender is leaked.
    #[cfg(test)]
    pub fn pending_count(&self) -> usize {
        self.pending.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    /// Core request/await logic, generic over the emit seam.
    ///
    /// Registers a request, emits `{ request_id, kind, params }` to
    /// `window_label`, then awaits the reply up to `timeout`. On timeout or a
    /// closed channel the pending entry is cancelled before returning `Err`,
    /// so nothing leaks. The registry `Mutex` is released by [`register`]
    /// before the `.await` — only the [`oneshot::Receiver`] is held.
    ///
    /// [`register`]: Self::register
    pub async fn request_with_emitter<E: UiRequestEmitter>(
        &self,
        emitter: &E,
        window_label: &str,
        kind: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, UiRequestError> {
        let (request_id, rx) = self.register();

        let payload = json!({
            "request_id": request_id,
            "kind": kind,
            "params": params,
            // The target window. The webview responder bus answers ONLY when
            // this matches its own window label — `emit_to` is not reliably
            // confined in a multi-window app (every window's global `listen`
            // receives it), so without this guard a non-target window can reply
            // first (often with null) and win the correlation.
            "window": window_label,
        });
        if let Err(e) = emitter.emit_request(window_label, UI_REQUEST_EVENT, &payload) {
            // Emit failed: the reply can never arrive, so don't strand the entry.
            self.cancel(&request_id);
            return Err(e);
        }

        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(_recv_closed)) => {
                self.cancel(&request_id);
                Err(UiRequestError::Closed)
            }
            Err(_elapsed) => {
                self.cancel(&request_id);
                Err(UiRequestError::Timeout)
            }
        }
    }
}

/// The process-wide registry of in-flight host→UI requests.
static UI_REQUESTS: LazyLock<UiRequestRegistry> = LazyLock::new(UiRequestRegistry::default);

/// Ask the webview at `window_label` a `kind` question and await its reply.
///
/// Emits a `ui/request` Tauri event carrying `{ request_id, kind, params }`
/// and resolves with the value the webview's responder returns via
/// `ui_request_reply`. Returns [`UiRequestError::Timeout`] if no reply arrives
/// within [`DEFAULT_REQUEST_TIMEOUT`] (window closed / no responder), or
/// [`UiRequestError::Emit`] if the event could not be delivered.
///
/// # Deadlock safety
///
/// The reply is delivered on the `ui_request_reply` command thread, which may
/// contend for [`AppState`] / spatial locks. The caller MUST drop any such
/// locks before awaiting this future — see the module docs. This function
/// itself holds no application lock across its `.await`.
///
/// The first in-tree caller is the focus kernel's [`UiGeometryProvider`]
/// implementation (`crate::command_services::TauriUiGeometryProvider`), which
/// pulls live geometry / scope chain / focus from the webview on demand.
pub async fn request_from_ui(
    app: &AppHandle,
    window_label: &str,
    kind: &str,
    params: Value,
) -> Result<Value, UiRequestError> {
    UI_REQUESTS
        .request_with_emitter(app, window_label, kind, params, DEFAULT_REQUEST_TIMEOUT)
        .await
}

/// Tauri command: the webview's reply to a host→UI [`request_from_ui`].
///
/// The responder calls `invoke("ui_request_reply", { request_id, result })`;
/// this looks the awaiting sender up by `request_id` and fires it with
/// `result`. An unknown id (its request already timed out) is a silent no-op.
///
/// Takes [`State<AppState>`] only to match this file's command convention and
/// keep the handler discoverable alongside the others; the correlation state
/// lives in the module-global [`UI_REQUESTS`] registry, not on `AppState`.
#[tauri::command]
pub async fn ui_request_reply(
    _state: State<'_, AppState>,
    request_id: String,
    result: Value,
) -> Result<(), String> {
    if !UI_REQUESTS.fulfill(&request_id, result) {
        tracing::debug!(
            request_id = %request_id,
            "ui_request_reply: no in-flight request for id (already timed out?)"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::Duration;

    /// A request correlates to the reply carrying its own id: register a
    /// request, fulfill it by that id, and the awaited value is the reply.
    #[tokio::test]
    async fn request_resolves_with_matching_reply() {
        let registry = UiRequestRegistry::default();
        let (id, rx) = registry.register();

        assert!(registry.fulfill(&id, json!({ "answer": 42 })));
        let value = rx.await.expect("sender should not be dropped");
        assert_eq!(value, json!({ "answer": 42 }));
        assert_eq!(registry.pending_count(), 0, "no leaked sender");
    }

    /// Two concurrent in-flight requests correlate independently: each id
    /// resolves with its own reply, no cross-talk.
    #[tokio::test]
    async fn concurrent_requests_correlate_by_id() {
        let registry = UiRequestRegistry::default();
        let (id_a, rx_a) = registry.register();
        let (id_b, rx_b) = registry.register();
        assert_ne!(id_a, id_b, "each request gets a distinct id");

        // Fulfill out of registration order to prove correlation, not ordering.
        assert!(registry.fulfill(&id_b, json!("bee")));
        assert!(registry.fulfill(&id_a, json!("aye")));

        assert_eq!(rx_a.await.unwrap(), json!("aye"));
        assert_eq!(rx_b.await.unwrap(), json!("bee"));
        assert_eq!(registry.pending_count(), 0);
    }

    /// Fulfilling an unknown id is a no-op (returns false) and leaks nothing —
    /// e.g. a reply that arrives after its request already timed out.
    #[tokio::test]
    async fn fulfill_unknown_id_is_noop() {
        let registry = UiRequestRegistry::default();
        assert!(!registry.fulfill("does-not-exist", json!(null)));
        assert_eq!(registry.pending_count(), 0);
    }

    /// `request_from_ui` returns its emitted request envelope and times out
    /// cleanly when no reply arrives — the sender is removed, not leaked.
    #[tokio::test]
    async fn request_from_ui_times_out_without_reply() {
        let registry = UiRequestRegistry::default();
        let emitter = RecordingEmitter::default();

        let result = registry
            .request_with_emitter(
                &emitter,
                "win-main",
                "focus.geometry",
                json!({ "fqm": "/window" }),
                Duration::from_millis(20),
            )
            .await;

        assert!(matches!(result, Err(UiRequestError::Timeout)));
        assert_eq!(registry.pending_count(), 0, "timed-out sender removed");

        // The host emitted exactly one `ui/request` to the named window,
        // carrying the generated id + kind + params.
        let emitted = emitter.emitted();
        assert_eq!(emitted.len(), 1);
        let (label, event, payload) = &emitted[0];
        assert_eq!(label, "win-main");
        assert_eq!(event, UI_REQUEST_EVENT);
        assert_eq!(payload["kind"], json!("focus.geometry"));
        assert_eq!(payload["params"], json!({ "fqm": "/window" }));
        assert!(payload["request_id"].is_string());
    }

    /// The happy path through `request_with_emitter`: a reply delivered via the
    /// registry (the seam the `ui_request_reply` command uses) resolves the
    /// awaited request with that value — proving the emit→reply round-trip
    /// without a real webview.
    #[tokio::test]
    async fn request_from_ui_resolves_when_reply_arrives() {
        let registry = std::sync::Arc::new(UiRequestRegistry::default());
        let emitter = RecordingEmitter::default();

        let reg = registry.clone();
        let emit = emitter.clone();
        let join = tokio::spawn(async move {
            reg.request_with_emitter(
                &emit,
                "win-main",
                "focus.geometry",
                json!({}),
                Duration::from_secs(5),
            )
            .await
        });

        // Wait for the request to be emitted, then reply by its id.
        let id = loop {
            if let Some((_, _, payload)) = emitter.emitted().into_iter().next() {
                break payload["request_id"].as_str().unwrap().to_string();
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        };
        assert!(registry.fulfill(&id, json!({ "x": 1, "y": 2 })));

        let value = join.await.unwrap().expect("request resolves");
        assert_eq!(value, json!({ "x": 1, "y": 2 }));
        assert_eq!(registry.pending_count(), 0);
    }

    /// A test double for the emit seam: records every `(label, event, payload)`
    /// instead of touching a real Tauri webview.
    #[derive(Clone, Default)]
    struct RecordingEmitter {
        emitted: std::sync::Arc<std::sync::Mutex<Vec<(String, String, serde_json::Value)>>>,
    }

    impl RecordingEmitter {
        fn emitted(&self) -> Vec<(String, String, serde_json::Value)> {
            self.emitted.lock().unwrap().clone()
        }
    }

    impl UiRequestEmitter for RecordingEmitter {
        fn emit_request(
            &self,
            window_label: &str,
            event: &str,
            payload: &serde_json::Value,
        ) -> Result<(), UiRequestError> {
            self.emitted.lock().unwrap().push((
                window_label.to_string(),
                event.to_string(),
                payload.clone(),
            ));
            Ok(())
        }
    }
}
