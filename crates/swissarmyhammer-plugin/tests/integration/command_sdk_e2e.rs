//! End-to-end test for the convention vs. direct form of command registration.
//!
//! The committed `command-sdk-direct` example bundle registers two commands in
//! its `load()`:
//!
//!   * one through `registerCommands(this, [...])`, the SDK convention helper;
//!   * one through `this.commands.command.command.register(...)`, the SDK's
//!     path-form dispatch Proxy directly — the same call `registerCommands`
//!     makes under the hood.
//!
//! This test loads that bundle against a host with the command service
//! bootstrapped and asserts that:
//!
//! 1. Both forms reach the live `commands` server and leave entries on the
//!    registry — observable through the host's `list command` call.
//! 2. Unloading the plugin purges both commands via the per-plugin ledger,
//!    proving the cleanup path treats both forms uniformly.
//!
//! The two forms producing the same observable state is the headline
//! correctness property: `registerCommands` is a thin convention wrapper, not
//! a separate code path, so the direct dispatch must work identically.

use crate::support::{list_command_ids, stage_example, BootstrappedHost, TIMEOUT};

/// The committed example bundle name.
const BUNDLE: &str = "command-sdk-direct";

/// The command id registered through the convention helper — must match the
/// bundle's `index.ts` literal.
const CONVENTION_COMMAND_ID: &str = "command-sdk.convention";

/// The command id registered through the direct path-form dispatch — must
/// match the bundle's `index.ts` literal.
const DIRECT_COMMAND_ID: &str = "command-sdk.direct";

/// One plugin registers two commands — one through `registerCommands`, one
/// through `this.commands.command.command.register(...)` directly. Both land
/// on the registry; unloading the plugin purges both.
#[tokio::test]
async fn register_commands_helper_and_direct_form_produce_the_same_observable_state() {
    let bootstrap = BootstrappedHost::new().await;
    let bundle = stage_example(BUNDLE, bootstrap.project_root());

    // ── Step 1 ─────────────────────────────────────────────────────────────
    // Load the bundle. Its `load()` runs:
    //   ensureServices(this, ["commands"])
    //   await registerCommands(this, [{ id: CONVENTION, ... }])
    //   await this.commands.command.command.register({ id: DIRECT, ... })
    //
    // The SDK's marshalling step replaces each `execute` function value with
    // a `$callback` marker before dispatching, so the command service
    // receives the canonical wire shape for both forms.
    let plugin_id = tokio::time::timeout(TIMEOUT, bootstrap.host.load(&bundle))
        .await
        .expect("loading the command-sdk-direct bundle should not hang")
        .expect(
            "the bundle's load should succeed — both registration forms \
             dispatch through the same `commands` operation tool",
        );

    // ── Step 2 ─────────────────────────────────────────────────────────────
    // Both command ids must appear on the command service's registry — proof
    // that both forms reached the live `commands` server and landed on the
    // override stack. The listing is read from the service handle directly
    // so the assertion is independent of the host-route activation state
    // (the post-unload step below tombstones the activation).
    let after_load = list_command_ids(&bootstrap.service);
    assert!(
        after_load.contains(&CONVENTION_COMMAND_ID.to_string()),
        "after load, the registry must hold the convention-form command \
         '{CONVENTION_COMMAND_ID}', got {after_load:?}"
    );
    assert!(
        after_load.contains(&DIRECT_COMMAND_ID.to_string()),
        "after load, the registry must hold the direct-form command \
         '{DIRECT_COMMAND_ID}', got {after_load:?}"
    );

    // ── Step 3 ─────────────────────────────────────────────────────────────
    // Unload the plugin. The per-plugin ledger drains: the command service's
    // lifecycle hook fires and removes every command attributed to this
    // plugin, regardless of which SDK form registered it.
    tokio::time::timeout(TIMEOUT, bootstrap.host.unload(&plugin_id))
        .await
        .expect("unloading the command-sdk-direct bundle should not hang")
        .expect("unloading the bundle should succeed");

    let after_unload = list_command_ids(&bootstrap.service);
    assert!(
        !after_unload.contains(&CONVENTION_COMMAND_ID.to_string()),
        "after unload, the convention-form command '{CONVENTION_COMMAND_ID}' \
         must be purged, got {after_unload:?}"
    );
    assert!(
        !after_unload.contains(&DIRECT_COMMAND_ID.to_string()),
        "after unload, the direct-form command '{DIRECT_COMMAND_ID}' \
         must be purged, got {after_unload:?}"
    );
    assert!(
        after_unload.is_empty(),
        "after unload, the registry should hold no plugin-registered \
         commands at all, got {after_unload:?}"
    );
}
