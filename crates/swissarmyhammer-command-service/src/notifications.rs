//! Debounced `notifications/commands/changed` emitter.
//!
//! Registry changes (register / unregister / purge) call [`ChangeNotifier::notify`].
//! The notifier coalesces a burst of changes within a small window
//! (default 100ms) into a single emission so subscribers see one "registry
//! changed" tick per logical batch instead of one per registration.
//!
//! On plugin load and unload boundaries the platform calls
//! [`ChangeNotifier::flush`] so subscribers see the post-batch state before
//! the next batch starts — the debounce timer is bypassed and any pending
//! emission fires immediately.
//!
//! The emit side is a plain `Fn()` closure: this layer is decoupled from
//! the rmcp transport. The service layer wires a closure that emits an
//! rmcp `notifications/commands/changed` to its peer; tests wire a closure
//! that increments a counter.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::Notify;
use tokio::task::JoinHandle;

/// Boxed, thread-safe emit callback. Held in an `Arc` so the background
/// debounce task and the public API can share the same sink.
type Emit = Arc<dyn Fn() + Send + Sync + 'static>;

/// State shared between the public [`ChangeNotifier`] handle and the
/// background debounce task. Wrapped in a `Mutex` so `notify` / `flush` can
/// flip the `pending` flag atomically with respect to the worker.
#[derive(Debug, Default)]
struct State {
    /// `true` when at least one [`ChangeNotifier::notify`] call has
    /// happened since the last emission. Reset to `false` after an emit.
    pending: bool,
}

/// Debounced emitter for `notifications/commands/changed`.
///
/// A `ChangeNotifier` owns a background task that wakes on
/// [`Self::notify`], sleeps for the debounce window, then emits exactly
/// once (whether one or many notifies happened during the window). A
/// [`Self::flush`] call short-circuits the timer and emits immediately if
/// a notification is pending.
pub struct ChangeNotifier {
    state: Arc<Mutex<State>>,
    notify_signal: Arc<Notify>,
    emit: Emit,
    worker: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for ChangeNotifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChangeNotifier")
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl ChangeNotifier {
    /// Create a notifier that emits `emit` at most once per `debounce`
    /// window. Spawns a background debounce task on the current tokio
    /// runtime — must be called from inside a tokio runtime (production
    /// uses the platform's runtime; tests use `#[tokio::test]`).
    pub fn new<F>(debounce: Duration, emit: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        let state = Arc::new(Mutex::new(State::default()));
        let notify_signal = Arc::new(Notify::new());
        let emit: Emit = Arc::new(emit);

        let worker = tokio::spawn(run_debounce_loop(
            state.clone(),
            notify_signal.clone(),
            emit.clone(),
            debounce,
        ));

        Self {
            state,
            notify_signal,
            emit,
            worker: Some(worker),
        }
    }

    /// Mark a registry change. Coalesces with other pending notifies in the
    /// current debounce window.
    pub fn notify(&self) {
        {
            let mut state = self.state.lock().expect("notifier state lock poisoned");
            state.pending = true;
        }
        // Wake the worker; if it is already in the debounce sleep this is a
        // no-op (one waker is buffered), which is exactly the coalesce
        // behavior we want.
        self.notify_signal.notify_one();
    }

    /// Drain any pending notification immediately, bypassing the debounce
    /// timer. Idempotent — calling `flush` with nothing pending is a no-op.
    pub fn flush(&self) {
        let should_emit = {
            let mut state = self.state.lock().expect("notifier state lock poisoned");
            let was_pending = state.pending;
            state.pending = false;
            was_pending
        };
        if should_emit {
            (self.emit)();
        }
    }
}

impl Drop for ChangeNotifier {
    /// Aborts the background debounce task so dropping the notifier doesn't
    /// leak a tokio task. A `flush` before drop is the caller's
    /// responsibility — the platform calls `flush` on load / unload
    /// boundaries explicitly.
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            worker.abort();
        }
    }
}

/// Background loop: wait for a notify, sleep for the debounce window, then
/// emit exactly once. Runs until the worker handle is aborted (i.e. until
/// the notifier is dropped).
async fn run_debounce_loop(
    state: Arc<Mutex<State>>,
    notify_signal: Arc<Notify>,
    emit: Emit,
    debounce: Duration,
) {
    loop {
        // Wait for the next notify before doing anything.
        notify_signal.notified().await;

        // Debounce window — additional notifies during the sleep merge into
        // this same emission cycle because they just re-arm the `pending`
        // flag while we are already sleeping.
        tokio::time::sleep(debounce).await;

        // Emit if still pending. A flush during the sleep may have already
        // drained the flag, in which case we skip the emit.
        let should_emit = {
            let mut state = state.lock().expect("notifier state lock poisoned");
            let was_pending = state.pending;
            state.pending = false;
            was_pending
        };
        if should_emit {
            emit();
        }
    }
}
