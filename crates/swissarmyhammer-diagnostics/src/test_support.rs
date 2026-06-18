//! Shared test-only helpers: a deterministic virtual clock and a no-op LSP
//! transport. Both the settle engine and the `diagnose` tests drive the async
//! timers and a `None`-client session, so these live in one place rather than
//! being copied per test module.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;
use tokio::sync::oneshot;

use swissarmyhammer_lsp::client::LspTransport;
use swissarmyhammer_lsp::LspError;

use crate::settle::Timer;

/// A deterministic virtual clock for tests.
///
/// [`Timer::sleep`] registers a waiter at `now + dur`; [`advance`](Self::advance)
/// moves `now` forward and completes every waiter whose deadline has passed. No
/// real time elapses, so a whole settle/diagnose scenario runs in microseconds
/// and is fully reproducible. Cloneable so a test keeps one handle to drive time
/// while the engine holds another to register sleeps.
#[derive(Clone, Default)]
pub struct ManualTimer {
    inner: Arc<Mutex<ManualInner>>,
}

#[derive(Default)]
struct ManualInner {
    now: Duration,
    waiters: Vec<(Duration, oneshot::Sender<()>)>,
}

impl Timer for ManualTimer {
    fn sleep(&self, dur: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        let (tx, rx) = oneshot::channel();
        {
            let mut inner = self.inner.lock().unwrap();
            let deadline = inner.now + dur;
            if deadline <= inner.now {
                let _ = tx.send(());
            } else {
                inner.waiters.push((deadline, tx));
            }
        }
        Box::pin(async move {
            let _ = rx.await;
        })
    }
}

impl ManualTimer {
    /// Advance the virtual clock and fire every waiter now due.
    pub fn advance(&self, dur: Duration) {
        let mut inner = self.inner.lock().unwrap();
        inner.now += dur;
        let now = inner.now;
        let mut still_waiting = Vec::new();
        for (deadline, tx) in std::mem::take(&mut inner.waiters) {
            if deadline <= now {
                let _ = tx.send(());
            } else {
                still_waiting.push((deadline, tx));
            }
        }
        inner.waiters = still_waiting;
    }
}

/// A do-nothing [`LspTransport`] for tests that drive an
/// [`LspSession`](swissarmyhammer_lsp::LspSession) purely through its
/// diagnostics cache/fan-out (via `handle_publish_diagnostics`) with no live
/// server. Built with a `None` client, so these methods are never called.
pub struct NullTransport;

impl LspTransport for NullTransport {
    fn send_request(&mut self, _method: &str, _params: Value) -> Result<Value, LspError> {
        Err(LspError::NotRunning)
    }

    fn send_notification(&mut self, _method: &str, _params: Value) -> Result<(), LspError> {
        Err(LspError::NotRunning)
    }

    fn read_message(&mut self) -> Result<Value, LspError> {
        Err(LspError::NotRunning)
    }
}

/// A recording [`LspTransport`] that captures the wire traffic a flow emits and
/// answers `textDocument/diagnostic` with a scripted full report.
///
/// Used to assert what the leader watcher pushes (`didOpen`/`didChange`) and
/// that it pulls diagnostics, without a real language server. Every request and
/// notification is recorded in order; a `textDocument/diagnostic` request
/// returns [`diagnostic_response`](Self::diagnostic_response) (default: one
/// scripted error), any other request returns an empty object.
#[derive(Default)]
pub struct RecordingTransport {
    /// `(method, params)` of every request, in order.
    pub requests: Vec<(String, Value)>,
    /// `(method, params)` of every notification, in order.
    pub notifications: Vec<(String, Value)>,
    /// The result returned for a `textDocument/diagnostic` request. Defaults to
    /// a one-item "full" report carrying a single error.
    pub diagnostic_response: Option<Value>,
}

impl RecordingTransport {
    /// Count notifications recorded for `method`.
    pub fn notification_count(&self, method: &str) -> usize {
        swissarmyhammer_lsp::count_recorded_method(&self.notifications, method)
    }

    /// Count requests recorded for `method`.
    pub fn request_count(&self, method: &str) -> usize {
        swissarmyhammer_lsp::count_recorded_method(&self.requests, method)
    }
}

impl LspTransport for RecordingTransport {
    fn send_request(&mut self, method: &str, params: Value) -> Result<Value, LspError> {
        self.requests.push((method.to_string(), params));
        if method == "textDocument/diagnostic" {
            Ok(self.diagnostic_response.clone().unwrap_or_else(|| {
                serde_json::json!({
                    "kind": "full",
                    "items": [{
                        "range": {
                            "start": { "line": 0, "character": 0 },
                            "end": { "line": 0, "character": 5 }
                        },
                        "severity": 1,
                        "message": "scripted error"
                    }]
                })
            }))
        } else {
            Ok(serde_json::json!({}))
        }
    }

    fn send_notification(&mut self, method: &str, params: Value) -> Result<(), LspError> {
        self.notifications.push((method.to_string(), params));
        Ok(())
    }

    fn read_message(&mut self) -> Result<Value, LspError> {
        Err(LspError::NotRunning)
    }
}
