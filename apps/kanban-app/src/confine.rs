//! One place for "run a plugin future off the async worker pool".
//!
//! Several call sites need to drive a `PluginPlatform`-related future to
//! completion *without* doing it on a Tokio worker thread:
//!
//! - [`build_board_platform`](crate::state) builds + wires + discovers a
//!   per-board host; that span borrows `&PluginPlatform` (which is `Send` but
//!   not `Sync`, because its `PluginHost` carries V8 isolate state) across
//!   `.await` points, so it cannot run on a future that must stay `Send`.
//! - The synchronous [`WindowShell`](swissarmyhammer_window_service::WindowShell)
//!   methods (`open_new_window` / `switch_board` / `close_board` / …) are
//!   invoked from inside the dispatcher's async task, where a bare `block_on`
//!   panics ("Cannot start a runtime from within a runtime") and
//!   `switch_board` / `close_board` re-enter `AppState`'s board locks.
//! - [`BoardHandle::drop`](crate::state) drops a per-board `PluginPlatform`
//!   whose `BridgeRuntime::drop` does a blocking thread-`join()`.
//!
//! These three previously each spun up a *fresh* OS thread with its own
//! current-thread runtime per call (`std::thread::scope` + join, or
//! `spawn_blocking` + a new runtime). That is the same confinement strategy
//! implemented three different ways — and the per-call thread+runtime build is
//! pure overhead.
//!
//! This module owns ONE long-lived dedicated multi-thread runtime (the
//! *confinement runtime*) and routes all three through it:
//!
//! - [`run_confined`] runs a `Send` closure on the confinement runtime's
//!   blocking pool and blocks the caller until it returns. The closure is given
//!   the runtime [`Handle`] and may `block_on` an arbitrarily `!Send` future,
//!   which is created and driven entirely on the confinement runtime — never on
//!   a Tokio worker, so it cannot starve or deadlock the main worker pool the
//!   way nesting would. The runtime is multi-threaded so a confined job that
//!   itself confines more work (e.g. `switch_board` → `open_board` →
//!   `build_board_platform`) runs the nested job on a *different* blocking
//!   thread instead of deadlocking a single consumer.
//! - [`run_future`] is the convenience wrapper for an already-`Send` future.
//! - [`drop_confined`] moves an owned value onto the confinement runtime to be
//!   dropped there, so a blocking `Drop` never runs on a worker (or while a
//!   lock is held).

use std::panic::{self, AssertUnwindSafe};
use std::sync::mpsc;
use std::sync::OnceLock;
use tokio::runtime::{Handle, Runtime};

/// The long-lived confinement runtime, started on first use and kept for the
/// rest of the process.
///
/// Leaked (`'static`) intentionally: the work it confines (GUI lifecycle
/// plumbing — open a window, switch a board, tear a host down) is needed until
/// the app exits, and there is no clean point to shut it down. A multi-thread
/// runtime so nested confinement (a confined job that confines more work) runs
/// on a fresh blocking thread rather than deadlocking.
static CONFINEMENT: OnceLock<&'static Runtime> = OnceLock::new();

/// The confinement runtime handle, building the runtime on first use.
fn runtime() -> &'static Runtime {
    CONFINEMENT.get_or_init(|| {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .thread_name("kanban-plugin-confinement")
            .enable_all()
            .build()
            .expect("confinement runtime must build");
        Box::leak(Box::new(rt))
    })
}

/// Run a `Send` closure on the confinement runtime, blocking the caller until
/// it returns a `Send` value.
///
/// The closure is handed the confinement runtime's [`Handle`], on which it can
/// `block_on` an arbitrarily `!Send` future — that future is created and driven
/// entirely on the confinement runtime, so only the `Send` closure and its
/// `Send` result ever cross the thread boundary. This is what lets the per-board
/// host build (a `!Send` span borrowing `&PluginPlatform` across awaits) run
/// here without tainting its `Send`-bound caller, while the synchronous
/// `WindowShell` ops use it via the [`run_future`] convenience.
///
/// The job runs on the confinement runtime's blocking pool (via
/// [`Handle::spawn_blocking`]), so `block_on` inside it is legal and a confined
/// job that confines more work does not deadlock. Any `AppState` lock the work
/// re-acquires is taken on the confinement runtime, not nested under a blocked
/// Tokio worker, so it cannot starve the main worker pool.
pub(crate) fn run_confined<T, J>(job: J) -> T
where
    J: FnOnce(&Handle) -> T + Send + 'static,
    T: Send + 'static,
{
    let rt = runtime();
    let handle = rt.handle().clone();
    // `spawn_blocking` runs the job on the confinement runtime's blocking pool;
    // `block_on` inside the job is legal there (a blocking thread is not a
    // runtime worker). The result returns over a plain channel: `recv()` is an
    // ordinary blocking wait, legal on any thread (including a Tokio worker, the
    // sync `WindowShell` caller) — unlike `Runtime::block_on`, it never nests a
    // runtime. The caller blocks only until the confinement runtime finishes.
    //
    // The job is wrapped in `catch_unwind` so a panic inside it crosses the
    // channel as the captured payload rather than dropping `result_tx` and
    // surfacing as a generic "no result" error on the caller. We then
    // `resume_unwind` that payload on the caller's thread, preserving the
    // original panic message/backtrace instead of masking it. `AssertUnwindSafe`
    // is sound here because the payload is re-raised — we are not observing
    // potentially-broken state across the boundary, only propagating the panic.
    let (result_tx, result_rx) = mpsc::sync_channel::<std::thread::Result<T>>(1);
    rt.spawn_blocking(move || {
        let output = panic::catch_unwind(AssertUnwindSafe(|| job(&handle)));
        // Receiver only gone if the caller was cancelled; dropping is correct.
        let _ = result_tx.send(output);
    });
    match result_rx.recv() {
        Ok(Ok(value)) => value,
        // Re-raise the confined job's panic on the caller so its original
        // payload and backtrace surface, rather than a generic wrapper panic.
        Ok(Err(payload)) => panic::resume_unwind(payload),
        // The job never produced a result and never panicked into the channel —
        // the only way is the blocking thread itself vanished. Keep the no-hang
        // guarantee by failing loudly instead of blocking forever.
        Err(_) => panic!("confinement runtime dropped the result channel without a value"),
    }
}

/// Drive a `Send` future to completion on the confinement runtime, blocking the
/// caller until it produces a value.
///
/// A thin wrapper over [`run_confined`] for the common case where the work is
/// already a `Send` future (e.g. the synchronous `WindowShell` callbacks, which
/// would otherwise nest a runtime on a Tokio worker).
pub(crate) fn run_future<F>(fut: F) -> F::Output
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    run_confined(move |handle: &Handle| handle.block_on(fut))
}

/// Run a `Send` closure on the confinement runtime, returning a `JoinHandle` the
/// caller can `.await` instead of blocking.
///
/// The asynchronous counterpart to [`run_confined`], for a confinement caller
/// that is itself `async` and runs on a Tokio worker (e.g.
/// [`build_board_platform`](crate::state)). The job is `spawn_blocking`'d onto
/// the confinement runtime — keeping the `!Send` build span off the main worker
/// pool — and the returned handle is awaited on the caller's runtime, so the
/// main worker is freed rather than blocked. The job runs on a confinement
/// blocking thread, so a `block_on` inside it is legal.
pub(crate) fn spawn_confined<T, J>(job: J) -> tokio::task::JoinHandle<T>
where
    J: FnOnce(&Handle) -> T + Send + 'static,
    T: Send + 'static,
{
    let handle = runtime().handle().clone();
    runtime().spawn_blocking(move || job(&handle))
}

/// Drop an owned value on the confinement runtime thread.
///
/// For values whose `Drop` blocks (e.g. a `PluginPlatform`, whose
/// `BridgeRuntime::drop` joins a thread): moving the value here keeps that
/// blocking teardown off the Tokio worker pool and out from under any lock the
/// caller holds. Fire-and-forget — the caller does not wait for the drop.
pub(crate) fn drop_confined<T: Send + 'static>(value: T) {
    runtime().spawn_blocking(move || {
        drop(value);
    });
}
