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
