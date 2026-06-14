//! Debounce + flush tests for the `notifications/commands/changed` emitter.
//!
//! The contract is:
//!
//! - Multiple rapid `notify()` calls within the debounce window collapse
//!   into a single emission (the receiver sees one tick, not N).
//! - A `flush()` call drains any pending notification immediately, so the
//!   platform can guarantee subscribers see the latest state at plugin-load
//!   / unload boundaries without waiting on the debounce timer.
//!
//! These tests use the real tokio clock with a short debounce window
//! (40ms) so the wall-clock cost stays under 200ms total. They do not
//! depend on tokio's paused-time `test-util` feature, which keeps the
//! crate's `[dev-dependencies]` minimal.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use swissarmyhammer_command_service::ChangeNotifier;

/// Debounce window used by every test. Short enough to keep the suite
/// fast, long enough that the worker scheduling jitter (~1ms) does not
/// race the "still inside the window" assertion.
const DEBOUNCE: Duration = Duration::from_millis(40);

/// Buffer added to the debounce window when waiting for an emit. Soaks up
/// scheduling jitter without making the suite slow.
const SETTLE: Duration = Duration::from_millis(60);

/// Build a notifier whose sink increments a shared counter. Returns
/// `(notifier, counter)`.
fn counting_notifier() -> (ChangeNotifier, Arc<AtomicUsize>) {
    let count = Arc::new(AtomicUsize::new(0));
    let count_clone = count.clone();
    let notifier = ChangeNotifier::new(DEBOUNCE, move || {
        count_clone.fetch_add(1, Ordering::SeqCst);
    });
    (notifier, count)
}

#[tokio::test]
async fn rapid_notifies_collapse_into_one_emission() {
    let (notifier, count) = counting_notifier();

    // 5 rapid notifies inside one debounce window.
    for _ in 0..5 {
        notifier.notify();
    }

    // Wait past the debounce window — exactly one emission should fire.
    tokio::time::sleep(DEBOUNCE + SETTLE).await;
    assert_eq!(
        count.load(Ordering::SeqCst),
        1,
        "5 rapid notifies should collapse into 1 emission"
    );
}

#[tokio::test]
async fn flush_emits_pending_notification_immediately() {
    let (notifier, count) = counting_notifier();

    notifier.notify();
    // No time has passed — debounce timer hasn't fired.
    assert_eq!(count.load(Ordering::SeqCst), 0);

    notifier.flush();
    // Flush drained the pending notify immediately.
    assert_eq!(
        count.load(Ordering::SeqCst),
        1,
        "flush should drain a pending notification immediately"
    );

    // Subsequent timer fire should not double-emit.
    tokio::time::sleep(DEBOUNCE + SETTLE).await;
    assert_eq!(
        count.load(Ordering::SeqCst),
        1,
        "the debounce timer must not double-emit after a flush"
    );
}

#[tokio::test]
async fn flush_with_no_pending_notification_is_a_noop() {
    let (notifier, count) = counting_notifier();

    notifier.flush();
    notifier.flush();
    assert_eq!(
        count.load(Ordering::SeqCst),
        0,
        "flush with nothing pending should not emit"
    );
}

#[tokio::test]
async fn separate_debounce_windows_emit_separately() {
    let (notifier, count) = counting_notifier();

    notifier.notify();
    tokio::time::sleep(DEBOUNCE + SETTLE).await;
    assert_eq!(count.load(Ordering::SeqCst), 1);

    notifier.notify();
    tokio::time::sleep(DEBOUNCE + SETTLE).await;
    assert_eq!(
        count.load(Ordering::SeqCst),
        2,
        "second window should produce its own emission"
    );
}
