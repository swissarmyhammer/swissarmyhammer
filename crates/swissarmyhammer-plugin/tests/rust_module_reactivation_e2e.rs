//! End-to-end regression test for **rust-module re-activation across an
//! unload/reload of the activating plugin**.
//!
//! # The bug this guards
//!
//! A host-exposed `{ rust }` module is *single-activation*: the host moves the
//! module out of its available-modules table the first time a plugin activates
//! it (`register(name, { rust: id })`). That is correct for two plugins racing
//! to activate the same id concurrently — only one may own it at a time.
//!
//! But it was ALSO permanently destructive across a sequential unload/reload of
//! the SAME plugin: unloading the activating plugin disposed its server
//! registration (tombstoning the registered name) but never returned the moved
//! module to the available-modules table. A second `load()` of the same bundle
//! — the exact thing the hot-reload watcher does on every plugin file change —
//! then re-ran `register(name, { rust: id })`, which resolved the now-empty
//! table slot to `UnknownServer`, the plugin's `load()` threw, the load was
//! rolled back, and the registered name stayed tombstoned. Any later call into
//! that name (e.g. a command callback reaching back into a `focus` / `views`
//! server) failed with `ServerUnavailable`.
//!
//! # What this test proves
//!
//! Exposing a `{ rust }` module ONCE, then loading → unloading → loading the
//! same activating bundle again, the second load succeeds — the module was
//! restored to the available-modules table when the first activation was
//! disposed. The registered name is live and callable after the reload.
//!
//! The test is deliberately self-contained: it stands up its own in-process
//! echo module and stages a minimal activating bundle, so it does not depend on
//! the heavier shared `support` harness.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use serde::{Deserialize, Serialize};
use serde_json::json;
use swissarmyhammer_plugin::{
    CallerId, InProcessServer, McpServer as PluginMcpServer, PluginHost, PLUGINS_SUBDIR,
};

/// A generous upper bound on any single host interaction (isolate spin-up).
const TIMEOUT: Duration = Duration::from_secs(60);

/// The available-modules id the test exposes the echo module under and that the
/// staged bundle activates with `{ rust: ECHO_MODULE_ID }`.
const ECHO_MODULE_ID: &str = "reactivation-echo-mod";

/// The registered server name the bundle claims for the activated module.
const SERVER_NAME: &str = "reactivation-echo";

/// The probe message the bundle echoes in `load()` and the test re-echoes after
/// the reload.
const PROBE_MESSAGE: &str = "reactivation echo is live";

/// Arguments for the in-process echo module's `echo` tool.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct EchoArgs {
    /// The payload echoed straight back to the caller.
    message: String,
}

/// A real `rmcp` server handler exposing a single flat `echo` tool.
#[derive(Clone)]
struct EchoServer {
    tool_router: ToolRouter<Self>,
}

#[tool_router(router = tool_router)]
impl EchoServer {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(name = "echo", description = "Echoes its message argument back.")]
    async fn echo(&self, Parameters(args): Parameters<EchoArgs>) -> String {
        args.message
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for EchoServer {}

/// Exposes a fresh in-process echo module under [`ECHO_MODULE_ID`].
async fn expose_echo_module(host: &PluginHost) {
    let server: Arc<dyn PluginMcpServer> = Arc::new(
        InProcessServer::new(EchoServer::new())
            .await
            .expect("wrapping the echo server should succeed"),
    );
    host.expose_rust_module(ECHO_MODULE_ID, server)
        .await
        .expect("exposing the echo module should succeed");
}

/// Stages a minimal activating bundle into `<layer_root>/plugins/<name>/`.
///
/// The bundle's `load()` activates the host-exposed `{ rust }` module under
/// [`SERVER_NAME`] and round-trips one `echo` call — the same shape the
/// committed `collide-probe-a` example uses, written inline so the test owns its
/// fixture.
fn stage_activating_bundle(layer_root: &Path, name: &str) -> PathBuf {
    let bundle = layer_root.join(PLUGINS_SUBDIR).join(name);
    std::fs::create_dir_all(&bundle).expect("bundle dir should be created");
    let index = format!(
        r#"import {{ Plugin }} from "@swissarmyhammer/plugin";

export default class ReactivationProbe extends Plugin {{
  readonly name = "Reactivation Probe";
  readonly version = "1.0.0";
  readonly description = "Activates a rust module under a name and echoes once.";

  async load(): Promise<void> {{
    this.register("{server}", {{ rust: "{module}" }});
    const probe = (
      this as unknown as Record<string, Record<string, (
        args: Record<string, unknown>,
      ) => Promise<unknown>>>
    )["{server}"];
    const result = await probe.echo({{ message: "{message}" }});
    const content = (result as {{ content?: Array<{{ text?: string }}> }}).content;
    const text = content && content[0] ? content[0].text : undefined;
    if (text !== "{message}") {{
      throw new Error(`echo round-trip returned '${{text}}'`);
    }}
  }}
}}
"#,
        server = SERVER_NAME,
        module = ECHO_MODULE_ID,
        message = PROBE_MESSAGE,
    );
    std::fs::write(bundle.join("index.ts"), index).expect("index.ts should be written");
    bundle
}

#[tokio::test]
async fn rust_module_reactivates_after_unload_reload_of_activating_plugin() {
    let work_dir = tempfile::TempDir::new().expect("work dir temp");
    let project_root = tempfile::TempDir::new().expect("project plugin root temp");

    let bundle_path = stage_activating_bundle(project_root.path(), "reactivation-probe");

    let host = PluginHost::for_tests(
        work_dir.path().to_path_buf(),
        Some(project_root.path().to_path_buf()),
    );

    // Expose the `{ rust }` module ONCE — exactly as production wiring does it
    // (a single `expose_rust_module` before any plugin discovery, never
    // re-exposed per reload).
    expose_echo_module(&host).await;

    // First load: the bundle activates `{ rust: ECHO_MODULE_ID }` under
    // SERVER_NAME and echoes a probe message in `load()`.
    let first_id = tokio::time::timeout(TIMEOUT, host.load(&bundle_path))
        .await
        .expect("first load should not hang")
        .expect("first load should succeed — the module is freshly exposed");

    // Unload the activating plugin. This disposes its server registration; the
    // fix returns the moved `{ rust }` module to the available-modules table so
    // a later re-activation can find it.
    tokio::time::timeout(TIMEOUT, host.unload(&first_id))
        .await
        .expect("unload should not hang")
        .expect("unload should succeed");

    // Second load WITHOUT re-exposing the module — this is the hot-reload path.
    // Before the fix this failed: the module was gone from the table, so the
    // bundle's `register(name, { rust: id })` resolved to `UnknownServer`,
    // `load()` threw, and the load was rolled back.
    let second_id = tokio::time::timeout(TIMEOUT, host.load(&bundle_path))
        .await
        .expect("second load should not hang")
        .expect(
            "second load should succeed — disposing the first activation must \
             restore the `{ rust }` module so it can be re-activated",
        );

    // The registered name is live and callable after the reload.
    let echo = tokio::time::timeout(
        TIMEOUT,
        host.call(
            CallerId::HostInternal,
            SERVER_NAME,
            "echo",
            json!({ "message": PROBE_MESSAGE }),
        ),
    )
    .await
    .expect("post-reload echo dispatch should not hang")
    .expect("the re-activated module must answer through its registered name");

    let text = echo
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|c| c.first())
        .and_then(|entry| entry.get("text"))
        .and_then(|text| text.as_str())
        .expect("an `echo` result must carry text content");
    assert_eq!(
        text, PROBE_MESSAGE,
        "the re-activated module must echo its argument verbatim"
    );

    tokio::time::timeout(TIMEOUT, host.unload(&second_id))
        .await
        .expect("final unload should not hang")
        .expect("final unload should succeed");
}
