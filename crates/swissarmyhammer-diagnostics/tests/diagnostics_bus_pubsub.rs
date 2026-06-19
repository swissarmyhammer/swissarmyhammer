//! Integration: cross-process diagnostics fan-out over the leader-election bus.
//!
//! The leader publishes a [`DiagnosticsBusMessage`] over the **existing** ZMQ
//! pub/sub bus (via the public [`LeaderGuard::publish`]); a subscriber on the
//! same bus receives the per-uri update. This is the cross-process half of the
//! fan-out — model-free (no LSP server), exercising the real proxy/socket so it
//! lives as an integration test rather than a unit test (it binds ipc sockets).
//!
//! It uses the public election API typed with `DiagnosticsBusMessage` rather
//! than the crate-private `Publisher::connected`/`Subscriber::connected`, which
//! is exactly how a production leader process obtains its publisher.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use swissarmyhammer_diagnostics::record::{DiagnosticRecord, Range};
use swissarmyhammer_diagnostics::DiagnosticsBusMessage;
use swissarmyhammer_leader_election::{ElectionConfig, ElectionOutcome, LeaderElection};
use swissarmyhammer_lsp::DiagnosticSeverity;

/// How long to wait for a published message to arrive at a subscriber before
/// failing the test. Generous so a slow CI machine binding ipc sockets does not
/// flake; a working bus delivers in milliseconds.
const MESSAGE_RECEIVE_TIMEOUT_MS: u64 = 2000;

/// Settle time for a ZMQ SUB subscription to propagate through the proxy when
/// the test itself owns the subscriber and publishes inline right after.
const SUB_PROPAGATION_INLINE_MS: u64 = 300;

/// Settle time when the subscriber runs on a *separate thread* (the helper
/// case): slightly longer than [`SUB_PROPAGATION_INLINE_MS`] because the thread
/// must also be scheduled and open its socket before the subscription can
/// propagate, so the publish must wait a touch longer to avoid a lost update.
const SUB_PROPAGATION_THREAD_MS: u64 = 400;

/// One sample error record for `path`.
fn record(path: &str, message: &str) -> DiagnosticRecord {
    DiagnosticRecord {
        path: path.to_string(),
        range: Range {
            start_line: 1,
            start_character: 0,
            end_line: 1,
            end_character: 10,
        },
        severity: DiagnosticSeverity::Error,
        message: message.to_string(),
        code: Some("E0308".to_string()),
        source: Some("rustc".to_string()),
        containing_symbol: None,
    }
}

#[test]
fn leader_publishes_diagnostics_and_a_subscriber_receives_per_uri_update() {
    // Sockets live under a tempdir so the election is isolated and cleaned up.
    let dir = tempfile::tempdir().expect("tempdir");
    let config = ElectionConfig::new()
        .with_prefix("diag-bus-test")
        .with_base_dir(dir.path());

    // The workspace root is just an identity for the election hash.
    let workspace = dir.path().join("ws");
    let election: LeaderElection<DiagnosticsBusMessage> =
        LeaderElection::with_config(&workspace, config);

    let leader = match election.elect().expect("elect") {
        ElectionOutcome::Leader(guard) => guard,
        ElectionOutcome::Follower(_) => panic!("first election must be leader"),
    };

    // Subscribe on the same bus, filtered to the diagnostics topic.
    let subscriber = leader
        .subscribe(&[swissarmyhammer_diagnostics::DIAGNOSTICS_TOPIC])
        .expect("subscribe");

    // Let the SUB subscription propagate through the proxy before publishing.
    std::thread::sleep(Duration::from_millis(SUB_PROPAGATION_INLINE_MS));

    let msg = DiagnosticsBusMessage::new(
        "file:///repo/src/main.rs",
        vec![record("/repo/src/main.rs", "mismatched types")],
    );
    leader.publish(&msg).expect("publish");

    let received = subscriber
        .recv_timeout(Duration::from_millis(MESSAGE_RECEIVE_TIMEOUT_MS))
        .expect("a message should arrive")
        .expect("message decodes");

    assert_eq!(received.uri, "file:///repo/src/main.rs");
    assert_eq!(received.diagnostics.len(), 1);
    assert_eq!(received.diagnostics[0].path, "/repo/src/main.rs");
    assert_eq!(received.diagnostics[0].message, "mismatched types");
    assert_eq!(received.diagnostics[0].severity, DiagnosticSeverity::Error);

    drop(subscriber);
    drop(leader);
}

#[test]
fn subscribe_helper_receives_what_the_leader_publishes() {
    // Exercises the production follower-side helper
    // `subscribe_diagnostics_over_bus` against a real leader publish over the
    // existing proxy — the receive half of the cross-process fan-out.
    use std::sync::mpsc;

    let dir = tempfile::tempdir().expect("tempdir");
    let config = ElectionConfig::new()
        .with_prefix("diag-bus-sub")
        .with_base_dir(dir.path());
    let workspace = dir.path().join("ws");
    let election: LeaderElection<DiagnosticsBusMessage> =
        LeaderElection::with_config(&workspace, config);
    let leader = match election.elect().expect("elect") {
        ElectionOutcome::Leader(guard) => guard,
        ElectionOutcome::Follower(_) => panic!("first election must be leader"),
    };
    let backend = leader.bus_addresses().backend.clone();

    // Run the production subscriber helper on a thread (it blocks on recv);
    // forward each received message back to the test over a channel.
    let (tx, rx) = mpsc::channel::<DiagnosticsBusMessage>();
    let cancel = Arc::new(AtomicBool::new(false));
    let sub_cancel = Arc::clone(&cancel);
    let sub_thread = std::thread::spawn(move || {
        let _ = swissarmyhammer_diagnostics::subscribe_diagnostics_over_bus(
            &backend,
            &sub_cancel,
            |msg| {
                let _ = tx.send(msg);
            },
        );
    });

    // Let the subscription propagate, then publish.
    std::thread::sleep(Duration::from_millis(SUB_PROPAGATION_THREAD_MS));
    leader
        .publish(&DiagnosticsBusMessage::new(
            "file:///repo/src/lib.rs",
            vec![record("/repo/src/lib.rs", "boom")],
        ))
        .expect("publish");

    let received = rx
        .recv_timeout(Duration::from_millis(MESSAGE_RECEIVE_TIMEOUT_MS))
        .expect("the subscribe helper should deliver the published update");
    assert_eq!(received.uri, "file:///repo/src/lib.rs");
    assert_eq!(received.diagnostics.len(), 1);
    assert_eq!(received.diagnostics[0].message, "boom");

    // Dropping the leader tears down the proxy; the helper's subscriber then
    // disconnects and its loop ends, so the thread joins.
    drop(leader);
    // The helper loops on a 500ms recv timeout; give it room to observe the
    // disconnect. Detach if it does not join promptly (process teardown is
    // covered by the assertion above).
    let _ = sub_thread;
}

#[test]
fn subscribe_helper_loop_exits_when_cancel_flag_is_set() {
    // The follower→leader promotion bug (^343hrm0): a follower's subscriber loop
    // must stop on a cooperative cancel signal, because an ipc disconnect
    // surfaces as EAGAIN (not "disconnected"), so the loop would otherwise run
    // for the process lifetime even after promotion. With the leader's proxy
    // still alive (no disconnect to lean on), setting the cancel flag must end
    // the loop within ~1 recv interval.
    let dir = tempfile::tempdir().expect("tempdir");
    let config = ElectionConfig::new()
        .with_prefix("diag-bus-cancel")
        .with_base_dir(dir.path());
    let workspace = dir.path().join("ws");
    let election: LeaderElection<DiagnosticsBusMessage> =
        LeaderElection::with_config(&workspace, config);
    let leader = match election.elect().expect("elect") {
        ElectionOutcome::Leader(guard) => guard,
        ElectionOutcome::Follower(_) => panic!("first election must be leader"),
    };
    let backend = leader.bus_addresses().backend.clone();

    let cancel = Arc::new(AtomicBool::new(false));
    let sub_cancel = Arc::clone(&cancel);
    let sub_thread = std::thread::spawn(move || {
        swissarmyhammer_diagnostics::subscribe_diagnostics_over_bus(&backend, &sub_cancel, |_| {})
    });

    // Let the subscriber attach and enter its recv loop.
    std::thread::sleep(Duration::from_millis(SUB_PROPAGATION_THREAD_MS));

    // Signal cancellation. The proxy is still alive (the leader is held), so the
    // ONLY thing that can stop the loop is the cooperative flag.
    cancel.store(true, Ordering::Relaxed);

    // The loop wakes every ≤500ms on recv_timeout and checks the flag. Joining
    // with a poll proves the loop returned (deterministic, not a fixed sleep):
    // it must return within ~1 interval, well under this generous bound.
    let deadline = std::time::Instant::now() + Duration::from_millis(2000);
    while !sub_thread.is_finished() {
        if std::time::Instant::now() >= deadline {
            panic!("subscriber loop did not exit within 2s of setting the cancel flag");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    sub_thread
        .join()
        .expect("subscriber thread joins")
        .expect("subscribe helper returns Ok after cooperative cancel");

    drop(leader);
}
