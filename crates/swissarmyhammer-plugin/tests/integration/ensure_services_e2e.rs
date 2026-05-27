//! End-to-end test for the `ensureServices` SDK convention helper.
//!
//! Two committed example bundles — `ensure-services-a` and `ensure-services-b`
//! — each call `ensureServices(this, ["commands"])` followed by
//! `registerCommands(this, [...])` in their `load()`. This test loads both
//! bundles against one host with the command service bootstrapped, then drives
//! the lifecycle through unload to prove the platform's idempotent
//! registration policy is wired end to end:
//!
//! 1. **Both loads succeed.** Each plugin's `ensureServices` call attempts to
//!    `register("commands", { rust: "commands" })`. The first registers; the
//!    second is a structural-equality no-op that joins the live registration
//!    via refcount.
//! 2. **Both plugins' commands land on the shared registry.** The host's
//!    `list command` call surfaces every plugin's commands together, proving
//!    the two `registerCommands` calls both reached the live `commands`
//!    server.
//! 3. **Unloading one plugin purges only its commands.** The platform's
//!    per-plugin ledger drives the command service's lifecycle hook on
//!    unload, removing exactly that plugin's entries.
//! 4. **The `commands` server stays live for the remaining plugin.** The
//!    second plugin's commands are still reachable; the second plugin can
//!    still call into `commands` (the `list command` call on the remaining
//!    state succeeds), proving the refcount left the registration up.

use crate::support::{list_command_ids, list_via_host, stage_example, BootstrappedHost, TIMEOUT};
use swissarmyhammer_plugin::Error;

/// The committed example bundle name for the first plugin in the pair.
const BUNDLE_A: &str = "ensure-services-a";

/// The committed example bundle name for the second plugin in the pair.
const BUNDLE_B: &str = "ensure-services-b";

/// The command id `ensure-services-a` registers — must match the bundle's
/// `index.ts` literal.
const COMMAND_A: &str = "ensure-services-a.greet";

/// The command id `ensure-services-b` registers — must match the bundle's
/// `index.ts` literal.
const COMMAND_B: &str = "ensure-services-b.farewell";

/// Two plugins both call `ensureServices(this, ["commands"])` in `load()`;
/// the platform merges the registrations and each plugin's commands land
/// independently. Unloading one plugin purges only its commands; the other
/// plugin's commands and the shared `commands` server stay live.
#[tokio::test]
async fn two_plugins_share_one_commands_server_via_ensure_services() {
    let bootstrap = BootstrappedHost::new().await;

    let bundle_a = stage_example(BUNDLE_A, bootstrap.project_root());
    let bundle_b = stage_example(BUNDLE_B, bootstrap.project_root());

    // ── Step 1 ─────────────────────────────────────────────────────────────
    // Both bundles' `load()` should succeed. Each runs `ensureServices(this,
    // ["commands"])` followed by `registerCommands`. The first registration
    // of `commands` claims the name; the second is a structural-equality
    // no-op that joins the live registration via refcount.
    let plugin_a_id = tokio::time::timeout(TIMEOUT, bootstrap.host.load(&bundle_a))
        .await
        .expect("loading bundle A should not hang")
        .expect("bundle A's load should succeed — first ensureServices claims `commands`");

    let plugin_b_id = tokio::time::timeout(TIMEOUT, bootstrap.host.load(&bundle_b))
        .await
        .expect("loading bundle B should not hang")
        .expect(
            "bundle B's load should succeed — second ensureServices is an \
             idempotent no-op, not a ServerNameTaken collision",
        );

    // ── Step 2 ─────────────────────────────────────────────────────────────
    // The command service's registry holds every caller's commands. After
    // both loads, the two plugins' command ids must both appear — proof
    // that the shared `commands` server is live and that each plugin's
    // `registerCommands` reached it.
    let after_both = list_command_ids(&bootstrap.service);
    assert!(
        after_both.contains(&COMMAND_A.to_string()),
        "after loading both bundles, the registry must hold '{COMMAND_A}', got {after_both:?}"
    );
    assert!(
        after_both.contains(&COMMAND_B.to_string()),
        "after loading both bundles, the registry must hold '{COMMAND_B}', got {after_both:?}"
    );

    // Sanity check the production path: the same listing is reachable via
    // the host's `commands` server while both plugins still hold the
    // registration. Bundle B unloading later will tombstone the
    // registration; this proves the activated route is live in the
    // shared-holder state.
    let via_host = list_via_host(&bootstrap.host)
        .await
        .expect("with both plugins holding `commands`, the host route must answer");
    assert!(
        serde_json::to_string(&via_host)
            .unwrap_or_default()
            .contains(COMMAND_A),
        "the host-route listing must surface '{COMMAND_A}', got {via_host}"
    );

    // ── Step 3 ─────────────────────────────────────────────────────────────
    // Unloading bundle A purges its commands via the per-plugin ledger. The
    // command service's lifecycle hook fires for the unloaded plugin and
    // removes every entry attributed to it. Bundle B's commands stay, AND
    // the `commands` server stays live because bundle B still holds the
    // refcounted registration.
    tokio::time::timeout(TIMEOUT, bootstrap.host.unload(&plugin_a_id))
        .await
        .expect("unloading bundle A should not hang")
        .expect("unloading bundle A should succeed");

    let after_a_unload = list_command_ids(&bootstrap.service);
    assert!(
        !after_a_unload.contains(&COMMAND_A.to_string()),
        "after unloading bundle A, '{COMMAND_A}' must be purged, got {after_a_unload:?}"
    );
    assert!(
        after_a_unload.contains(&COMMAND_B.to_string()),
        "after unloading bundle A, '{COMMAND_B}' must remain, got {after_a_unload:?}"
    );

    // The `commands` server is still reachable through the host route —
    // bundle B is the surviving holder. This is the headline refcount
    // proof: the registration outlived the first holder's unload.
    list_via_host(&bootstrap.host)
        .await
        .expect("after only bundle A unloads, the `commands` server must stay live for bundle B");

    // ── Step 4 ─────────────────────────────────────────────────────────────
    // Unloading bundle B (the last holder of the shared `commands`
    // registration) drops the refcount to zero. The command service's
    // lifecycle hook purges bundle B's commands; the registry is now empty.
    // The `commands` server itself is tombstoned in the host's registry —
    // a `list_via_host` call afterward would surface `ServerUnavailable`,
    // and that distinction is exactly what the refcount design exists for.
    tokio::time::timeout(TIMEOUT, bootstrap.host.unload(&plugin_b_id))
        .await
        .expect("unloading bundle B should not hang")
        .expect("unloading bundle B should succeed");

    let after_b_unload = list_command_ids(&bootstrap.service);
    assert!(
        !after_b_unload.contains(&COMMAND_A.to_string()),
        "after unloading both bundles, '{COMMAND_A}' must remain purged, got {after_b_unload:?}"
    );
    assert!(
        !after_b_unload.contains(&COMMAND_B.to_string()),
        "after unloading both bundles, '{COMMAND_B}' must also be purged, got {after_b_unload:?}"
    );

    // The refcount-to-zero teardown of the shared `commands` registration is
    // observable on the production path: with no plugin holding the
    // registration, the host's `commands` server is tombstoned and a call
    // through it fails with `ServerUnavailable`. The empty service-side
    // registry only tells us no commands are *registered* — it does not on
    // its own prove the server slot is gone. This assertion pins the
    // headline refcount contract end to end.
    let after_b_via_host = list_via_host(&bootstrap.host).await;
    assert!(
        matches!(after_b_via_host, Err(Error::ServerUnavailable)),
        "after the last holder unloads, the host's `commands` server must \
         tombstone — expected Err(ServerUnavailable), got {after_b_via_host:?}"
    );
}
