//! Direct registry test: a second registration with the SAME name but a
//! DIFFERENT source still fails with [`Error::ServerNameTaken`].
//!
//! The point of idempotent registration is to merge structurally-equal
//! registrations — never to silently shadow a live server with a different
//! implementation. This file pins that distinction at the registry level so a
//! genuine implementation-collision keeps producing the loud error the
//! platform's name-uniqueness policy promises, while
//! [`registry_idempotent`](super) covers the same-source merge path.

use std::sync::Arc;

use async_trait::async_trait;
use rmcp::model::Tool;
use serde_json::{Map, Value};
use swissarmyhammer_plugin::registry::ServerSource;
use swissarmyhammer_plugin::{
    CallerId, Error, McpServer, Result, ServerRegistry, ServerStatus, ToolMetadata,
};

/// A trivial in-process `McpServer` carrying a tag so a tools/list cross-check
/// can prove which server is live after the rejected second registration.
struct TaggedServer {
    tag: &'static str,
}

#[async_trait]
impl McpServer for TaggedServer {
    fn tools(&self) -> Vec<ToolMetadata> {
        vec![ToolMetadata::new(Tool::new(
            self.tag,
            "tagged-server's lone tool",
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
fn same_name_with_a_different_url_source_fails_with_server_name_taken() {
    let mut registry = ServerRegistry::new();

    let url_a = url_source("https://a.example.test/weather");
    let url_b = url_source("https://b.example.test/weather");

    let original: Arc<dyn McpServer> = Arc::new(TaggedServer { tag: "original" });
    registry
        .register("weather".to_string(), url_a.clone(), Arc::clone(&original))
        .expect("the first registration should succeed");

    let usurper: Arc<dyn McpServer> = Arc::new(TaggedServer { tag: "usurper" });
    let err = registry
        .register("weather".to_string(), url_b, Arc::clone(&usurper))
        .expect_err(
            "a different-source registration of an already-live name must fail with ServerNameTaken",
        );

    match err {
        Error::ServerNameTaken(name) => assert_eq!(name, "weather"),
        other => panic!("expected ServerNameTaken, got {other:?}"),
    }

    // The live server must still be the original, not the usurper.
    let live = registry
        .get("weather")
        .expect("the original server must remain live");
    let live_tag = live
        .tools()
        .into_iter()
        .next()
        .expect("the tagged server exposes one tool")
        .name()
        .to_string();
    assert_eq!(
        live_tag, "original",
        "a rejected different-source registration must not displace the live server"
    );

    // Sanity: resolving by name still reports Live, not some intermediate state.
    assert!(
        matches!(registry.resolve("weather"), ServerStatus::Live(_)),
        "the live server must stay reachable after the rejected collision"
    );
}
