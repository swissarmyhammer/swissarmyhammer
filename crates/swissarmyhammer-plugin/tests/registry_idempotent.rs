//! Direct registry test: `ServerRegistry::register` is idempotent for the same
//! `(name, source)` pair.
//!
//! Two plugins that both depend on the same external MCP server (a community
//! `weather` server, the host's `kanban` module, etc.) should both be able to
//! call `register(name, source)` without one of them blowing up on
//! `ServerNameTaken`. The platform must merge structurally-equal registrations:
//!
//! - The first `register` of a fresh name creates the registration with
//!   refcount=1.
//! - A second `register` of the same name with a STRUCTURALLY EQUAL source
//!   succeeds: the live server is kept (no second connect), the refcount is
//!   bumped, and the second `Arc<dyn McpServer>` the caller passed is dropped
//!   in favor of the already-registered one.
//! - A second `register` of the same name with a DIFFERENT source still fails
//!   with [`Error::ServerNameTaken`] — the platform never silently shadows a
//!   live server with a different implementation.
//!
//! Refcounting threads through unregister too: the first `unregister` for a
//! shared name decrements the refcount but leaves the server live; only the
//! last `unregister` actually tears the server down and tombstones the name.

use std::sync::Arc;

use async_trait::async_trait;
use rmcp::model::Tool;
use serde_json::{Map, Value};
use swissarmyhammer_plugin::registry::{RegisterOutcome, ServerSource, UnregisterOutcome};
use swissarmyhammer_plugin::{
    CallerId, Error, McpServer, Result, ServerRegistry, ServerStatus, ToolMetadata,
};

/// A trivial in-process `McpServer` used to verify the registry without a real
/// transport. Each instance carries an `id` so tests can prove that the
/// already-registered server — not a second instance built by a second
/// `register` call — is the one kept under a shared name.
struct LabelServer {
    id: &'static str,
}

#[async_trait]
impl McpServer for LabelServer {
    fn tools(&self) -> Vec<ToolMetadata> {
        vec![ToolMetadata::new(Tool::new(
            self.id,
            "label-server's lone tool",
            Map::new(),
        ))]
    }

    async fn invoke(&self, _caller: CallerId, _tool: &str, input: Value) -> Result<Value> {
        Ok(input)
    }
}

fn url_source(url: &str) -> ServerSource {
    ServerSource::from_json(&serde_json::json!({ "url": url }))
        .expect("a valid `{ url }` JSON should parse")
}

#[test]
fn registering_the_same_name_and_source_twice_shares_one_server() {
    let mut registry = ServerRegistry::new();
    let source = url_source("https://example.test/weather");

    let first: Arc<dyn McpServer> = Arc::new(LabelServer { id: "first" });
    let outcome = registry
        .register("weather".to_string(), source.clone(), Arc::clone(&first))
        .expect("first registration of a fresh name should succeed");
    assert!(
        matches!(outcome, RegisterOutcome::Registered),
        "the first registration is a fresh insert, got {outcome:?}"
    );

    // A second registration with the SAME source succeeds. The new `Arc` is
    // dropped — the live registration is the one from the first call. The
    // outcome distinguishes "share" from "fresh insert" so the host knows it
    // should not emit a duplicate types-emitter event for the second call.
    let second: Arc<dyn McpServer> = Arc::new(LabelServer { id: "second" });
    let outcome = registry
        .register("weather".to_string(), source.clone(), Arc::clone(&second))
        .expect("a duplicate same-source registration must not error");
    assert!(
        matches!(outcome, RegisterOutcome::AlreadyRegistered),
        "a same-source second registration must report AlreadyRegistered, got {outcome:?}"
    );

    // The live server is the FIRST one. The `Arc` the registry hands back must
    // point at `first`, not `second`, so a tool call routed through the
    // registry reaches the originally-registered handler.
    let live = registry
        .get("weather")
        .expect("the shared name must resolve to a live server");
    let live_id = live
        .tools()
        .into_iter()
        .next()
        .expect("the label server exposes one tool")
        .name()
        .to_string();
    assert_eq!(
        live_id, "first",
        "the same-source dedup must keep the already-registered server, not the duplicate"
    );
}

#[test]
fn unregister_decrements_until_the_last_caller_then_tears_down() {
    let mut registry = ServerRegistry::new();
    let source = url_source("https://example.test/weather");

    let server: Arc<dyn McpServer> = Arc::new(LabelServer { id: "weather" });
    registry
        .register("weather".to_string(), source.clone(), Arc::clone(&server))
        .expect("first registration of a fresh name should succeed");
    registry
        .register("weather".to_string(), source.clone(), Arc::clone(&server))
        .expect("a duplicate same-source registration must succeed");

    // First unregister: a holder is gone but another still has the server
    // registered. The live server stays callable; the name is not tombstoned.
    let outcome = registry.unregister("weather");
    assert!(
        matches!(outcome, UnregisterOutcome::Decremented),
        "the first unregister of a shared name should only decrement the refcount, got {outcome:?}"
    );
    assert!(
        matches!(registry.resolve("weather"), ServerStatus::Live(_)),
        "a shared server must stay live until every caller has unregistered"
    );

    // Second (last) unregister: refcount hits zero, the server is torn down,
    // and the name is tombstoned for future Resolve calls.
    let outcome = registry.unregister("weather");
    let teardown = match outcome {
        UnregisterOutcome::Removed(server) => server,
        other => panic!("the last unregister must report Removed, got {other:?}"),
    };
    assert!(
        Arc::ptr_eq(&teardown, &server),
        "the removed server arc must point at the one that was registered"
    );
    assert!(
        matches!(registry.resolve("weather"), ServerStatus::Disposed),
        "after the last unregister the name should resolve as Disposed"
    );
}

#[test]
fn registering_with_a_different_source_under_a_taken_name_errors() {
    let mut registry = ServerRegistry::new();

    let weather_v1 = url_source("https://v1.example.test/weather");
    let server: Arc<dyn McpServer> = Arc::new(LabelServer { id: "weather" });
    registry
        .register("weather".to_string(), weather_v1, Arc::clone(&server))
        .expect("the first registration should succeed");

    let weather_v2 = url_source("https://v2.example.test/weather");
    let err = registry
        .register("weather".to_string(), weather_v2, Arc::clone(&server))
        .expect_err(
            "a different-source registration under a live name must fail with ServerNameTaken",
        );
    match err {
        Error::ServerNameTaken(name) => assert_eq!(name, "weather"),
        other => panic!("expected ServerNameTaken, got {other:?}"),
    }

    // The original registration must still be the one that resolves.
    assert!(
        matches!(registry.resolve("weather"), ServerStatus::Live(_)),
        "a rejected different-source registration must not disturb the live server"
    );
}

#[test]
fn unregister_of_a_never_registered_name_reports_not_registered() {
    let mut registry = ServerRegistry::new();
    let outcome = registry.unregister("ghost");
    assert!(
        matches!(outcome, UnregisterOutcome::NotRegistered),
        "unregister of a name the registry never saw should report NotRegistered, got {outcome:?}"
    );
    assert!(
        matches!(registry.resolve("ghost"), ServerStatus::Unknown),
        "a never-registered name must keep resolving as Unknown after a no-op unregister"
    );
}
