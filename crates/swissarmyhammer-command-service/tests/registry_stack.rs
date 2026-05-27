//! Stack-semantics tests for [`CommandRegistry`].
//!
//! These tests pin the override-stack contract documented in
//! `ideas/plugins/command-service.md`:
//!
//! - The most recent registration for an id is "active" (top of stack).
//! - When a caller is unloaded (`purge_caller`) or explicitly removes its
//!   entry (`pop_caller`), the next-most-recent registration for that id
//!   re-emerges as active.
//! - `list(filter)` returns only top-of-stack entries — overridden entries
//!   are hidden from discovery surfaces.
//!
//! The canonical scenario from the design doc — host registers
//! `core.archive`; plugin A overrides; plugin B overrides; B unloads → A
//! active; A unloads → host active — is exercised in
//! [`override_stack_three_levels_fallback`].

use swissarmyhammer_command_service::{CommandRegistry, RegisterCommand};
use swissarmyhammer_plugin::{CallerId, PluginId};

/// Build a minimal [`RegisterCommand`] payload that distinguishes itself by
/// `name`, so a test can assert which registration is active by name alone.
fn registration(id: &str, name: &str) -> RegisterCommand {
    RegisterCommand {
        id: id.to_string(),
        name: name.to_string(),
        execute: swissarmyhammer_command_service::CallbackMarker::new("cb_test"),
        ..Default::default()
    }
}

#[test]
fn override_stack_three_levels_fallback() {
    let mut registry = CommandRegistry::new();

    let host = CallerId::HostInternal;
    let plugin_a = CallerId::Plugin(PluginId::new("plugin-a"));
    let plugin_b = CallerId::Plugin(PluginId::new("plugin-b"));

    registry.push(host.clone(), registration("core.archive", "host"));
    registry.push(plugin_a.clone(), registration("core.archive", "plugin-a"));
    registry.push(plugin_b.clone(), registration("core.archive", "plugin-b"));

    // Top of stack is plugin B.
    let active = registry
        .active("core.archive")
        .expect("plugin-b registration should be active");
    assert_eq!(active.registration.name, "plugin-b");
    assert_eq!(active.caller, plugin_b);

    // B unloads — A re-emerges.
    registry.purge_caller(&plugin_b);
    let active = registry
        .active("core.archive")
        .expect("plugin-a registration should re-emerge");
    assert_eq!(active.registration.name, "plugin-a");
    assert_eq!(active.caller, plugin_a);

    // A unloads — host re-emerges.
    registry.purge_caller(&plugin_a);
    let active = registry
        .active("core.archive")
        .expect("host registration should re-emerge");
    assert_eq!(active.registration.name, "host");
    assert_eq!(active.caller, host);

    // Host unloads — no active registration.
    registry.purge_caller(&host);
    assert!(
        registry.active("core.archive").is_none(),
        "no caller should remain after the host purge"
    );
}

#[test]
fn pop_caller_removes_only_that_callers_entry() {
    let mut registry = CommandRegistry::new();

    let host = CallerId::HostInternal;
    let plugin_a = CallerId::Plugin(PluginId::new("plugin-a"));
    let plugin_b = CallerId::Plugin(PluginId::new("plugin-b"));

    registry.push(host.clone(), registration("core.archive", "host"));
    registry.push(plugin_a.clone(), registration("core.archive", "plugin-a"));
    registry.push(plugin_b.clone(), registration("core.archive", "plugin-b"));

    // Pop the middle caller — A — out from underneath B.
    let removed = registry.pop_caller(&plugin_a, "core.archive");
    assert!(removed, "plugin-a had an entry to pop");

    // B is still on top.
    let active = registry
        .active("core.archive")
        .expect("plugin-b should remain active after popping the middle caller");
    assert_eq!(active.registration.name, "plugin-b");

    // B pops — host re-emerges (not A, because A was already removed).
    registry.pop_caller(&plugin_b, "core.archive");
    let active = registry
        .active("core.archive")
        .expect("host should re-emerge after B pops");
    assert_eq!(active.registration.name, "host");
}

#[test]
fn pop_caller_with_no_entry_is_a_noop() {
    let mut registry = CommandRegistry::new();
    let plugin_a = CallerId::Plugin(PluginId::new("plugin-a"));

    let removed = registry.pop_caller(&plugin_a, "never.registered");
    assert!(!removed, "popping a non-existent entry returns false");
}

#[test]
fn purge_caller_removes_all_entries_for_one_caller() {
    let mut registry = CommandRegistry::new();
    let plugin_a = CallerId::Plugin(PluginId::new("plugin-a"));

    registry.push(plugin_a.clone(), registration("cmd.one", "a-one"));
    registry.push(plugin_a.clone(), registration("cmd.two", "a-two"));
    registry.push(plugin_a.clone(), registration("cmd.three", "a-three"));

    assert!(registry.active("cmd.one").is_some());
    assert!(registry.active("cmd.two").is_some());
    assert!(registry.active("cmd.three").is_some());

    registry.purge_caller(&plugin_a);

    assert!(registry.active("cmd.one").is_none());
    assert!(registry.active("cmd.two").is_none());
    assert!(registry.active("cmd.three").is_none());
}

#[test]
fn list_returns_only_top_of_stack_entries() {
    let mut registry = CommandRegistry::new();
    let host = CallerId::HostInternal;
    let plugin_a = CallerId::Plugin(PluginId::new("plugin-a"));

    registry.push(host.clone(), registration("cmd.shared", "host-shared"));
    registry.push(host.clone(), registration("cmd.host-only", "host-only"));
    registry.push(plugin_a.clone(), registration("cmd.shared", "a-shared"));
    registry.push(plugin_a.clone(), registration("cmd.a-only", "a-only"));

    let listed = registry.list();
    let names: Vec<&str> = listed
        .iter()
        .map(|e| e.registration.name.as_str())
        .collect();

    // 3 distinct ids — `cmd.shared` shows only the active (a-shared) entry.
    assert_eq!(listed.len(), 3, "list should return one entry per id");
    assert!(names.contains(&"host-only"));
    assert!(names.contains(&"a-only"));
    assert!(names.contains(&"a-shared"));
    assert!(
        !names.contains(&"host-shared"),
        "the overridden host entry should not appear in list"
    );
}
