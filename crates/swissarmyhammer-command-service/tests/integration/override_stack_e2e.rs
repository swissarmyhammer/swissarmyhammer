//! Override-stack re-emergence end-to-end tests.
//!
//! Exercises the headline scenario from `ideas/plugins/command-service.md`:
//! the **active** registration for one command id is the top of an
//! override stack keyed by registering caller, and unloading a caller
//! re-emerges the next-most-recent registration — all the way down to
//! the host's original registration at the bottom of the stack.
//!
//! Concretely:
//!
//! 1. The host registers `core.archive` as `CallerId::HostInternal` — the
//!    floor of the stack, never purged because the host never unloads.
//! 2. Plugin A loads and registers `core.archive` — A's registration
//!    becomes active, the host's original is shadowed.
//! 3. Plugin B loads and registers `core.archive` — B becomes active,
//!    A is shadowed, the host's is still at the bottom.
//! 4. B unloads — the bootstrap's lifecycle hook purges B's entry, A's
//!    re-emerges as active.
//! 5. A unloads — A's entry purged, the host's original is active again.
//!
//! The headline registry-invariant tests
//! ([`override_stack_re_emerges_through_host_a_b_then_unwind`] and
//! [`reloading_an_unloaded_plugin_uses_a_fresh_caller_slot`]) use no-op JS
//! bundles whose `load()` does nothing. The Rust test drives the register /
//! list path against the bootstrap-wired service from outside the isolate,
//! attributing each call to the correct [`CallerId::Plugin`]. This exercises
//! the full bootstrap wiring — the in-process `commands` server, the
//! `HostCallerLifecycle` ledger-hook installation, and the ledger drain on
//! unload — through pure registry-state assertions (`active_caller` +
//! `stack_depth`), which remain the most direct test for the registry
//! invariant.
//!
//! [`override_stack_round_trips_execute_callbacks_through_sdk_isolates`]
//! complements those by exercising the same host → A → B → unwind sequence
//! through the full SDK callback round trip: each probe plugin's `load()`
//! uses `ensureServices` + `registerCommands` to register `core.archive`
//! with an `execute` callback that returns a per-plugin sentinel string.
//! After each stack mutation the test invokes `execute command` against
//! the bootstrap-wired service and asserts the returned sentinel matches
//! the expected top-of-stack caller's plugin — proving the dispatcher
//! routes into the active plugin's isolate, the SDK's `$callback` marshalling
//! reaches the function the plugin actually registered, and the per-plugin
//! ledger drains the callback table on unload so the next-most-recent
//! plugin's callback re-emerges intact.
//!
//! [`CallerId::Plugin`]: swissarmyhammer_plugin::CallerId::Plugin

use std::sync::Arc;

use swissarmyhammer_command_service::CommandService;
use swissarmyhammer_plugin::{CallerId, PluginHost, PluginId};

use crate::support::{
    call_command, execute_args, execute_result, register_args, try_call_command,
    write_noop_probe_plugin, write_sentinel_probe_plugin, BootstrappedHost,
};

/// The command id every layer in this test registers under.
const COMMAND_ID: &str = "core.archive";

/// Snapshot the caller of the active entry for `COMMAND_ID`.
///
/// Returns `None` when the stack is empty. The caller identity is what
/// the override-stack semantics turn on: every layer in the test
/// registers under one id with a distinct caller, so the active
/// caller is the unambiguous signal for "which registration is on top".
fn active_caller(service: &Arc<CommandService>) -> Option<CallerId> {
    service.with_registry(|registry| {
        registry
            .active(COMMAND_ID)
            .map(|entry| entry.caller.clone())
    })
}

/// Read the per-id stack depth for `COMMAND_ID`.
///
/// Used to assert that each register pushed exactly one new entry, and
/// each unload purged exactly one. A bug in the lifecycle hook
/// (forgetting to install, or installing twice) would show up as a
/// depth mismatch even when the active caller still matches.
fn stack_depth(service: &Arc<CommandService>) -> usize {
    service.with_registry(|registry| registry.stack_for(COMMAND_ID).len())
}

/// Register `COMMAND_ID` from `caller` against the bootstrap-built
/// service, with a per-caller callback id so each layer is recognisably
/// distinct in the stack.
async fn register_from(service: &Arc<CommandService>, caller: CallerId, callback_id: &str) {
    let _ = call_command(
        service,
        caller,
        register_args(COMMAND_ID, "Archive", callback_id),
    )
    .await;
}

/// Load a no-op probe plugin into `host`'s user-layer plugins
/// directory under `id`, returning the host-minted [`PluginId`].
async fn load_probe(host: &PluginHost, plugins_dir: &std::path::Path, id: &str) -> PluginId {
    let plugin_dir = write_noop_probe_plugin(plugins_dir, id);
    host.load(&plugin_dir)
        .await
        .unwrap_or_else(|error| panic!("probe plugin '{id}' must load cleanly: {error}"))
}

/// The headline scenario: host → A → B, then B unloads → A unloads.
///
/// At each step the test asserts both the active caller AND the stack
/// depth, so a future regression that leaves a stale entry behind (or
/// fails to push one) is caught immediately.
#[tokio::test]
async fn override_stack_re_emerges_through_host_a_b_then_unwind() {
    let bootstrap = BootstrappedHost::new().await;
    let plugins_dir = bootstrap._user_root.path().join("plugins");
    std::fs::create_dir_all(&plugins_dir).expect("plugins dir");

    // (1) Host registers first — the floor of the stack. The host
    //     never unloads, so this entry stays for the whole test and
    //     re-emerges at the end after every plugin layer has unloaded.
    register_from(
        &bootstrap.service,
        CallerId::HostInternal,
        "cb_host_archive",
    )
    .await;
    assert_eq!(
        active_caller(&bootstrap.service),
        Some(CallerId::HostInternal),
        "after only the host has registered, the host's entry must be active",
    );
    assert_eq!(stack_depth(&bootstrap.service), 1);

    // (2) Plugin A loads and registers — A becomes active, shadowing
    //     the host's original.
    let plugin_a = load_probe(&bootstrap.host, &plugins_dir, "override-probe-a").await;
    let caller_a = CallerId::Plugin(plugin_a.clone());
    register_from(&bootstrap.service, caller_a.clone(), "cb_a_archive").await;
    assert_eq!(
        active_caller(&bootstrap.service),
        Some(caller_a.clone()),
        "after plugin A registers, A's entry must be active (shadows the host's)",
    );
    assert_eq!(stack_depth(&bootstrap.service), 2);

    // (3) Plugin B loads and registers — B becomes active, A is
    //     shadowed, the host's stays at the bottom.
    let plugin_b = load_probe(&bootstrap.host, &plugins_dir, "override-probe-b").await;
    let caller_b = CallerId::Plugin(plugin_b.clone());
    register_from(&bootstrap.service, caller_b.clone(), "cb_b_archive").await;
    assert_eq!(
        active_caller(&bootstrap.service),
        Some(caller_b.clone()),
        "after plugin B registers, B's entry must be active (shadows A)",
    );
    assert_eq!(stack_depth(&bootstrap.service), 3);

    // (4) Plugin B unloads — the bootstrap's lifecycle hook purges B
    //     from the registry, A's registration re-emerges as active.
    bootstrap
        .host
        .unload(&plugin_b)
        .await
        .expect("plugin B should unload cleanly");
    assert_eq!(
        active_caller(&bootstrap.service),
        Some(caller_a.clone()),
        "after plugin B unloads, A's registration must re-emerge as active",
    );
    assert_eq!(
        stack_depth(&bootstrap.service),
        2,
        "B's entry should have been purged by the unload hook",
    );

    // (5) Plugin A unloads — A's entry purged, the host's original is
    //     active again. This is the round-trip that proves the
    //     override stack and the auto-cleanup compose correctly.
    bootstrap
        .host
        .unload(&plugin_a)
        .await
        .expect("plugin A should unload cleanly");
    assert_eq!(
        active_caller(&bootstrap.service),
        Some(CallerId::HostInternal),
        "after plugin A unloads, the host's original registration must re-emerge as active",
    );
    assert_eq!(
        stack_depth(&bootstrap.service),
        1,
        "only the host's original entry should remain on the stack",
    );
}

/// A re-load of plugin A after it had unloaded gets a fresh
/// [`PluginId`] — and therefore a fresh stack slot — so its new
/// registration is treated as a new override of the host's entry, not
/// a resurrection of the old one.
///
/// This protects against a subtle bug where a per-id "is this caller
/// still here" cache or a stale ledger entry could let A's *old*
/// registration leak back onto the active stack after an unload-load
/// cycle.
#[tokio::test]
async fn reloading_an_unloaded_plugin_uses_a_fresh_caller_slot() {
    let bootstrap = BootstrappedHost::new().await;
    let plugins_dir = bootstrap._user_root.path().join("plugins");
    std::fs::create_dir_all(&plugins_dir).expect("plugins dir");

    register_from(
        &bootstrap.service,
        CallerId::HostInternal,
        "cb_host_archive",
    )
    .await;

    // Load → register → unload — one full cycle.
    let first_load = load_probe(&bootstrap.host, &plugins_dir, "override-probe-reload").await;
    register_from(
        &bootstrap.service,
        CallerId::Plugin(first_load.clone()),
        "cb_first_load",
    )
    .await;
    bootstrap
        .host
        .unload(&first_load)
        .await
        .expect("first unload");
    assert_eq!(
        active_caller(&bootstrap.service),
        Some(CallerId::HostInternal),
        "after the first plugin instance unloads, the host's entry must be active",
    );

    // Re-load the same bundle. The host mints a *fresh* PluginId —
    // unrelated to `first_load` — so any stale state keyed on the old
    // id would not propagate.
    let second_load = load_probe(&bootstrap.host, &plugins_dir, "override-probe-reload").await;
    assert_ne!(
        first_load, second_load,
        "the host must mint a fresh PluginId on the second load",
    );

    register_from(
        &bootstrap.service,
        CallerId::Plugin(second_load.clone()),
        "cb_second_load",
    )
    .await;
    assert_eq!(
        active_caller(&bootstrap.service),
        Some(CallerId::Plugin(second_load.clone())),
        "the freshly-loaded plugin's registration must be active",
    );
    assert_eq!(
        stack_depth(&bootstrap.service),
        2,
        "the stack should be host + second-load only — the first load's entry was purged",
    );
}

/// Sentinel string each layer's `execute` callback returns. Picked so the
/// expected-vs-actual diff in a failing assertion immediately names the
/// offending layer.
const SENTINEL_A: &str = "sentinel-from-plugin-a";

/// Sentinel string for plugin B's `execute` callback.
const SENTINEL_B: &str = "sentinel-from-plugin-b";

/// Load a sentinel-probe plugin: the bundle's `load()` uses the SDK's
/// `ensureServices` + `registerCommands` helpers to register [`COMMAND_ID`]
/// with an `execute` callback that returns `sentinel` verbatim.
///
/// Returns the host-minted [`PluginId`] so the test can attribute later
/// active-caller assertions to the same plugin id and unload by handle.
async fn load_sentinel_probe(
    host: &PluginHost,
    plugins_dir: &std::path::Path,
    id: &str,
    sentinel: &str,
) -> PluginId {
    let plugin_dir = write_sentinel_probe_plugin(plugins_dir, id, COMMAND_ID, sentinel);
    host.load(&plugin_dir)
        .await
        .unwrap_or_else(|error| panic!("sentinel probe '{id}' must load cleanly: {error}"))
}

/// Invoke `execute command` for [`COMMAND_ID`] and return the dispatcher's
/// result value (the active stack entry's `execute` callback's return value).
///
/// Asserts the response was a success and pulls `structuredContent.result`
/// out for direct comparison against the expected sentinel.
async fn execute_top_of_stack(service: &Arc<CommandService>) -> serde_json::Value {
    let response = call_command(service, CallerId::HostInternal, execute_args(COMMAND_ID)).await;
    execute_result(&response)
}

/// Assert that `execute command` for [`COMMAND_ID`] fails with
/// `CallbackFailed`, which is what happens when the active stack entry's
/// caller has no isolate the dispatcher can route into (the host floor's
/// `CallerId::HostInternal` is the canonical case — see
/// [`crate::bootstrap::HostCallbackDispatcher::invoke`]).
async fn assert_execute_fails_with_no_isolate(service: &Arc<CommandService>) {
    let err = try_call_command(service, CallerId::HostInternal, execute_args(COMMAND_ID))
        .await
        .expect_err(
            "executing the host-floor registration should fail — the bootstrap \
             dispatcher rejects non-plugin callers because they have no isolate \
             to invoke",
        );
    let data = err
        .data
        .expect("CallbackFailed must carry structured `data`");
    assert_eq!(
        data["kind"], "CallbackFailed",
        "the dispatcher's no-isolate rejection must surface as CallbackFailed, \
         got data={data}"
    );
}

/// Round-trip companion to
/// [`override_stack_re_emerges_through_host_a_b_then_unwind`]: drives the
/// same host → A → B → unload B → unload A → host sequence and, at every
/// step, invokes `execute command` to prove the active entry's `execute`
/// callback actually runs end-to-end through the SDK dispatch path.
///
/// At each plugin-on-top step the test asserts the dispatcher returned that
/// plugin's sentinel string verbatim — proving:
///
/// - the [`crate::bootstrap::HostCallbackDispatcher`] routed the invocation
///   into the right plugin's isolate;
/// - the SDK's `$callback` marshalling preserved the function the plugin
///   actually registered (each call reaches a callback id that was minted
///   by `registerCommands` from inside that plugin's isolate);
/// - the registry's "active = top of stack" rule survives a round trip
///   through the callback table.
///
/// At each host-on-top step (start and end) the test asserts execute fails
/// with `CallbackFailed` — the dispatcher rejects non-plugin callers
/// because they have no isolate to reach back into. This is the same
/// no-isolate contract the host-only Rust unit tests pin separately; the
/// override-stack test sees it as the natural side-effect of the host
/// floor re-emerging after all plugins unload.
#[tokio::test]
async fn override_stack_round_trips_execute_callbacks_through_sdk_isolates() {
    let bootstrap = BootstrappedHost::new().await;
    let plugins_dir = bootstrap._user_root.path().join("plugins");
    std::fs::create_dir_all(&plugins_dir).expect("plugins dir");

    // (1) Host registers first — the floor of the stack. The host's
    //     callback id is opaque-marker only (no isolate behind it), so
    //     executing while the host is on top must fail with
    //     CallbackFailed.
    register_from(
        &bootstrap.service,
        CallerId::HostInternal,
        "cb_host_archive",
    )
    .await;
    assert_eq!(
        active_caller(&bootstrap.service),
        Some(CallerId::HostInternal),
        "after only the host has registered, the host's entry must be active",
    );
    assert_eq!(stack_depth(&bootstrap.service), 1);
    assert_execute_fails_with_no_isolate(&bootstrap.service).await;

    // (2) Plugin A loads via the SDK convention and registers — A's
    //     isolate-resident `execute` callback returns SENTINEL_A. The
    //     dispatcher must now reach into A's isolate when execute runs.
    let plugin_a = load_sentinel_probe(
        &bootstrap.host,
        &plugins_dir,
        "override-sentinel-a",
        SENTINEL_A,
    )
    .await;
    let caller_a = CallerId::Plugin(plugin_a.clone());
    assert_eq!(
        active_caller(&bootstrap.service),
        Some(caller_a.clone()),
        "after plugin A registers via the SDK, A's entry must be active",
    );
    assert_eq!(stack_depth(&bootstrap.service), 2);
    assert_eq!(
        execute_top_of_stack(&bootstrap.service).await,
        serde_json::json!(SENTINEL_A),
        "with A on top, execute must reach A's isolate and return SENTINEL_A",
    );

    // (3) Plugin B loads via the SDK convention and registers — B's
    //     callback returns SENTINEL_B and shadows A's.
    let plugin_b = load_sentinel_probe(
        &bootstrap.host,
        &plugins_dir,
        "override-sentinel-b",
        SENTINEL_B,
    )
    .await;
    let caller_b = CallerId::Plugin(plugin_b.clone());
    assert_eq!(
        active_caller(&bootstrap.service),
        Some(caller_b.clone()),
        "after plugin B registers via the SDK, B's entry must be active",
    );
    assert_eq!(stack_depth(&bootstrap.service), 3);
    assert_eq!(
        execute_top_of_stack(&bootstrap.service).await,
        serde_json::json!(SENTINEL_B),
        "with B on top, execute must reach B's isolate and return SENTINEL_B",
    );

    // (4) Plugin B unloads — the bootstrap's lifecycle hook purges B's
    //     entry AND the per-plugin ledger drains B's callback id. A's
    //     registration re-emerges as active; A's callback id is still
    //     resident in A's isolate, so execute must now return SENTINEL_A
    //     again.
    bootstrap
        .host
        .unload(&plugin_b)
        .await
        .expect("plugin B should unload cleanly");
    assert_eq!(
        active_caller(&bootstrap.service),
        Some(caller_a.clone()),
        "after plugin B unloads, A's registration must re-emerge as active",
    );
    assert_eq!(stack_depth(&bootstrap.service), 2);
    assert_eq!(
        execute_top_of_stack(&bootstrap.service).await,
        serde_json::json!(SENTINEL_A),
        "after B unloads, execute must reach A's isolate and return SENTINEL_A — \
         proving B's callback purge did not corrupt A's still-live callback table",
    );

    // (5) Plugin A unloads — A's entry purged, the host's floor is
    //     active again, and execute reverts to the no-isolate failure.
    bootstrap
        .host
        .unload(&plugin_a)
        .await
        .expect("plugin A should unload cleanly");
    assert_eq!(
        active_caller(&bootstrap.service),
        Some(CallerId::HostInternal),
        "after plugin A unloads, the host's original registration must re-emerge as active",
    );
    assert_eq!(stack_depth(&bootstrap.service), 1);
    assert_execute_fails_with_no_isolate(&bootstrap.service).await;
}
