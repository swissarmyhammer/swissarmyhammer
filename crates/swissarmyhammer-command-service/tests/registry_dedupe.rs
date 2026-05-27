//! Per-caller dedupe tests for [`CommandRegistry::push`].
//!
//! Pins the rule that re-registration by the same caller for the same id
//! replaces that caller's entry in place rather than producing a duplicate.
//! This keeps the stack bounded by `unique_callers × unique_ids` and ensures
//! a `pop_caller` for that caller removes exactly the right entry.

use swissarmyhammer_command_service::{CommandRegistry, RegisterCommand};
use swissarmyhammer_plugin::{CallerId, PluginId};

fn registration(id: &str, name: &str) -> RegisterCommand {
    RegisterCommand {
        id: id.to_string(),
        name: name.to_string(),
        execute: swissarmyhammer_command_service::CallbackMarker::new("cb_test"),
        ..Default::default()
    }
}

#[test]
fn same_caller_same_id_replaces_in_place() {
    let mut registry = CommandRegistry::new();
    let plugin_b = CallerId::Plugin(PluginId::new("plugin-b"));

    registry.push(plugin_b.clone(), registration("foo", "first"));
    registry.push(plugin_b.clone(), registration("foo", "second"));

    let entries = registry.stack_for("foo");
    assert_eq!(
        entries.len(),
        1,
        "same caller pushing twice should produce one entry, not two"
    );

    let active = registry.active("foo").expect("foo should be active");
    assert_eq!(
        active.registration.name, "second",
        "the latest registration's data should win"
    );
}

#[test]
fn replacement_moves_entry_to_top_of_stack() {
    let mut registry = CommandRegistry::new();
    let host = CallerId::HostInternal;
    let plugin_a = CallerId::Plugin(PluginId::new("plugin-a"));

    registry.push(host.clone(), registration("foo", "host"));
    registry.push(plugin_a.clone(), registration("foo", "plugin-a"));
    // Host re-registers — its entry should be deduped AND promoted to top.
    registry.push(host.clone(), registration("foo", "host-v2"));

    let entries = registry.stack_for("foo");
    assert_eq!(entries.len(), 2, "still two callers — no duplicates");

    let active = registry.active("foo").expect("foo should be active");
    assert_eq!(
        active.registration.name, "host-v2",
        "the re-registering caller's entry should be on top"
    );
    assert_eq!(active.caller, host);
}

#[test]
fn different_callers_same_id_produce_distinct_entries() {
    let mut registry = CommandRegistry::new();
    let host = CallerId::HostInternal;
    let plugin_a = CallerId::Plugin(PluginId::new("plugin-a"));

    registry.push(host.clone(), registration("foo", "host"));
    registry.push(plugin_a.clone(), registration("foo", "plugin-a"));

    assert_eq!(registry.stack_for("foo").len(), 2);
}
