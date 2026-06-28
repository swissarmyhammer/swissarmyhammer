//! Real-pipeline coverage for `notifications/commands/changed`.
//!
//! A registry change (register / unregister) must reach a bridge subscriber as a
//! single, declared `notifications/commands/changed` after the debounce window —
//! so the palette / availability cache refreshes and a plugin can react via
//! `this.commands.on("changed", …)`. This pins the WHOLE production path with no
//! hand-built notification:
//!
//!   register/unregister verb → service `ChangeNotifier::notify` (debounced) →
//!   the bootstrap-wired `BridgeNotifierSink` → `NotificationBridge::publish` →
//!   subscriber receives `notifications/commands/changed`.
//!
//! Nothing here constructs the notification directly: the service's notifier is
//! wired to the host's real bridge by [`install_commands_module`] (via
//! `BootstrappedHost`), so a missing publisher would leave the subscriber empty
//! and the test would FAIL.

use std::time::Duration;

use swissarmyhammer_plugin::{CallerId, McpNotification, NotificationSubscription};

use super::support::{call_command, register_args, BootstrappedHost};

/// The `commands/changed` event method, asserted verbatim so a renamed wire
/// method trips this test (the helper that builds it is pinned to the declared
/// `#[notification]` method by the in-crate coverage guard).
const COMMANDS_CHANGED: &str = "notifications/commands/changed";

/// Generous wait past the service's 100ms default debounce window. Long enough
/// to soak up worker scheduling jitter without making the suite slow.
const SETTLE: Duration = Duration::from_millis(400);

/// Drain every notification currently buffered on `sub` in arrival order.
fn drain(sub: &mut NotificationSubscription) -> Vec<McpNotification> {
    let mut out = Vec::new();
    while let Ok(note) = sub.try_recv() {
        out.push(note);
    }
    out
}

/// Count the `commands/changed` notifications in a drained batch.
fn changed_count(notes: &[McpNotification]) -> usize {
    notes
        .iter()
        .filter(|n| n.method == COMMANDS_CHANGED)
        .count()
}

#[tokio::test]
async fn registering_a_command_publishes_commands_changed() {
    let scaffold = BootstrappedHost::new().await;
    let mut client = scaffold.host.notification_bridge().subscribe();
    let caller = CallerId::External("agent-changed".to_string());

    // A registry change through the real verb path.
    call_command(
        &scaffold.service,
        caller,
        register_args("tag.alpha", "Alpha", "cb_alpha"),
    )
    .await;

    // Nothing should have landed yet — the notifier debounces.
    assert_eq!(
        changed_count(&drain(&mut client)),
        0,
        "commands/changed must not fire before the debounce window elapses",
    );

    tokio::time::sleep(SETTLE).await;

    let notes = drain(&mut client);
    assert_eq!(
        changed_count(&notes),
        1,
        "registering a command must publish exactly one commands/changed after \
         the debounce window; got {notes:?}",
    );
    // The delivered event is the thin epoch bump: provenance only, no per-command
    // payload (the subscriber refetches the registry itself).
    let changed = notes
        .iter()
        .find(|n| n.method == COMMANDS_CHANGED)
        .expect("a commands/changed was delivered");
    assert_eq!(changed.origin(), Some("user"));
}

#[tokio::test]
async fn rapid_registrations_coalesce_into_one_commands_changed() {
    let scaffold = BootstrappedHost::new().await;
    let mut client = scaffold.host.notification_bridge().subscribe();
    let caller = CallerId::External("agent-burst".to_string());

    // A burst of registrations inside one debounce window.
    for n in 0..5 {
        call_command(
            &scaffold.service,
            caller.clone(),
            register_args(&format!("tag.t{n}"), "T", &format!("cb_{n}")),
        )
        .await;
    }

    tokio::time::sleep(SETTLE).await;

    assert_eq!(
        changed_count(&drain(&mut client)),
        1,
        "five rapid registrations must coalesce into a single commands/changed",
    );
}

#[tokio::test]
async fn unregistering_a_command_publishes_commands_changed() {
    let scaffold = BootstrappedHost::new().await;
    let caller = CallerId::External("agent-unreg".to_string());

    // Register first and let that change drain.
    call_command(
        &scaffold.service,
        caller.clone(),
        register_args("tag.bravo", "Bravo", "cb_bravo"),
    )
    .await;
    tokio::time::sleep(SETTLE).await;

    // Subscribe AFTER the register so we observe only the unregister's event.
    let mut client = scaffold.host.notification_bridge().subscribe();

    call_command(
        &scaffold.service,
        caller,
        serde_json::json!({ "op": "unregister command", "id": "tag.bravo" }),
    )
    .await;
    tokio::time::sleep(SETTLE).await;

    assert_eq!(
        changed_count(&drain(&mut client)),
        1,
        "unregistering a command must publish exactly one commands/changed",
    );
}

#[tokio::test]
async fn a_noop_unregister_publishes_no_commands_changed() {
    let scaffold = BootstrappedHost::new().await;
    let mut client = scaffold.host.notification_bridge().subscribe();
    let caller = CallerId::External("agent-noop".to_string());

    // Unregister an id that was never registered — the registry does not change,
    // so the debounced notifier must stay silent (no spurious epoch bump).
    call_command(
        &scaffold.service,
        caller,
        serde_json::json!({ "op": "unregister command", "id": "tag.never" }),
    )
    .await;
    tokio::time::sleep(SETTLE).await;

    assert_eq!(
        changed_count(&drain(&mut client)),
        0,
        "a no-op unregister must not publish a spurious commands/changed",
    );
}
