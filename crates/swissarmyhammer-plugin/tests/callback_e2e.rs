//! End-to-end integration test for the **callback primitive**, driven through
//! a real multi-file plugin.
//!
//! This is the capability-level companion to `callbacks.rs`. Where
//! `callbacks.rs` drives the primitive with inline `entry.ts` bodies and a
//! recording `HostDispatcher`, this test proves the same primitive works when
//! a *real, multi-file plugin bundle* — loaded from disk, its sibling import
//! resolved by [`PluginModuleLoader`], run in a real V8 isolate — hands the
//! host a function, and the function's effect is observed through the *real
//! in-process `files` tool*.
//!
//! It follows the reference shape of `files_dispatch_e2e.rs`: a real V8
//! isolate, a real registered server (the genuine `files` operation tool), and
//! an effect observed on disk that can only happen if every stage works.
//!
//! # The callback primitive, end to end
//!
//! Functions cannot cross the host/plugin boundary. The SDK marshals a
//! function in a callback-bearing dispatch payload into a `{ "$callback": id }`
//! marker and stores the function in an isolate-local table. The host invokes a
//! stored callback by sending a `notifications/callbacks/invoke` into the
//! isolate — the [`PluginRuntime::invoke_callback`] seam — and the SDK runs the
//! stored function and flows its return value back.
//!
//! # What a passing run proves
//!
//! 1. The plugin bundle is **multi-file and loaded from disk**: `entry.ts`
//!    imports a sibling `./probe.ts` with a relative specifier. Loading it
//!    through [`PluginRuntime::call_plugin_lifecycle`] exercises the real
//!    [`PluginModuleLoader`] — the entry, the relative import, and the
//!    `@swissarmyhammer/plugin` SDK virtual module all resolve and transpile.
//! 2. The plugin's `load()` hands the host **a function** in a callback-bearing
//!    `callbackDispatch` payload. The [`CallbackHostBridge`] — doing exactly
//!    what the production host's `callback_dispatch` does — records the
//!    `$callback` marker the SDK substituted.
//! 3. The host invokes that callback by id via `invoke_callback`
//!    (`notifications/callbacks/invoke`). The stored function, when it runs:
//!    - writes a probe file through the **real `files` operation tool** (so the
//!      function genuinely ran inside the isolate and reached a real server),
//!    - and returns a computed value.
//!
//! Two assertions prove the round trip:
//!
//! - the probe file exists on disk holding the host-supplied argument — the
//!   callback function ran, and its `this.fs.files(...)` call reached the real
//!   `files` handler;
//! - `invoke_callback` returned the function's computed value — the return
//!   value flowed back from the isolate to the host.
//!
//! If the callback primitive is broken — the function is not stored, the
//! marker is not produced, `invoke_callback` does not reach the stored
//! function, or the return value does not flow back — at least one assertion
//! fails.
//!
//! # Isolation
//!
//! The test owns its own [`tempfile::TempDir`] roots, a fresh
//! [`PluginRuntime`], and a dedicated bridge runtime; nothing is `static` and
//! no temp dir is reused. Every cross-thread interaction is bounded by a
//! timeout so a wedged isolate or a wedged host task fails the test fast
//! instead of hanging CI.
//!
//! [`PluginModuleLoader`]: swissarmyhammer_plugin::PluginModuleLoader

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use swissarmyhammer_plugin::{
    CallerId, HostDispatcher, McpServer as PluginMcpServer, PluginRuntime, RuntimeConfig,
};
use swissarmyhammer_prompts::PromptLibrary;
use swissarmyhammer_tools::mcp::McpServer;

/// A generous upper bound on any single runtime or host interaction.
///
/// Building the MCP server stands up the full in-process tool registry, so the
/// bound is wider than a bare isolate test would need.
const TIMEOUT: Duration = Duration::from_secs(60);

/// The server name the probe plugin registers the real `files` tool under.
const FILES_SERVER: &str = "fs";

/// The module id the real `files` tool is exposed as — the value a plugin
/// names in `register(name, { rust: id })`.
const FILES_MODULE_ID: &str = "files";

/// The probe file the plugin's callback function writes through the real
/// `files` tool — proof the host-invoked callback actually ran and reached a
/// real server.
const PROBE_FILE: &str = "callback_probe.txt";

/// The argument the host passes when it invokes the plugin's callback. The
/// callback writes it to the probe file and folds it into its return value, so
/// it is observable both on disk and in the value that flows back.
const CALLBACK_INPUT: &str = "payload delivered by notifications/callbacks/invoke";

/// A multi-thread Tokio runtime owned on its own dedicated thread.
///
/// The [`HostDispatcher::dispatch`] seam is synchronous and runs on the plugin
/// isolate's worker thread — which is itself already inside a `block_on`. To
/// run the real `files` tool's *async* `invoke` from there, the future is
/// spawned onto this separate runtime and the worker blocks on a channel for
/// the reply. This mirrors the production host's `bridge_runtime`.
///
/// The runtime is moved onto a parked OS thread and only its
/// [`Handle`](tokio::runtime::Handle) is kept, so the runtime's blocking drop
/// never runs inside another runtime.
struct BridgeRuntime {
    /// A spawn handle into the runtime, used by the dispatcher.
    handle: tokio::runtime::Handle,
    /// Signals the owner thread to drop the runtime; `Some` until [`Drop`].
    shutdown: Option<std::sync::mpsc::Sender<()>>,
    /// The owner thread; joined on drop so shutdown completes before return.
    owner: Option<std::thread::JoinHandle<()>>,
}

impl BridgeRuntime {
    /// Builds the bridge runtime and parks it on its own dedicated thread.
    fn new() -> Self {
        let (handle_tx, handle_rx) = std::sync::mpsc::channel();
        let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel();

        let owner = std::thread::Builder::new()
            .name("callback-e2e-bridge".to_string())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .expect("the callback-e2e bridge runtime must build");
                let _ = handle_tx.send(runtime.handle().clone());
                // Park until the test drops this `BridgeRuntime`. The runtime
                // is then dropped here, on this plain OS thread, so its
                // blocking shutdown never runs inside another runtime.
                let _ = shutdown_rx.recv();
            })
            .expect("the callback-e2e bridge runtime thread must spawn");

        let handle = handle_rx
            .recv()
            .expect("the bridge runtime thread must report its handle");

        Self {
            handle,
            shutdown: Some(shutdown_tx),
            owner: Some(owner),
        }
    }

    /// Spawns `future` on the runtime and blocks — bounded — for its result.
    ///
    /// The isolate worker thread calls this from the synchronous `dispatch`
    /// seam. A host task that never answers becomes a prompt error rather than
    /// a hung worker.
    fn block_on<F, T>(&self, future: F) -> Result<T, String>
    where
        F: std::future::Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        let (tx, rx) = std::sync::mpsc::channel();
        self.handle.spawn(async move {
            let _ = tx.send(future.await);
        });
        rx.recv_timeout(TIMEOUT)
            .map_err(|_| "the bridge runtime did not answer in time".to_string())
    }
}

impl Drop for BridgeRuntime {
    /// Signals the owner thread to drop the runtime, then joins it.
    fn drop(&mut self) {
        drop(self.shutdown.take());
        if let Some(owner) = self.owner.take() {
            let _ = owner.join();
        }
    }
}

/// A [`HostDispatcher`] backed by the *real* in-process `files` operation tool.
///
/// It answers the SDK wire envelopes the plugin emits, doing exactly what the
/// production host does for each — no mocked registry, no mocked tool:
///
/// - `register` — acknowledged; the plugin registers `fs` → `{ rust: "files" }`
///   and the dispatcher already holds that one real module.
/// - `toolsList` — returns the real `files` module's `tools()`, serialized the
///   same way the host's `tools_to_json` serializes them.
/// - `toolsCall` — routes the call into the real `files` module's async
///   `invoke`, run on the bridge runtime. This is the genuine `files` handler.
/// - `callbackDispatch` — records the marshalled payload, exactly as the
///   production host's `callback_dispatch` does, so the `$callback` marker the
///   SDK substituted for the plugin's function is observable to the test.
struct CallbackHostBridge {
    /// The real in-process `files` tool, as a platform `McpServer`.
    files: Arc<dyn PluginMcpServer>,
    /// The runtime the async `files` `invoke` is driven on.
    bridge: BridgeRuntime,
    /// Every callback-bearing payload the SDK dispatched, in order.
    callback_payloads: Mutex<Vec<Value>>,
}

impl CallbackHostBridge {
    /// Builds a bridge over the real `files` tool exposed by `server`.
    ///
    /// Panics if the MCP server does not expose a `files` module — it always
    /// does in `agent_mode`, the mode the test builds the server in.
    async fn new(server: &McpServer) -> Self {
        let modules = server.plugin_tool_modules().await;
        let (_, files) = modules
            .into_iter()
            .find(|(id, _)| id == FILES_MODULE_ID)
            .expect("the `files` tool should be exposed as a Rust module");
        Self {
            files,
            bridge: BridgeRuntime::new(),
            callback_payloads: Mutex::new(Vec::new()),
        }
    }

    /// Returns the callback-bearing payloads recorded so far.
    fn callback_payloads(&self) -> Vec<Value> {
        self.callback_payloads
            .lock()
            .expect("callback payloads mutex")
            .clone()
    }

    /// Answers a `toolsList` envelope with the real `files` module's tools.
    fn tools_list(&self, payload: &Value) -> Result<Value, String> {
        let server = payload.get("server").and_then(Value::as_str).unwrap_or("");
        if server != FILES_SERVER {
            return Err(format!("unknown server '{server}'"));
        }
        // Serialize each real `rmcp` tool the same way the host does — this is
        // what carries the tool name and any `_meta` the SDK resolves against.
        let tools: Vec<Value> = PluginMcpServer::tools(self.files.as_ref())
            .iter()
            .map(|tool| {
                serde_json::to_value(tool.as_tool())
                    .expect("an rmcp Tool always serializes to JSON")
            })
            .collect();
        Ok(json!({ "tools": tools }))
    }

    /// Answers a `toolsCall` envelope by routing into the real `files` tool.
    fn tools_call(&self, payload: &Value) -> Result<Value, String> {
        let server = payload.get("server").and_then(Value::as_str).unwrap_or("");
        if server != FILES_SERVER {
            return Err(format!("unknown server '{server}'"));
        }
        let tool = payload
            .get("tool")
            .and_then(Value::as_str)
            .ok_or_else(|| "toolsCall envelope missing 'tool'".to_string())?
            .to_string();
        let arguments = payload
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));

        // The real `files` tool's `invoke` is async; drive it on the bridge
        // runtime and block the isolate worker — bounded — for the result.
        let files = Arc::clone(&self.files);
        self.bridge
            .block_on(async move { files.invoke(CallerId::HostInternal, &tool, arguments).await })?
            .map_err(|error| error.to_string())
    }

    /// Records a `callbackDispatch` envelope's marshalled payload.
    ///
    /// This is exactly what the production host's `callback_dispatch` does with
    /// the callback-bearing path: it treats the `$callback` markers as opaque
    /// handles and keeps the payload so the marker ids are observable.
    fn callback_dispatch(&self, payload: &Value) -> Result<Value, String> {
        let inner = payload
            .get("payload")
            .cloned()
            .ok_or_else(|| "callbackDispatch envelope missing 'payload'".to_string())?;
        self.callback_payloads
            .lock()
            .expect("callback payloads mutex")
            .push(inner);
        Ok(json!({ "ok": true }))
    }
}

impl HostDispatcher for CallbackHostBridge {
    /// Routes one SDK wire envelope by its `kind`.
    fn dispatch(&self, payload: Value) -> Result<Value, String> {
        let kind = payload
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| "bridge payload missing 'kind'".to_string())?;
        match kind {
            // The plugin registers `fs` → `{ rust: "files" }`; the dispatcher
            // already holds that one real module, so this is acknowledged.
            "register" => Ok(json!({ "ok": true })),
            "toolsList" => self.tools_list(&payload),
            "toolsCall" => self.tools_call(&payload),
            "callbackDispatch" => self.callback_dispatch(&payload),
            other => Err(format!("unsupported bridge kind '{other}'")),
        }
    }
}

/// Builds a real MCP server against an isolated temp working directory.
///
/// The temp `work_dir` keeps the server's bootstrap from walking the real
/// monorepo and gives the `files` tool a clean place to write. `agent_mode` is
/// `true` so the unified `files` tool — the operation tool this test routes the
/// callback's effect through — is registered and reachable for exposure.
async fn build_mcp_server(work_dir: &Path) -> McpServer {
    McpServer::new_with_work_dir(PromptLibrary::new(), work_dir.to_path_buf(), None, true)
        .await
        .expect("MCP server bootstrap should succeed")
}

/// Encodes `value` as a JSON/TypeScript string literal, quotes included.
fn json_string(value: &str) -> String {
    serde_json::to_string(value).expect("a string always serializes to JSON")
}

/// Writes the probe plugin bundle — a real `plugin.json` and a real, **multi-
/// file** TypeScript entry — into `bundle_dir`.
///
/// The bundle is genuinely multi-file: `entry.ts` imports a sibling
/// `./probe.ts` with a relative specifier. Loading it exercises the real
/// [`PluginModuleLoader`](swissarmyhammer_plugin::PluginModuleLoader) — the
/// entry, the relative import, and the `@swissarmyhammer/plugin` SDK virtual
/// module all resolve and transpile.
///
/// `probe.ts` exports `makeCallbackHandler`, which builds the function the
/// plugin hands the host. The handler, when the host invokes it:
///
/// 1. writes its string argument into the probe file through the real `files`
///    tool registered as `fs` — an effect the test observes on disk;
/// 2. returns a string folding the argument in — a value the test observes
///    flowing back from `invoke_callback`.
///
/// `entry.ts`'s `load()` registers the real `files` tool as `fs` and hands the
/// host the handler in a `callbackDispatch` payload, so the SDK marshals it
/// into a `$callback` marker.
fn write_probe_plugin(bundle_dir: &Path, probe_path: &Path) {
    // A real manifest. `provides` lists the one server name `load()` registers.
    let manifest = "{\n  \
         \"id\": \"probe\",\n  \
         \"name\": \"callback primitive probe\",\n  \
         \"version\": \"1.0.0\",\n  \
         \"entry\": \"entry.ts\",\n  \
         \"provides\": [\"fs\"]\n}\n";
    std::fs::write(bundle_dir.join("plugin.json"), manifest)
        .expect("probe plugin.json should be written");

    // The sibling module `entry.ts` imports with a relative specifier. It owns
    // the callback function: when the host invokes it, it writes the host's
    // argument to disk through the real `files` tool and returns a value.
    let probe_module = format!(
        "import type {{ ServerDispatcher }} from '@swissarmyhammer/plugin';\n\
         \n\
         /** The probe file path the callback writes through the `files` tool. */\n\
         const PROBE_PATH = {probe};\n\
         \n\
         /**\n\
         \x20* Builds the function the plugin hands the host as a callback.\n\
         \x20*\n\
         \x20* `files` is the dispatcher for the registered real `files` tool.\n\
         \x20* When the host invokes the returned function it writes its\n\
         \x20* argument to disk through that tool and returns a folded value.\n\
         \x20*/\n\
         export function makeCallbackHandler(\n\
         \x20 files: ServerDispatcher,\n\
         ): (input: string) => Promise<string> {{\n\
         \x20 return async (input: string): Promise<string> => {{\n\
         \x20   await files.files({{\n\
         \x20     op: 'write file',\n\
         \x20     file_path: PROBE_PATH,\n\
         \x20     content: input,\n\
         \x20   }});\n\
         \x20   return 'callback ran with: ' + input;\n\
         \x20 }};\n\
         }}\n",
        probe = json_string(&probe_path.to_string_lossy()),
    );
    std::fs::write(bundle_dir.join("probe.ts"), probe_module)
        .expect("probe probe.ts should be written");

    // The entry module. `load()` registers the real `files` tool as `fs`, then
    // hands the host the callback handler from the sibling module in a
    // callback-bearing dispatch — the SDK marshals the function into a
    // `{ $callback: id }` marker the test then drives.
    let entry = "import { Plugin, makePluginThis } from '@swissarmyhammer/plugin';\n\
         import { makeCallbackHandler } from './probe.ts';\n\
         \n\
         class ProbePlugin extends Plugin {\n\
         \x20 async load(): Promise<void> {\n\
         \x20   // Activate the host-exposed real `files` tool under `fs`.\n\
         \x20   this.register('fs', { rust: 'files' });\n\
         \n\
         \x20   // Hand the host a function in a callback-bearing payload. The\n\
         \x20   // SDK marshals it into a `{ $callback: id }` marker; the host\n\
         \x20   // invokes it later via notifications/callbacks/invoke.\n\
         \x20   const handler = makeCallbackHandler(this.fs);\n\
         \x20   this.__transport.callbackDispatch({ handler });\n\
         \x20 }\n\
         }\n\
         \n\
         export async function load(): Promise<unknown> {\n\
         \x20 const p = makePluginThis(new ProbePlugin()) as ProbePlugin;\n\
         \x20 await p.load();\n\
         \x20 return null;\n\
         }\n";
    std::fs::write(bundle_dir.join("entry.ts"), entry).expect("probe entry.ts should be written");
}

/// Reads the single `$callback` id at `payload[field]`.
fn callback_id(payload: &Value, field: &str) -> String {
    payload
        .get(field)
        .and_then(|marker| marker.get("$callback"))
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("expected a $callback marker at '{field}', got {payload}"))
        .to_string()
}

/// A real multi-file plugin hands the host a function; the host invokes it via
/// `notifications/callbacks/invoke` and the function runs and returns a value.
///
/// This single test stitches the callback primitive together end to end:
///
/// - the probe bundle is **multi-file** (`entry.ts` + a sibling `./probe.ts`)
///   and **loaded from disk** through [`PluginRuntime::call_plugin_lifecycle`],
///   which drives the real [`PluginModuleLoader`] — entry, relative import, and
///   SDK virtual module all resolve and transpile in a fresh V8 isolate;
/// - the plugin's `load()` hands the host a function in a `callbackDispatch`
///   payload; the SDK marshals it into a `$callback` marker, which the
///   [`CallbackHostBridge`] records exactly as the production host does;
/// - the host invokes that callback by id with `invoke_callback`
///   (`notifications/callbacks/invoke`); the stored function runs inside the
///   isolate — writing a probe file through the real `files` operation tool —
///   and its return value flows back.
#[tokio::test]
async fn host_invokes_a_real_plugins_callback_end_to_end() {
    // Per-test isolation: every root is this test's own `TempDir`.
    let work_dir = tempfile::TempDir::new().expect("work dir temp");
    let bundle_dir = tempfile::TempDir::new().expect("plugin bundle temp");
    let output_dir = tempfile::TempDir::new().expect("probe output temp");

    let probe_path = output_dir.path().join(PROBE_FILE);

    // Lay out the real multi-file probe bundle on disk.
    write_probe_plugin(bundle_dir.path(), &probe_path);

    // The real in-process tool set, including the unified `files` tool the
    // callback's effect is routed through.
    let server = build_mcp_server(work_dir.path()).await;

    // The host dispatcher backed by the real `files` tool. It records the
    // callback markers the plugin's `callbackDispatch` produces.
    let bridge = Arc::new(CallbackHostBridge::new(&server).await);

    // A fresh runtime whose SDK bridge is the real-`files`-backed dispatcher.
    let runtime = PluginRuntime::new(RuntimeConfig {
        dispatcher: Some(bridge.clone() as Arc<dyn HostDispatcher>),
        ..Default::default()
    })
    .expect("the plugin runtime should start");

    // Load the multi-file bundle from disk and run its `load` export. This
    // drives the real module loader (entry + relative import + SDK) and the
    // plugin hands the host its callback function.
    tokio::time::timeout(
        TIMEOUT,
        runtime.call_plugin_lifecycle(bundle_dir.path(), "entry.ts", "load"),
    )
    .await
    .expect("loading the multi-file probe bundle should not hang")
    .expect("the probe plugin's load should succeed");

    // The plugin handed the host a function: the SDK marshalled it into a
    // `$callback` marker, which the dispatcher recorded.
    let payloads = bridge.callback_payloads();
    assert_eq!(
        payloads.len(),
        1,
        "the plugin should have dispatched exactly one callback-bearing payload"
    );
    let id = callback_id(&payloads[0], "handler");
    assert!(
        id.starts_with("cb_"),
        "the SDK must marshal the plugin's function into a `cb_`-prefixed \
         callback marker, got '{id}'"
    );

    // The host invokes the stored callback by id — the host→isolate direction
    // of the primitive, delivered as `notifications/callbacks/invoke`.
    let returned = tokio::time::timeout(
        TIMEOUT,
        runtime.invoke_callback(&id, json!([CALLBACK_INPUT])),
    )
    .await
    .expect("invoking the callback should not hang")
    .expect("invoking the plugin-supplied callback should succeed");

    // Assertion 1 — the probe file exists holding the host-supplied argument.
    // This can only be true if `invoke_callback` reached the stored function,
    // the function ran inside the isolate, and its `this.fs.files(...)` call
    // routed through the real `files` operation tool's handler.
    let written = std::fs::read_to_string(&probe_path).unwrap_or_else(|error| {
        panic!(
            "the callback probe file must exist at {} — the host-invoked \
             callback did not run or did not reach the real files tool: {error}",
            probe_path.display()
        )
    });
    assert_eq!(
        written, CALLBACK_INPUT,
        "the probe file must hold the argument the host passed to the callback \
         — proving the plugin's function ran and reached the real files tool"
    );

    // Assertion 2 — the callback's return value flowed back to the host.
    // `invoke_callback` returns what the stored function returned, so this
    // proves the return value crossed the isolate boundary back to the host.
    assert_eq!(
        returned,
        json!(format!("callback ran with: {CALLBACK_INPUT}")),
        "invoke_callback must return the callback's computed value — proving \
         the return value flowed back from the isolate to the host"
    );
}
