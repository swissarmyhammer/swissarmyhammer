//! End-to-end integration tests for the **event-subscription** path, driven
//! through a real [`PluginHost`], a real V8 isolate, and the real
//! [`NotificationBridge`].
//!
//! This is the real-pipeline proof for the host half of the SDK event API
//! (kanban `plugin-arch`): a plugin subscribes to a notification method, the
//! test publishes a notification on the host's bridge, and the host's event
//! pump invokes the plugin's callback inside the isolate with the
//! notification's params. Nothing is mocked — the plugin registers a *real*
//! in-process `rmcp` server and the subscribed callback's effect is observed
//! through it, so a passing run proves every stage of the wire works:
//!
//! 1. the SDK's `this.__transport.subscribe(method, cb)` primitive marshals the
//!    callback and emits a `subscribe` envelope;
//! 2. the host records it in its event-subscription registry and lazily spawns
//!    the event pump;
//! 3. a `NotificationBridge::publish` reaches the pump, which invokes the stored
//!    callback via the host→isolate callback path with the notification params;
//! 4. the callback runs inside the isolate and reaches a real registered server.
//!
//! The companion `events.rs` unit tests cover the registry's pure logic
//! (subscribe/unsubscribe/remove-plugin/dedupe); these tests cover the live
//! delivery, the unsubscribe teardown, and the unload purge.
//!
//! Every cross-thread interaction is bounded by a timeout so a wedged isolate
//! fails the test fast instead of hanging CI.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use serde::{Deserialize, Serialize};
use serde_json::json;
use swissarmyhammer_command_service::bootstrap::install_commands_module;
use swissarmyhammer_plugin::{CallerId, InProcessServer, McpNotification, McpServer, PluginHost};

/// A generous upper bound on any single host interaction.
const TIMEOUT: Duration = Duration::from_secs(20);

/// How long a negative test waits for a (forbidden) delivery before concluding
/// none arrived. Long enough that a real delivery — which lands in
/// milliseconds — would have shown up.
const GRACE: Duration = Duration::from_millis(500);

/// The notification method the probe plugin subscribes to.
const METHOD: &str = "notifications/commands/executed";

/// Arguments for the recorder server's `record` tool.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct RecordArgs {
    /// The payload the subscribed callback hands the recorder — the stringified
    /// notification params.
    message: String,
}

/// A real `rmcp` server whose single `record` tool appends each call's
/// `message` to a shared vector the test can inspect.
///
/// This is the observable sink: the probe plugin's subscribed callback calls
/// `record` with the notification params, so the test asserts delivery by
/// reading `seen` rather than by mocking the callback path.
#[derive(Clone)]
struct RecorderServer {
    /// The macro-generated tool router for this handler.
    tool_router: ToolRouter<Self>,
    /// Every `message` the `record` tool has received, in order.
    seen: Arc<Mutex<Vec<String>>>,
}

#[tool_router(router = tool_router)]
impl RecorderServer {
    /// Builds a recorder that appends into `seen`.
    fn new(seen: Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            tool_router: Self::tool_router(),
            seen,
        }
    }

    /// Records the `message` argument and echoes it back.
    #[tool(name = "record", description = "Records its message argument.")]
    async fn record(&self, Parameters(args): Parameters<RecordArgs>) -> String {
        self.seen
            .lock()
            .expect("recorder mutex")
            .push(args.message.clone());
        args.message
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for RecorderServer {}

/// Builds a host, exposes the recorder module under `rec-mod`, and returns the
/// host plus the shared recording sink.
async fn host_with_recorder() -> (PluginHost, tempfile::TempDir, Arc<Mutex<Vec<String>>>) {
    let bundle = tempfile::TempDir::new().expect("temp dir");
    let host = PluginHost::for_tests(bundle.path().to_path_buf(), None);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let recorder: Arc<dyn McpServer> = Arc::new(
        InProcessServer::new(RecorderServer::new(Arc::clone(&seen)))
            .await
            .expect("wrapping the recorder rmcp handler should succeed"),
    );
    tokio::time::timeout(TIMEOUT, host.expose_rust_module("rec-mod", recorder))
        .await
        .expect("expose_rust_module should not hang")
        .expect("exposing the recorder module should succeed");
    (host, bundle, seen)
}

/// Writes a one-file plugin bundle whose default-class `load()` runs `body`.
fn write_plugin(dir: &std::path::Path, body: &str) {
    let entry = format!(
        "import {{ Plugin }} from '@swissarmyhammer/plugin';\n\
         export default class P extends Plugin {{\n\
           async load(): Promise<void> {{\n{body}\n}}\n\
         }}\n"
    );
    std::fs::write(dir.join("index.ts"), entry).expect("index.ts should be written");
}

/// Polls `seen` until it holds at least `want` entries, or fails on timeout.
async fn wait_for_recordings(seen: &Arc<Mutex<Vec<String>>>, want: usize) -> Vec<String> {
    let deadline = tokio::time::Instant::now() + TIMEOUT;
    loop {
        {
            let got = seen.lock().expect("recorder mutex");
            if got.len() >= want {
                return got.clone();
            }
        }
        if tokio::time::Instant::now() >= deadline {
            panic!(
                "timed out waiting for {want} recording(s); got {:?}",
                seen.lock().expect("recorder mutex")
            );
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// A plugin subscribes to a method; a notification published on the host's
/// bridge is delivered to the plugin's callback inside the isolate with the
/// notification's params.
#[tokio::test]
async fn published_notification_reaches_a_subscribed_plugin_callback() {
    let (host, bundle, seen) = host_with_recorder().await;

    // The plugin registers the recorder, then subscribes a callback that hands
    // each notification's params to the recorder as a JSON string.
    write_plugin(
        bundle.path(),
        "this.register('rec', { rust: 'rec-mod' });\n\
         const rec = this.rec;\n\
         this.__transport.subscribe('notifications/commands/executed', async (params) => {\n\
           await rec.record({ message: JSON.stringify(params) });\n\
         });",
    );

    tokio::time::timeout(TIMEOUT, host.load(bundle.path()))
        .await
        .expect("loading the plugin should not hang")
        .expect("the plugin's load should succeed");

    // Publish on the host's bridge — exactly what a production fan-in / the
    // command engine does. The pump (subscribed synchronously during load)
    // catches it and invokes the plugin's callback.
    let reached = host.notification_bridge().publish(McpNotification::new(
        METHOD,
        json!({ "id": "task.move", "marker": "hello-events" }),
    ));
    assert!(
        reached >= 1,
        "the event pump should be a live subscriber on the bridge, got {reached} subscribers"
    );

    let recordings = wait_for_recordings(&seen, 1).await;
    assert_eq!(recordings.len(), 1, "exactly one delivery expected");
    let delivered: serde_json::Value =
        serde_json::from_str(&recordings[0]).expect("the callback recorded the params as JSON");
    assert_eq!(
        delivered,
        json!({ "id": "task.move", "marker": "hello-events" }),
        "the callback must receive the published notification's params verbatim"
    );
}

/// After the plugin unsubscribes, a published notification is not delivered.
#[tokio::test]
async fn unsubscribe_stops_delivery() {
    let (host, bundle, seen) = host_with_recorder().await;

    // Subscribe and immediately unsubscribe using the id the primitive returns.
    write_plugin(
        bundle.path(),
        "this.register('rec', { rust: 'rec-mod' });\n\
         const rec = this.rec;\n\
         const id = this.__transport.subscribe('notifications/commands/executed', async (params) => {\n\
           await rec.record({ message: 'SHOULD NOT BE DELIVERED' });\n\
         });\n\
         this.__transport.unsubscribe('notifications/commands/executed', id);",
    );

    tokio::time::timeout(TIMEOUT, host.load(bundle.path()))
        .await
        .expect("loading the plugin should not hang")
        .expect("the plugin's load should succeed");

    host.notification_bridge()
        .publish(McpNotification::new(METHOD, json!({ "id": "task.move" })));

    // Give any (erroneous) delivery time to land, then assert none did.
    tokio::time::sleep(GRACE).await;
    assert!(
        seen.lock().expect("recorder mutex").is_empty(),
        "an unsubscribed callback must not be delivered to, got {:?}",
        seen.lock().expect("recorder mutex")
    );
}

/// After the plugin is unloaded, its subscription is purged: a published
/// notification is not delivered and the publish does not panic.
#[tokio::test]
async fn unload_purges_subscriptions() {
    let (host, bundle, seen) = host_with_recorder().await;

    write_plugin(
        bundle.path(),
        "this.register('rec', { rust: 'rec-mod' });\n\
         const rec = this.rec;\n\
         this.__transport.subscribe('notifications/commands/executed', async (params) => {\n\
           await rec.record({ message: 'SHOULD NOT BE DELIVERED AFTER UNLOAD' });\n\
         });",
    );

    let plugin_id = tokio::time::timeout(TIMEOUT, host.load(bundle.path()))
        .await
        .expect("loading the plugin should not hang")
        .expect("the plugin's load should succeed");

    tokio::time::timeout(TIMEOUT, host.unload(&plugin_id))
        .await
        .expect("unloading the plugin should not hang")
        .expect("unloading the plugin should succeed");

    // Publishing after unload reaches the pump but resolves to no targets — the
    // plugin's entry was purged — so nothing is delivered and nothing panics.
    host.notification_bridge()
        .publish(McpNotification::new(METHOD, json!({ "id": "task.move" })));

    tokio::time::sleep(GRACE).await;
    assert!(
        seen.lock().expect("recorder mutex").is_empty(),
        "an unloaded plugin's callback must not be delivered to, got {:?}",
        seen.lock().expect("recorder mutex")
    );
}

/// The ergonomic `this.<server>.on(event, cb)` surface, end to end against the
/// REAL command tool: `.on("executed", …)` resolves the short event to
/// `notifications/commands/executed` via the command service's declared
/// `io.swissarmyhammer/notifications` `_meta`, subscribes, and delivers.
#[tokio::test]
async fn server_on_resolves_declared_event_and_delivers() {
    let (host, bundle, seen) = host_with_recorder().await;
    // Expose the real command service module so `commands` carries the declared
    // `commands/executed` notification in its `_meta`.
    tokio::time::timeout(TIMEOUT, install_commands_module(&host))
        .await
        .expect("installing the command module should not hang")
        .expect("installing the command module should succeed");

    // No raw transport access — the plugin uses the ergonomic `.on()`.
    write_plugin(
        bundle.path(),
        "this.register('commands', { rust: 'commands' });\n\
         this.register('rec', { rust: 'rec-mod' });\n\
         const rec = this.rec;\n\
         this.commands.on('executed', async (params) => {\n\
           await rec.record({ message: JSON.stringify(params) });\n\
         });",
    );

    tokio::time::timeout(TIMEOUT, host.load(bundle.path()))
        .await
        .expect("loading the plugin should not hang")
        .expect("the plugin's load should succeed");

    host.notification_bridge().publish(McpNotification::new(
        METHOD,
        json!({ "id": "task.move", "result": { "ok": true } }),
    ));

    let recordings = wait_for_recordings(&seen, 1).await;
    let delivered: serde_json::Value =
        serde_json::from_str(&recordings[0]).expect("the callback recorded the params as JSON");
    assert_eq!(
        delivered["id"], "task.move",
        ".on(\"executed\") must resolve to commands/executed and deliver its params"
    );
}

/// `.on()` for an event the server does not declare throws
/// `UnknownNotification` — failing the plugin's load with a message that names
/// the bad event and lists the valid ones.
#[tokio::test]
async fn server_on_unknown_event_throws() {
    let bundle = tempfile::TempDir::new().expect("temp dir");
    let host = PluginHost::for_tests(bundle.path().to_path_buf(), None);
    tokio::time::timeout(TIMEOUT, install_commands_module(&host))
        .await
        .expect("installing the command module should not hang")
        .expect("installing the command module should succeed");

    write_plugin(
        bundle.path(),
        "this.register('commands', { rust: 'commands' });\n\
         this.commands.on('nope', () => {});",
    );

    let err = tokio::time::timeout(TIMEOUT, host.load(bundle.path()))
        .await
        .expect("loading the plugin should not hang")
        .expect_err("load should fail when .on names an undeclared event");
    let message = err.to_string();
    assert!(
        message.contains("nope") && message.contains("executed"),
        "error should name the bad event and list the valid ones, got: {message}"
    );
}

/// The full chain, end to end through a real isolate: a plugin registers a
/// command AND subscribes via `.on("executed", …)`; executing that command
/// runs its callback, the command service's production publisher emits
/// `commands/executed`, and the subscriber's callback fires with the executed
/// command's id. No manual bridge publish — a real `execute` drives it.
#[tokio::test]
async fn real_command_execute_fires_the_on_subscriber() {
    let (host, bundle, seen) = host_with_recorder().await;
    tokio::time::timeout(TIMEOUT, install_commands_module(&host))
        .await
        .expect("installing the command module should not hang")
        .expect("installing the command module should succeed");

    // The plugin registers a command with an `execute` callback, records every
    // `executed` event via the recorder, then registers the command. When the
    // test executes it, the command service publishes `commands/executed`, which
    // the `.on()` subscription delivers back into the isolate.
    write_plugin(
        bundle.path(),
        "this.register('commands', { rust: 'commands' });\n\
         this.register('rec', { rust: 'rec-mod' });\n\
         const rec = this.rec;\n\
         this.commands.on('executed', async (p) => {\n\
           await rec.record({ message: 'event:' + p.id });\n\
         });\n\
         await this.commands.command.command.register({\n\
           id: 'test.ping', name: 'Ping', execute: async () => 'pong',\n\
         });",
    );

    tokio::time::timeout(TIMEOUT, host.load(bundle.path()))
        .await
        .expect("loading the plugin should not hang")
        .expect("the plugin's load should succeed");

    // A real execute through the command service — runs the plugin's execute
    // callback and emits the action event via the production BridgeActionSink.
    tokio::time::timeout(
        TIMEOUT,
        host.call(
            CallerId::HostInternal,
            "commands",
            "command",
            json!({ "op": "execute command", "id": "test.ping", "ctx": {} }),
        ),
    )
    .await
    .expect("executing the command should not hang")
    .expect("executing the registered command should succeed");

    let recordings = wait_for_recordings(&seen, 1).await;
    assert_eq!(
        recordings[0], "event:test.ping",
        "a real command execute must drive the .on(\"executed\") subscriber with the command id"
    );
}
