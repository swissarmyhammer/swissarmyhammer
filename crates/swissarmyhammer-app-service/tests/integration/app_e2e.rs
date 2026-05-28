//! End-to-end tests for the `app` MCP server's shell verbs.
//!
//! Builds an `AppService` over a recording `SpyShell` and exercises every verb
//! the `_meta` tree advertises: `quit app`, `show about`, `show help`. Each
//! test drives the verb through the real `ServerHandler` / `call_tool` path
//! and asserts both the structured response and the recorded shell call.

use serde_json::json;

use super::common::{call_tool, Harness, SpyShell};
use swissarmyhammer_app_service::AboutInfo;

/// `quit app` routes through the shell's `quit` — the same exit-with-0
/// behavior the original `quit_app` Tauri command performed.
#[tokio::test]
async fn quit_routes_through_shell_quit() {
    let h = Harness::new();
    let service = h.service();

    let res = call_tool(&service, "quit app", json!({ "op": "quit app" }))
        .await
        .expect("quit app should succeed");

    assert_eq!(res["ok"], json!(true));
    assert_eq!(
        h.shell.calls(),
        vec!["quit"],
        "quit app must fire exactly one shell.quit()"
    );
}

/// `show about` returns the shell's app name / version and records the call.
#[tokio::test]
async fn about_returns_app_metadata() {
    let h = Harness::with_shell(SpyShell::new(
        AboutInfo {
            name: "MyApp".to_string(),
            version: "1.2.3".to_string(),
        },
        "https://help.example/docs",
    ));
    let service = h.service();

    let res = call_tool(&service, "show about", json!({ "op": "show about" }))
        .await
        .expect("show about should succeed");

    assert_eq!(res["ok"], json!(true));
    assert_eq!(res["name"], json!("MyApp"));
    assert_eq!(res["version"], json!("1.2.3"));
    assert_eq!(h.shell.calls(), vec!["about"]);
}

/// `show help` returns the shell's help target and records the call.
#[tokio::test]
async fn help_returns_help_target() {
    let h = Harness::with_shell(SpyShell::new(
        AboutInfo {
            name: "MyApp".to_string(),
            version: "1.2.3".to_string(),
        },
        "https://help.example/docs",
    ));
    let service = h.service();

    let res = call_tool(&service, "show help", json!({ "op": "show help" }))
        .await
        .expect("show help should succeed");

    assert_eq!(res["ok"], json!(true));
    assert_eq!(res["target"], json!("https://help.example/docs"));
    assert_eq!(h.shell.calls(), vec!["help"]);
}

/// An unknown op surfaces a structured `invalid_params` error and fires no
/// shell call.
#[tokio::test]
async fn unknown_op_errors_without_side_effect() {
    let h = Harness::new();
    let service = h.service();

    let err = call_tool(&service, "frobnicate app", json!({ "op": "frobnicate app" }))
        .await
        .expect_err("unknown op should error");

    assert!(
        err.message.contains("frobnicate app"),
        "error should name the unknown op: {}",
        err.message
    );
    assert!(
        h.shell.calls().is_empty(),
        "an unknown op must not drive any shell action"
    );
}

/// Calling the service with the wrong tool name is rejected.
#[tokio::test]
async fn wrong_tool_name_is_rejected() {
    use rmcp::model::CallToolRequestParams;
    use rmcp::ServerHandler;
    use std::borrow::Cow;

    use super::common::request_context;

    let h = Harness::new();
    let service = h.service();

    let request = CallToolRequestParams::new(Cow::Borrowed("not-app"));
    let err = service
        .call_tool(request, request_context())
        .await
        .expect_err("wrong tool name should error");

    assert!(err.message.contains("not-app"), "{}", err.message);
}
