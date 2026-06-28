//! Plugin-unload auto-cleanup end-to-end tests.
//!
//! Asserts that the bootstrap's ledger-driven unload hook drives the
//! command service's [`CommandService::purge_caller`] when a plugin is
//! unloaded:
//!
//! - a loaded plugin's [`CallerId::Plugin`] entries are visible while
//!   the plugin is loaded;
//! - on unload, those entries vanish — purged by the unload hook the
//!   service installed during register;
//! - no stale ledger or registry state remains (the host's
//!   [`PluginLedger::len`] drops the plugin entirely).
//!
//! Each test loads a no-op probe plugin so the host runs its full
//! lifecycle path (isolate created, evaluated, `load()` invoked) and
//! the bootstrap's lifecycle hook gets a real ledger entry to attach
//! to. The probe plugin itself does not call `register command` — the
//! SDK does not yet expose the callback-marshalling path required —
//! so the test drives the service from the Rust side, passing the
//! plugin's [`PluginId`] as the caller. This exercises the exact
//! `(register → unload-hook → purge)` chain a future plugin-side
//! register would walk.
//!
//! [`CommandService::purge_caller`]: swissarmyhammer_command_service::CommandService::purge_caller
//! [`CallerId::Plugin`]: swissarmyhammer_plugin::CallerId::Plugin
//! [`PluginLedger::len`]: swissarmyhammer_plugin::PluginLedger::len
//! [`PluginId`]: swissarmyhammer_plugin::PluginId

use swissarmyhammer_plugin::CallerId;

use crate::support::{
    call_command, ids_of, list_args, register_args, write_noop_probe_plugin, BootstrappedHost,
};

/// Unloading a plugin auto-purges every command it registered.
///
/// The headline behavior: the bootstrap's lifecycle hook attached a
/// purge closure to the plugin's per-plugin ledger on every register;
/// the host's unload path drains the ledger and runs the closure, so
/// the plugin's registrations vanish without the plugin's cooperation.
///
/// This is the contract a user-facing palette depends on: an unloaded
/// plugin's commands disappear from the picker the next time it opens,
/// no manual `unregister` required.
#[tokio::test]
async fn unload_purges_every_command_the_plugin_registered() {
    let bootstrap = BootstrappedHost::new().await;
    let plugins_dir = bootstrap._user_root.path().join("plugins");
    std::fs::create_dir_all(&plugins_dir).expect("plugins dir");
    let plugin_dir = write_noop_probe_plugin(&plugins_dir, "probe-unload");

    let plugin_id = bootstrap
        .host
        .load(&plugin_dir)
        .await
        .expect("the no-op probe plugin must load cleanly");

    // Drive register from outside the isolate, attributing to the plugin.
    // Two distinct ids so the purge covers more than one stack.
    let caller = CallerId::Plugin(plugin_id.clone());
    let _ = call_command(
        &bootstrap.service,
        caller.clone(),
        register_args("probe.foo", "Probe Foo", "cb_probe_foo_execute"),
    )
    .await;
    let _ = call_command(
        &bootstrap.service,
        caller.clone(),
        register_args("probe.bar", "Probe Bar", "cb_probe_bar_execute"),
    )
    .await;

    // While the plugin is loaded the registrations are visible.
    let listed_before = call_command(&bootstrap.service, caller.clone(), list_args()).await;
    let mut ids_before = ids_of(&listed_before);
    ids_before.sort();
    assert_eq!(
        ids_before,
        vec!["probe.bar".to_string(), "probe.foo".to_string()],
        "both registrations should be active while the plugin is loaded, got {ids_before:?}"
    );

    bootstrap
        .host
        .unload(&plugin_id)
        .await
        .expect("unload must succeed");

    // After unload the bootstrap's lifecycle hook has purged the
    // plugin's entries: a fresh `list command` returns empty.
    let listed_after = call_command(&bootstrap.service, CallerId::HostInternal, list_args()).await;
    let ids_after = ids_of(&listed_after);
    assert!(
        ids_after.is_empty(),
        "plugin unload must auto-purge every registration the plugin made, got {ids_after:?}"
    );

    // No zombie ledger state for the unloaded plugin: `len` reports
    // `None` because the plugin is no longer tracked at all.
    assert_eq!(
        bootstrap.host.ledger_len(&plugin_id).await,
        None,
        "the unloaded plugin should no longer appear in the per-plugin ledger",
    );

    // The service-level registry is also fully drained.
    let registry_empty = bootstrap
        .service
        .with_registry(|registry| registry.is_empty());
    assert!(
        registry_empty,
        "the command-service registry must be empty after the only registered caller unloads",
    );
}

/// Purging a caller with no entries is a safe no-op.
///
/// The lifecycle hook is appended once per successful register, so a
/// plugin that registers two commands installs two purge hooks. Each
/// runs on unload, calling [`CommandService::purge_caller`] in
/// succession — the second one finds nothing to purge. The contract is
/// that this duplicate purge does not panic and does not emit a
/// spurious `commands/changed` (the change-detection compares total
/// entries before/after).
///
/// [`CommandService::purge_caller`]: swissarmyhammer_command_service::CommandService::purge_caller
#[tokio::test]
async fn duplicate_purge_after_unload_is_a_safe_noop() {
    let bootstrap = BootstrappedHost::new().await;
    let caller = CallerId::Plugin(swissarmyhammer_plugin::PluginId::new("noop-plugin"));

    // No registrations: `purge_caller` is a pure no-op.
    bootstrap.service.purge_caller(&caller);
    bootstrap.service.purge_caller(&caller);

    let listed = call_command(&bootstrap.service, CallerId::HostInternal, list_args()).await;
    let ids = ids_of(&listed);
    assert!(
        ids.is_empty(),
        "duplicate purges of a caller with no entries must leave the registry empty, got {ids:?}",
    );
}
