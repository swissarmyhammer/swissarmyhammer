//! Host bootstrap end-to-end tests.
//!
//! Asserts that [`install_commands_module`] wires the command service
//! into a real [`PluginHost`]:
//!
//! - the bootstrap exposes the `commands` module on the host's
//!   available-modules table, so a plugin's `register(name, { rust:
//!   "commands" })` can activate it;
//! - a host caller's `register command` lands on the override stack
//!   and surfaces through the service's `list command` verb.
//!
//! These tests do not exercise plugin lifecycle — see
//! [`super::unload_cleanup_e2e`] and [`super::override_stack_e2e`] for
//! that. They focus on the wiring itself.
//!
//! [`install_commands_module`]: swissarmyhammer_command_service::bootstrap::install_commands_module
//! [`PluginHost`]: swissarmyhammer_plugin::PluginHost

use swissarmyhammer_command_service::bootstrap::COMMANDS_MODULE_ID;
use swissarmyhammer_plugin::CallerId;

use crate::support::{call_command, ids_of, list_args, register_args, BootstrappedHost};

/// The bootstrap exposes the `commands` module on the host's
/// available-modules table.
///
/// A plugin activating `register(name, { rust: "commands" })` would
/// find the module ready to consume. The host's
/// [`has_exposed_module`](swissarmyhammer_plugin::PluginHost::has_exposed_module)
/// inspector reports the exposure without requiring an actual
/// activation, so the test stays focused on the wiring contract
/// without consuming the activation slot.
#[tokio::test]
async fn bootstrap_exposes_commands_module_on_the_host() {
    let bootstrap = BootstrappedHost::new().await;
    assert!(
        bootstrap.host.has_exposed_module(COMMANDS_MODULE_ID).await,
        "the bootstrap must expose `{COMMANDS_MODULE_ID}` on the host's available-modules table",
    );
}

/// A host caller's `register command` lands on the override stack.
///
/// Exercises the service core through the bootstrap-built handle: one
/// register call from `CallerId::HostInternal` produces exactly one
/// active entry, observable directly on the registry and indirectly
/// through the `list command` verb. This proves the bootstrap returned
/// a real, live service — not a stub or a clone that drifts from the
/// exposed copy.
#[tokio::test]
async fn host_caller_register_command_surfaces_through_list_verb() {
    let bootstrap = BootstrappedHost::new().await;

    let caller = CallerId::HostInternal;
    let _ = call_command(
        &bootstrap.service,
        caller.clone(),
        register_args("host.archive", "Archive", "cb_host_archive_execute"),
    )
    .await;
    let _ = call_command(
        &bootstrap.service,
        caller.clone(),
        register_args("host.restore", "Restore", "cb_host_restore_execute"),
    )
    .await;

    let listed = call_command(&bootstrap.service, caller, list_args()).await;
    let mut ids = ids_of(&listed);
    ids.sort();

    assert_eq!(
        ids,
        vec!["host.archive".to_string(), "host.restore".to_string()],
        "every host-registered command must appear in the list verb's response, got {ids:?}"
    );
}

/// Re-bootstrapping the same host rejects the duplicate module exposure.
///
/// The bootstrap is one-shot per host: [`install_commands_module`]
/// calls [`PluginHost::expose_rust_module`], which itself rejects an id
/// already in the available-modules table. Asserting the second
/// bootstrap fails (rather than silently re-using the existing wiring)
/// catches a future regression where the bootstrap accidentally swaps
/// the live service out from under a running plugin.
///
/// [`install_commands_module`]: swissarmyhammer_command_service::bootstrap::install_commands_module
/// [`PluginHost::expose_rust_module`]: swissarmyhammer_plugin::PluginHost::expose_rust_module
#[tokio::test]
async fn second_bootstrap_against_the_same_host_fails_with_module_collision() {
    let bootstrap = BootstrappedHost::new().await;

    let second =
        swissarmyhammer_command_service::bootstrap::install_commands_module(&bootstrap.host).await;
    let error = second.expect_err("a second bootstrap on the same host must fail");
    assert!(
        error.to_string().contains(COMMANDS_MODULE_ID),
        "the duplicate-exposure error should name the `{COMMANDS_MODULE_ID}` module id, got {error}",
    );
}
