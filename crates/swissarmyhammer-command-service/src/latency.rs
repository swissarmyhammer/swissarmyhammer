//! Soft latency budget for the `available command` verb.
//!
//! The `available` callback is on the palette / menu hot path — a slow
//! check stalls every surface that asks "can the user run this command?".
//! The service enforces a two-tier soft budget:
//!
//! - Anything past [`AVAILABLE_WARN_THRESHOLD`] (5ms) is logged at WARN.
//! - Anything past [`AVAILABLE_HARD_DEADLINE`] (50ms) is force-cancelled and
//!   reported as `{ ok: false, reason: "available timeout" }`.
//!
//! "Soft" because nothing prevents a callback from blocking the isolate; the
//! enforcement is on the *waiter*, via `tokio::time::timeout`, so the
//! service-side request stops waiting at the deadline even if the isolate
//! is still running the function.

use std::future::Future;
use std::time::Duration;

use tokio::time::{timeout, Instant};

/// Latency past which an `available` call is logged at WARN.
///
/// Picked so a sub-frame check (the palette renders at 16ms / frame) still
/// passes silently while anything noticeable to the user generates a log
/// line operators can grep for.
pub const AVAILABLE_WARN_THRESHOLD: Duration = Duration::from_millis(5);

/// Hard deadline past which an `available` call is force-cancelled.
///
/// Three frames of stall is the upper bound on what the palette can absorb
/// before users perceive lag. Anything past this is treated as "the check
/// timed out" rather than waiting indefinitely.
pub const AVAILABLE_HARD_DEADLINE: Duration = Duration::from_millis(50);

/// The reason string returned when an `available` check exceeds
/// [`AVAILABLE_HARD_DEADLINE`].
///
/// Surfaced verbatim in the wire response (`{ ok: false, reason: "<this>" }`)
/// so callers can branch on the constant string.
pub const AVAILABLE_TIMEOUT_REASON: &str = "available timeout";

/// Outcome of running an `available` callback under the soft latency
/// budget.
///
/// Carries either the callback's settled result and the elapsed wall time,
/// or the elapsed wall time alone when the hard deadline forced
/// cancellation.
#[derive(Debug)]
pub enum AvailableLatencyOutcome<T> {
    /// The callback returned within the hard deadline. `elapsed` is the
    /// measured duration; callers check it against
    /// [`AVAILABLE_WARN_THRESHOLD`] to decide whether to log.
    Completed {
        /// The callback's settled result.
        result: T,
        /// Wall-clock time spent waiting for the callback.
        elapsed: Duration,
    },
    /// The callback exceeded [`AVAILABLE_HARD_DEADLINE`] and was
    /// force-cancelled. Its work is abandoned; the service returns the
    /// canned timeout response.
    TimedOut {
        /// Wall-clock time spent waiting before force-cancellation. Bounded
        /// above by [`AVAILABLE_HARD_DEADLINE`] plus a small overshoot.
        elapsed: Duration,
    },
}

/// Run `fut` under the soft `available` latency budget.
///
/// Returns [`AvailableLatencyOutcome::Completed`] with the callback's
/// settled result on completion within the deadline, or
/// [`AvailableLatencyOutcome::TimedOut`] when the hard deadline elapses
/// first.
///
/// Cancellation is "soft" in the sense that dropping `fut` is the only
/// signal the caller sends — the isolate on the far side of an `invoke`
/// may continue executing. The waiter, however, stops blocking the verb
/// handler at the deadline.
pub async fn run_with_available_budget<F, T>(fut: F) -> AvailableLatencyOutcome<T>
where
    F: Future<Output = T>,
{
    let start = Instant::now();
    match timeout(AVAILABLE_HARD_DEADLINE, fut).await {
        Ok(result) => AvailableLatencyOutcome::Completed {
            result,
            elapsed: start.elapsed(),
        },
        Err(_) => AvailableLatencyOutcome::TimedOut {
            elapsed: start.elapsed(),
        },
    }
}
