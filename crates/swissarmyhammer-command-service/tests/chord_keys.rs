//! Chord-key schema tests for `register command` (Card J).
//!
//! The `keys` schema admits multi-key chords: a per-keymap binding value is a
//! sequence of one or more canonical keystrokes separated by single spaces
//! (e.g. vim `"g g"`, `"g Shift+T"`). A single keystroke (no space) is a
//! chord of length 1 — exactly the pre-chord behavior. The canonical
//! keystroke grammar never contains a literal space (the spacebar is the
//! symbolic `"Space"` token), so the separator is unambiguous.
//!
//! These tests pin the two halves of the schema:
//!
//! 1. **Round-trip** — a chord binding registers and surfaces verbatim
//!    through `list command` (registration → `CommandMetadata`).
//! 2. **Validation** — malformed chord strings (empty value, empty step from
//!    leading/trailing/doubled separators, non-space whitespace) are rejected
//!    at registration time with a structured `InvalidKeyBinding` error, so a
//!    bad binding can never reach the webview keymap.

mod common;

use common::{call_tool, register_payload};
use serde_json::{json, Value};
use swissarmyhammer_command_service::CommandService;
use swissarmyhammer_plugin::{CallerId, PluginId};

/// Build a `register command` payload carrying the given `keys` map.
fn register_payload_with_keys(id: &str, keys: Value) -> Value {
    let mut payload = register_payload(id, "Chord Test", "cb_exec");
    payload["keys"] = keys;
    payload
}

#[tokio::test]
async fn chord_keys_round_trip_through_list_command() {
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-chord"));

    let payload = register_payload_with_keys(
        "nav.first",
        json!({ "vim": "g g", "cua": "Home", "emacs": "Alt+<" }),
    );
    call_tool(&service, "register command", payload, &caller)
        .await
        .expect("chord registration should succeed");

    let result = call_tool(
        &service,
        "list command",
        json!({ "op": "list command" }),
        &caller,
    )
    .await
    .expect("list should succeed");
    let structured = result
        .structured_content
        .as_ref()
        .expect("list response should carry structured content");
    let commands = structured["commands"].as_array().expect("commands array");
    let cmd = commands
        .iter()
        .find(|c| c["id"] == json!("nav.first"))
        .expect("registered command should be listed");
    assert_eq!(
        cmd["keys"],
        json!({ "vim": "g g", "cua": "Home", "emacs": "Alt+<" }),
        "chord binding must round-trip verbatim through the metadata projection",
    );
}

#[tokio::test]
async fn multi_step_chord_with_modifier_step_round_trips() {
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-chord"));

    let payload = register_payload_with_keys(
        "perspective.prev",
        json!({ "cua": "Mod+[", "vim": "g Shift+T" }),
    );
    let result = call_tool(&service, "register command", payload, &caller)
        .await
        .expect("modifier-step chord should register");
    let structured = result
        .structured_content
        .as_ref()
        .expect("register response should carry structured content");
    assert_eq!(
        structured["active"]["keys"],
        json!({ "cua": "Mod+[", "vim": "g Shift+T" }),
    );
}

#[tokio::test]
async fn single_key_bindings_still_register() {
    // Backward compatibility: a chord of length 1 is today's single-key
    // binding and must keep registering unchanged.
    let service = CommandService::new();
    let caller = CallerId::HostInternal;

    let payload = register_payload_with_keys(
        "app.undo",
        json!({ "vim": "u", "cua": "Mod+z", "emacs": "Mod+z" }),
    );
    let result = call_tool(&service, "register command", payload, &caller)
        .await
        .expect("single-key bindings must remain valid");
    let structured = result.structured_content.as_ref().expect("structured");
    assert_eq!(structured["ok"], json!(true));
}

#[tokio::test]
async fn empty_binding_is_rejected() {
    let service = CommandService::new();
    let caller = CallerId::HostInternal;
    let payload = register_payload_with_keys("bad.keys", json!({ "vim": "" }));
    let err = call_tool(&service, "register command", payload, &caller)
        .await
        .expect_err("empty binding must be rejected");
    let data = err.data.expect("error must carry structured data");
    assert_eq!(data["kind"], json!("InvalidKeyBinding"));
    assert_eq!(data["id"], json!("bad.keys"));
    assert_eq!(data["keymap"], json!("vim"));
    assert_eq!(data["binding"], json!(""));
}

#[tokio::test]
async fn doubled_separator_is_rejected() {
    let service = CommandService::new();
    let caller = CallerId::HostInternal;
    let payload = register_payload_with_keys("bad.keys", json!({ "vim": "g  g" }));
    let err = call_tool(&service, "register command", payload, &caller)
        .await
        .expect_err("doubled separator (empty chord step) must be rejected");
    let data = err.data.expect("error must carry structured data");
    assert_eq!(data["kind"], json!("InvalidKeyBinding"));
    assert_eq!(data["binding"], json!("g  g"));
}

#[tokio::test]
async fn leading_and_trailing_separators_are_rejected() {
    for binding in [" g", "g ", " "] {
        let service = CommandService::new();
        let caller = CallerId::HostInternal;
        let payload = register_payload_with_keys("bad.keys", json!({ "vim": binding }));
        let err = call_tool(&service, "register command", payload, &caller)
            .await
            .unwrap_err();
        let data = err.data.expect("error must carry structured data");
        assert_eq!(
            data["kind"],
            json!("InvalidKeyBinding"),
            "binding {binding:?} must be rejected as InvalidKeyBinding",
        );
    }
}

#[tokio::test]
async fn non_space_whitespace_inside_a_step_is_rejected() {
    let service = CommandService::new();
    let caller = CallerId::HostInternal;
    let payload = register_payload_with_keys("bad.keys", json!({ "vim": "g\tg" }));
    let err = call_tool(&service, "register command", payload, &caller)
        .await
        .expect_err("tab inside a binding must be rejected");
    let data = err.data.expect("error must carry structured data");
    assert_eq!(data["kind"], json!("InvalidKeyBinding"));
}

#[tokio::test]
async fn rejected_registration_does_not_touch_the_registry() {
    let service = CommandService::new();
    let caller = CallerId::HostInternal;
    let payload = register_payload_with_keys("bad.keys", json!({ "vim": "d  d" }));
    call_tool(&service, "register command", payload, &caller)
        .await
        .expect_err("malformed chord must be rejected");
    service.with_registry(|reg| assert!(reg.is_empty()));
}
