//! Shared helpers for the bootstrap integration tests.
//!
//! The helpers here cover the boilerplate every test in this suite
//! repeats: spinning up a [`PluginHost`] with a temp user-layer root,
//! routing a host or plugin caller's `register command` through the
//! bootstrap-wired `CommandService`, listing the active stack, and
//! writing a minimal probe-plugin bundle to disk so a `host.load(...)`
//! pulls real plugin lifecycle through the wiring under test.
//!
//! Verb calls go through the service's `rmcp::ServerHandler` surface
//! directly — bypassing the host's registry activation step that a
//! plugin would normally drive — so the tests focus on the bootstrap
//! wiring (callback dispatcher + caller lifecycle + module exposure)
//! without depending on the still-finalising SDK callback-marshalling
//! path.

#![allow(dead_code)] // shared by multiple submodules

use std::borrow::Cow;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rmcp::model::{CallToolRequestParams, NumberOrString};
use rmcp::service::{serve_directly, Peer, RequestContext, RxJsonRpcMessage, TxJsonRpcMessage};
use rmcp::transport::Transport;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use serde_json::{json, Value};
use swissarmyhammer_command_service::bootstrap::install_commands_module;
use swissarmyhammer_command_service::CommandService;
use swissarmyhammer_plugin::{CallerId, PluginHost};
use tempfile::TempDir;

/// One-stop scaffolding for a bootstrap integration test.
///
/// Owns a temp [`TempDir`] (kept alive for the test's duration), the
/// host built against it, and the [`Arc<CommandService>`] the bootstrap
/// returned. Tests drive the service either through the host's
/// `commands` server (the production path) or by calling the service
/// directly via the returned handle.
pub struct BootstrappedHost {
    /// The temp user-layer root, kept alive so its directory survives
    /// the test.
    pub _user_root: TempDir,
    /// The live plugin host the bootstrap wired into.
    pub host: PluginHost,
    /// Shared handle to the bootstrapped command service.
    pub service: Arc<CommandService>,
}

impl BootstrappedHost {
    /// Build a fresh host, install the commands module, and return all
    /// three handles wrapped.
    pub async fn new() -> Self {
        let user_root = TempDir::new().expect("user root temp dir");
        let host = PluginHost::for_tests(user_root.path().to_path_buf(), None);
        let service = install_commands_module(&host)
            .await
            .expect("install_commands_module must succeed");
        Self {
            _user_root: user_root,
            host,
            service,
        }
    }
}

/// A transport that yields no messages and closes immediately, used
/// solely to mint a [`Peer<RoleServer>`] for the [`RequestContext`] an
/// rmcp call needs.
struct ClosedTransport;

impl Transport<RoleServer> for ClosedTransport {
    type Error = std::io::Error;

    fn send(
        &mut self,
        _item: TxJsonRpcMessage<RoleServer>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send + 'static {
        std::future::ready(Ok(()))
    }

    fn receive(&mut self) -> impl Future<Output = Option<RxJsonRpcMessage<RoleServer>>> + Send {
        std::future::ready(None)
    }

    fn close(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send {
        std::future::ready(Ok(()))
    }
}

/// Mint an inert [`Peer<RoleServer>`] by briefly serving a placeholder
/// handler over a closed transport.
fn mint_peer() -> Peer<RoleServer> {
    struct PeerProbe;
    impl ServerHandler for PeerProbe {}

    let running = serve_directly(PeerProbe, ClosedTransport, None);
    running.peer().clone()
}

/// Build a [`RequestContext`] pre-populated with `caller` in its
/// extensions.
///
/// Mirrors what the in-process transport does in production: the
/// transport plants the [`CallerId`] before dispatching `call_tool`,
/// and the service reads it back from the context's extensions.
fn request_context_for(caller: CallerId) -> RequestContext<RoleServer> {
    let mut context = RequestContext::new(NumberOrString::Number(0), mint_peer());
    context.extensions.insert(caller);
    context
}

/// Invoke `arguments` against `service`'s `command` tool, with
/// `caller` planted in the request context's extensions.
///
/// Wraps the rmcp boilerplate so tests read at the verb level: build a
/// `CallToolRequestParams { name: "command", arguments }` and dispatch.
/// Returns the rmcp `CallToolResult` serialized to a [`Value`] — the
/// same wire shape an MCP `tools/call` response carries.
pub async fn call_command(service: &CommandService, caller: CallerId, arguments: Value) -> Value {
    try_call_command(service, caller, arguments)
        .await
        .expect("call_command should succeed")
}

/// Like [`call_command`] but surfaces the underlying [`McpError`] instead of
/// panicking, so tests can assert on negative paths (a command whose execute
/// callback the dispatcher cannot route to, for example).
///
/// Returns the rmcp `CallToolResult` serialized to a [`Value`] on success, or
/// the verb dispatcher's `McpError` on failure.
pub async fn try_call_command(
    service: &CommandService,
    caller: CallerId,
    arguments: Value,
) -> Result<Value, McpError> {
    let context = request_context_for(caller);
    let mut request = CallToolRequestParams::new(Cow::Borrowed("command"));
    if let Value::Object(map) = arguments {
        request = request.with_arguments(map);
    }
    let result = service.call_tool(request, context).await?;
    Ok(serde_json::to_value(result).expect("CallToolResult must serialise to a JSON value"))
}

/// Build a `tools/call` arguments object for `register command`.
///
/// The execute callback marker carries `callback_id` verbatim — the
/// SDK's auto-marshalling is bypassed here because every test driving
/// the bootstrap from Rust supplies callback ids by hand rather than
/// minting them through a plugin isolate.
pub fn register_args(id: &str, name: &str, callback_id: &str) -> Value {
    json!({
        "op": "register command",
        "id": id,
        "name": name,
        "execute": { "$callback": callback_id },
    })
}

/// Build a `tools/call` arguments object for `list command`.
pub fn list_args() -> Value {
    json!({ "op": "list command" })
}

/// Build a `tools/call` arguments object for `execute command` with no `ctx`
/// payload — the active stack entry's execute callback runs with a default
/// context.
pub fn execute_args(id: &str) -> Value {
    json!({ "op": "execute command", "id": id })
}

/// Pull the `result` field out of an `execute command` response value.
///
/// The wire shape is `{ "structuredContent": { "ok": true, "result": <value> } }`.
/// Panics if the field is missing — the success path always populates it, so a
/// missing `result` is a test scaffolding bug, not an expected outcome.
pub fn execute_result(response: &Value) -> Value {
    response
        .get("structuredContent")
        .and_then(|sc| sc.get("result"))
        .cloned()
        .unwrap_or_else(|| {
            panic!("execute response must carry structuredContent.result, got {response}")
        })
}

/// Walk the rmcp `CallToolResult`-shaped JSON value `result` returned by
/// `list command` and return the active commands' ids in iteration order.
///
/// The result is the wire shape `{ "structuredContent": { ok, commands:
/// [...] } }`; each entry in `commands` carries an `id` field.
pub fn ids_of(list_result: &Value) -> Vec<String> {
    let commands = list_result
        .get("structuredContent")
        .and_then(|sc| sc.get("commands"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    commands
        .into_iter()
        .filter_map(|entry| entry.get("id").and_then(Value::as_str).map(str::to_string))
        .collect()
}

/// Write a no-op probe plugin bundle into `plugins_dir/<id>/`.
///
/// The bundle is the minimal real plugin: a TypeScript `index.ts` that
/// defines a `Plugin` subclass whose `load()` is a no-op. Loading the
/// bundle drives the host's full lifecycle path — isolate creation,
/// transpile, evaluate, run `load()` — without registering anything.
/// The test then drives the command service from the Rust side using
/// the plugin's [`PluginId`](swissarmyhammer_plugin::PluginId), exactly
/// as a future SDK helper would do from inside the plugin.
pub fn write_noop_probe_plugin(plugins_dir: &Path, id: &str) -> PathBuf {
    let plugin_dir = plugins_dir.join(id);
    std::fs::create_dir_all(&plugin_dir).expect("plugin dir");
    let entry = format!(
        "import {{ Plugin }} from '@swissarmyhammer/plugin';\n\
         export default class P extends Plugin {{\n\
           async load(): Promise<void> {{\n\
             this.log.info('{id} loaded');\n\
           }}\n\
         }}\n"
    );
    std::fs::write(plugin_dir.join("index.ts"), entry).expect("index.ts");
    plugin_dir
}

/// Write a probe plugin bundle that registers `command_id` via the SDK's
/// `registerCommands` helper with an `execute` callback that returns
/// `sentinel` verbatim.
///
/// The bundle's `load()` follows the canonical command-registering convention:
/// `await ensureServices(this, ["commands"])` first, then
/// `await registerCommands(this, [...])`. The execute function value is
/// marshalled to a `$callback` marker by the SDK's `toolsCall` path on the way
/// out of the isolate, so the host's later `invoke_plugin_callback(...)`
/// reaches the same function and observes its return value.
///
/// This is the round-trip sentinel mechanism the override-stack
/// re-emergence test uses: each probe registers the same `command_id`
/// with a distinct `sentinel`, and the test asserts the active stack
/// entry actually runs by invoking `execute command` and matching the
/// callback's return value against the expected top-of-stack plugin's
/// sentinel.
///
/// # Why the return value is the sentinel
///
/// The original task description suggested a temp-file side-effect, but
/// the SDK does not expose a filesystem surface to plugin isolates —
/// host traffic crosses the boundary through `op_host_dispatch` only.
/// The execute callback's return value already round-trips verbatim
/// through `handle_execute` (`{ ok: true, result: <value> }`), which is
/// a strictly more direct signal than a file write: the test reads the
/// callback's output through the same wire the production caller would.
pub fn write_sentinel_probe_plugin(
    plugins_dir: &Path,
    id: &str,
    command_id: &str,
    sentinel: &str,
) -> PathBuf {
    let plugin_dir = plugins_dir.join(id);
    std::fs::create_dir_all(&plugin_dir).expect("plugin dir");
    // The bundle is deliberately minimal — no descriptive metadata, no
    // unload hook. The SDK's per-plugin ledger drains the command and
    // callback registrations on unload without an explicit body, which
    // is the contract the override-stack test relies on for step
    // "unload B/A re-emerges the previous registration".
    let entry = format!(
        "import {{\n\
           Plugin,\n\
           ensureServices,\n\
           registerCommands,\n\
         }} from '@swissarmyhammer/plugin';\n\
         export default class P extends Plugin {{\n\
           async load(): Promise<void> {{\n\
             await ensureServices(this, ['commands']);\n\
             await registerCommands(this, [{{\n\
               id: '{command_id}',\n\
               name: '{id} archive',\n\
               execute: () => '{sentinel}',\n\
             }}]);\n\
             this.log.info('{id} registered {command_id} with sentinel {sentinel}');\n\
           }}\n\
         }}\n"
    );
    std::fs::write(plugin_dir.join("index.ts"), entry).expect("index.ts");
    plugin_dir
}
