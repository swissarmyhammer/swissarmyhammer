//! The JavaScript runtime that hosts plugin code.
//!
//! Each plugin runs in its own [`PluginRuntime`] — a dedicated V8 isolate
//! wrapped in a `deno_core::JsRuntime`. One isolate per plugin gives the
//! platform two properties it needs:
//!
//! - **Fault isolation** — a plugin that exhausts memory, throws, or wedges
//!   its event loop cannot corrupt another plugin's state, because globals,
//!   the heap, and the module graph are all isolate-local.
//! - **Clean teardown** — dropping a [`PluginRuntime`] tears the whole isolate
//!   down, reclaiming everything the plugin allocated.
//!
//! # Threading model
//!
//! `deno_core::JsRuntime` wraps a V8 isolate, which is single-threaded and
//! `!Send`. The runtime is therefore owned by exactly one dedicated OS thread
//! — the *worker* — and every other thread talks to it by sending [`Command`]
//! values down an [`mpsc`](std::sync::mpsc) channel. Each command carries a
//! [`oneshot`](tokio::sync::oneshot) reply sender, so callers (which are
//! `async`) await the worker's response without blocking. This mirrors the
//! worker model in the `swissarmyhammer-js` crate.
//!
//! # TypeScript
//!
//! Plugin modules are TypeScript. They are transpiled to JavaScript at
//! load time by [`transpile`] — a purely syntactic transform with no
//! type-checking — and the transpiled code carries an inline source map so V8
//! stack traces and any attached inspector report original TypeScript
//! positions.
//!
//! # The host bridge
//!
//! Plugin code calls back into the host through a single op installed by the
//! [`bridge`] module. This crate provides only that seam; the SDK and
//! `PluginHost` tasks wire a real dispatcher into it.

mod bridge;
mod transpile;

pub use bridge::{HostDispatcher, UnboundHostDispatcher};
pub use transpile::{transpile_typescript, TranspiledModule};

use std::rc::Rc;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use deno_core::v8;
use deno_core::{
    JsRuntime, ModuleSpecifier, NoopModuleLoader, PollEventLoopOptions, RuntimeOptions,
};
use tokio::sync::oneshot;

use crate::error::{Error, Result};

/// Initial V8 heap size for a plugin isolate (1 MiB).
const HEAP_INITIAL_BYTES: usize = 1024 * 1024;

/// Maximum V8 heap size for a plugin isolate (64 MiB).
///
/// This is the cap passed to [`v8::CreateParams::heap_limits`]. On its own
/// that cap makes V8 treat an exhausted heap as a *fatal, process-level* OOM —
/// it would abort the whole host. The near-heap-limit callback the worker
/// registers (see [`worker_loop`]) is what makes the cap an isolate-local
/// failure: when a plugin nears this limit its script is terminated instead of
/// the host being aborted. The limit is larger than the `swissarmyhammer-js`
/// expression engine's because a plugin module is a full program rather than a
/// one-line expression.
const HEAP_MAX_BYTES: usize = 64 * 1024 * 1024;

/// How long a caller waits for the worker thread to answer one command.
///
/// A bounded wait turns a wedged isolate (an infinite loop, a never-settling
/// promise) into a prompt [`Error::RuntimeTimeout`] instead of a hang.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

/// Configuration for a [`PluginRuntime`].
#[derive(Debug, Clone, Default)]
pub struct RuntimeConfig {
    /// TCP port for the V8 Inspector, or `None` to disable it.
    ///
    /// When `Some`, the isolate is created with inspector support so a
    /// DevTools-protocol client can attach. This is the runtime side of a
    /// `--inspect[=PORT]` flag and is expected to be set only in dev mode;
    /// production plugin hosts leave it `None`.
    ///
    /// The runtime initializes the in-isolate inspector; standing up the TCP
    /// server that DevTools connects to on this port is the embedder's
    /// responsibility and is wired by a later task.
    pub inspect_port: Option<u16>,
}

/// A handle to one plugin's JavaScript runtime.
///
/// The handle is cheap to hold and `Send`; the V8 isolate it controls lives on
/// a dedicated worker thread. All methods are `async` because each one round-
/// trips a [`Command`] to that worker. Dropping the handle shuts the worker —
/// and the isolate — down.
pub struct PluginRuntime {
    /// Channel to the worker thread that owns the isolate.
    sender: mpsc::Sender<Command>,

    /// Thread-safe handle to the worker's V8 isolate.
    ///
    /// Held so teardown can call [`v8::IsolateHandle::terminate_execution`]
    /// from the dropping thread, forcing a wedged isolate (one stuck in a
    /// non-terminating plugin script) to unwind so the worker thread can exit.
    /// `None` only if the worker failed to build its isolate during startup.
    isolate: Option<v8::IsolateHandle>,

    /// Join handle for the worker, taken during [`PluginRuntime::shutdown`].
    worker: Option<std::thread::JoinHandle<()>>,
}

/// Compile-time proof that [`PluginRuntime`] is `Send`.
///
/// The handle's documented contract is that it can be moved across threads
/// while the isolate stays pinned to its worker. This assertion fails the
/// build if an accidentally `!Send` field is ever added.
const _: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<PluginRuntime>();
};

/// Outcome the worker reports once it has finished (or failed) startup.
///
/// On success it carries the isolate's [`v8::IsolateHandle`], which the
/// [`PluginRuntime`] holds so teardown can terminate a wedged isolate from
/// another thread. On failure it carries a human-readable reason.
type StartupResult = std::result::Result<v8::IsolateHandle, String>;

/// A unit of work sent to the worker thread.
///
/// Every variant carries a [`oneshot`] sender so the worker can hand a result
/// back to the awaiting caller.
enum Command {
    /// Evaluate a JavaScript snippet and return its value as JSON.
    EvalScript {
        /// The JavaScript source to evaluate.
        code: String,
        /// Where the result (or an error) is delivered.
        reply: oneshot::Sender<Result<serde_json::Value>>,
    },

    /// Transpile a TypeScript module, evaluate it, and optionally call one of
    /// its exported lifecycle functions.
    LoadModule {
        /// Module URL used in stack traces and the source map.
        specifier: String,
        /// TypeScript source of the plugin entry module.
        source: String,
        /// Name of an exported function to call after evaluation, if any.
        lifecycle_export: Option<String>,
        /// Where the lifecycle return value (or an error) is delivered.
        reply: oneshot::Sender<Result<serde_json::Value>>,
    },
}

impl PluginRuntime {
    /// Spawn a new plugin runtime: a fresh V8 isolate on its own worker thread.
    ///
    /// # Arguments
    ///
    /// * `config` - Isolate configuration, including optional inspector support.
    ///
    /// # Errors
    ///
    /// Returns [`Error::RuntimeStartup`] if the worker thread cannot be
    /// spawned, or if the worker fails to build its V8 isolate or Tokio
    /// runtime during startup.
    pub fn new(config: RuntimeConfig) -> Result<Self> {
        let (sender, receiver) = mpsc::channel::<Command>();
        // The worker reports startup success — and hands back its isolate
        // handle — through this one-shot channel before it serves commands.
        let (ready_tx, ready_rx) = mpsc::channel::<StartupResult>();

        let worker = std::thread::Builder::new()
            .name("plugin-runtime".to_string())
            .spawn(move || worker_loop(config, receiver, ready_tx))
            .map_err(|e| Error::RuntimeStartup(format!("failed to spawn worker thread: {e}")))?;

        // Wait for the worker to finish building its isolate. A dropped
        // channel means the worker panicked before reporting; an `Err`
        // payload means it failed to build the runtime.
        let isolate = match ready_rx.recv() {
            Ok(Ok(handle)) => handle,
            Ok(Err(message)) => {
                let _ = worker.join();
                return Err(Error::RuntimeStartup(message));
            }
            Err(_) => {
                let _ = worker.join();
                return Err(Error::RuntimeStartup(
                    "worker thread exited before reporting startup".to_string(),
                ));
            }
        };

        Ok(Self {
            sender,
            isolate: Some(isolate),
            worker: Some(worker),
        })
    }

    /// Evaluate a JavaScript snippet in the isolate and return its value.
    ///
    /// The snippet runs as a classic script (not a module), so it shares the
    /// isolate's global object: a global it assigns is observable by later
    /// calls. The result value is converted to JSON.
    ///
    /// # Arguments
    ///
    /// * `code` - JavaScript source to evaluate.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Runtime`] if the script throws or has a syntax error,
    /// or [`Error::RuntimeTimeout`] / [`Error::RuntimeStopped`] if the worker
    /// does not answer.
    pub async fn eval(&self, code: impl Into<String>) -> Result<serde_json::Value> {
        let (reply, response) = oneshot::channel();
        self.send(Command::EvalScript {
            code: code.into(),
            reply,
        })?;
        await_reply(response).await
    }

    /// Transpile and evaluate a TypeScript plugin module.
    ///
    /// The `source` is transpiled to JavaScript (syntactically — no type
    /// checking) and evaluated as the isolate's main module. The returned
    /// value is JSON `null`: the module's exports are reached via
    /// [`PluginRuntime::call_lifecycle`].
    ///
    /// # Arguments
    ///
    /// * `specifier` - Module URL for stack traces and the source map.
    /// * `source` - TypeScript source of the plugin entry module.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Transpile`] for a TypeScript syntax error,
    /// [`Error::Runtime`] if the module throws while evaluating, or a
    /// worker-communication error.
    pub async fn load_module(
        &self,
        specifier: impl Into<String>,
        source: impl Into<String>,
    ) -> Result<()> {
        let (reply, response) = oneshot::channel();
        self.send(Command::LoadModule {
            specifier: specifier.into(),
            source: source.into(),
            lifecycle_export: None,
            reply,
        })?;
        await_reply(response).await.map(|_| ())
    }

    /// Transpile and evaluate a TypeScript plugin module, then call one of its
    /// exported functions.
    ///
    /// This is the entry point for plugin lifecycle hooks: load the entry
    /// module and immediately invoke an exported function such as `activate`.
    /// If the function returns a promise, the isolate's event loop is run
    /// until it settles.
    ///
    /// # Arguments
    ///
    /// * `specifier` - Module URL for stack traces and the source map.
    /// * `source` - TypeScript source of the plugin entry module.
    /// * `export` - Name of the exported function to call.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Transpile`] for a syntax error, [`Error::Runtime`] if
    /// the module or the lifecycle function throws or the export is missing or
    /// not a function, or a worker-communication error.
    pub async fn call_lifecycle(
        &self,
        specifier: impl Into<String>,
        source: impl Into<String>,
        export: impl Into<String>,
    ) -> Result<serde_json::Value> {
        let (reply, response) = oneshot::channel();
        self.send(Command::LoadModule {
            specifier: specifier.into(),
            source: source.into(),
            lifecycle_export: Some(export.into()),
            reply,
        })?;
        await_reply(response).await
    }

    /// Shut the runtime down: stop the worker thread and tear down the isolate.
    ///
    /// Dropping the worker's command sender ends its receive loop and the
    /// isolate is dropped as that thread unwinds. Any plugin script still
    /// running — including a non-terminating one — is first forcibly
    /// terminated via the retained [`v8::IsolateHandle`], so this cannot hang.
    /// This is also what [`Drop`] does, so calling `shutdown` explicitly is
    /// only needed to *wait* for teardown to finish and observe its result.
    ///
    /// # Errors
    ///
    /// Returns [`Error::RuntimeStopped`] if the worker thread panicked.
    pub fn shutdown(mut self) -> Result<()> {
        self.join_worker()
    }

    /// Send a command to the worker, mapping a dead channel to an error.
    fn send(&self, command: Command) -> Result<()> {
        self.sender.send(command).map_err(|_| Error::RuntimeStopped)
    }

    /// Drop the command channel, unwedge the isolate, and join the worker.
    ///
    /// Closing the command channel ends the worker's `recv` loop *once it is
    /// back at `recv`* — but a worker still executing a non-terminating plugin
    /// script never returns there. So before joining, this terminates V8
    /// execution on the isolate from this thread via the retained
    /// [`v8::IsolateHandle`]: a wedged script unwinds with an uncatchable
    /// "execution terminated" exception, the worker falls back to `recv`, sees
    /// the closed channel, and exits. This guarantees `join` cannot deadlock.
    fn join_worker(&mut self) -> Result<()> {
        // Replace the live sender with a fresh, unconnected one so the worker's
        // `recv` sees the channel close and exits its loop.
        let (dead, _) = mpsc::channel();
        self.sender = dead;

        // Force any in-flight (possibly non-terminating) plugin script to
        // unwind so the worker can reach `recv` and observe the closed channel.
        // `IsolateHandle` is thread-safe by design and may be called here even
        // though the isolate itself lives on the worker thread.
        if let Some(isolate) = self.isolate.take() {
            isolate.terminate_execution();
        }

        match self.worker.take() {
            Some(handle) => handle.join().map_err(|_| Error::RuntimeStopped),
            None => Ok(()),
        }
    }
}

impl Drop for PluginRuntime {
    /// Tear the isolate down when the handle goes away.
    fn drop(&mut self) {
        let _ = self.join_worker();
    }
}

/// Await a worker reply, bounding the wait so a wedged isolate fails fast.
///
/// A closed reply channel means the worker thread is gone; a timeout means the
/// isolate is busy (an infinite loop, an unsettled promise) past
/// [`COMMAND_TIMEOUT`].
async fn await_reply(
    response: oneshot::Receiver<Result<serde_json::Value>>,
) -> Result<serde_json::Value> {
    match tokio::time::timeout(COMMAND_TIMEOUT, response).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err(Error::RuntimeStopped),
        Err(_) => Err(Error::RuntimeTimeout),
    }
}

/// The worker thread's main loop: own the isolate, serve commands until close.
///
/// V8 is single-threaded and `JsRuntime` is `!Send`, so the isolate is built
/// and used entirely on this thread. A current-thread Tokio runtime drives the
/// V8 event loop (promise jobs, dynamic imports) when a command needs it.
///
/// Once the isolate is built, the worker reports success through `ready` and
/// hands back the isolate's [`v8::IsolateHandle`] so the owning
/// [`PluginRuntime`] can terminate a wedged isolate during teardown. A startup
/// failure is reported through the same channel instead.
fn worker_loop(
    config: RuntimeConfig,
    receiver: mpsc::Receiver<Command>,
    ready: mpsc::Sender<StartupResult>,
) {
    // A current-thread Tokio runtime: `block_on` here is what advances the V8
    // event loop and resolves the futures `deno_core` hands back.
    let tokio_rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            tracing::error!("plugin runtime worker failed to build Tokio runtime: {e}");
            // Report the failure so `PluginRuntime::new` returns a clear
            // startup error. The command receiver is dropped on return.
            let _ = ready.send(Err(format!("worker Tokio runtime unavailable: {e}")));
            return;
        }
    };

    // Cap the V8 heap. `heap_limits` alone makes V8 raise a *fatal* OOM that
    // aborts the whole host process when the cap is hit; the near-heap-limit
    // callback registered below turns that into a per-isolate failure instead.
    let create_params = v8::CreateParams::default().heap_limits(HEAP_INITIAL_BYTES, HEAP_MAX_BYTES);

    let mut runtime = JsRuntime::new(RuntimeOptions {
        // The real module loader (relative / bare / `@swissarmyhammer/*`
        // resolution) is a separate task; a plugin entry module is supplied
        // directly as source, so a no-op loader is sufficient here.
        module_loader: Some(Rc::new(NoopModuleLoader)),
        create_params: Some(create_params),
        // The SDK-to-host bridge op is installed for every plugin isolate.
        // The dispatcher starts unbound; the host binds a real one later.
        extensions: vec![bridge::host_bridge::init(
            Arc::new(UnboundHostDispatcher) as Arc<dyn HostDispatcher>
        )],
        // Inspector support is gated to dev mode via `inspect_port`.
        inspector: config.inspect_port.is_some(),
        ..Default::default()
    });

    // Keep the heap cap from aborting the host process. `heap_limits` on its
    // own makes V8 treat an exhausted heap as a fatal, process-level OOM. This
    // callback fires just before that point: it terminates JavaScript
    // execution on this isolate (the offending plugin's script unwinds with an
    // uncatchable "execution terminated" exception) and hands V8 a larger
    // limit so it has the headroom to propagate that termination cleanly
    // instead of aborting. The bumped limit is transient — execution is
    // already being torn down — so it does not defeat the cap.
    let heap_limit_handle = runtime.v8_isolate().thread_safe_handle();
    runtime.add_near_heap_limit_callback(move |current_limit, _initial_limit| {
        tracing::warn!(
            "plugin isolate approached its {HEAP_MAX_BYTES}-byte heap cap; \
             terminating the offending script"
        );
        heap_limit_handle.terminate_execution();
        // Give V8 room to unwind the termination without a fatal OOM.
        current_limit * 2
    });

    // When the inspector is enabled, log it. deno_core already constructs the
    // in-isolate inspector inside `JsRuntime::new` because `inspector: true`
    // was set above, so no explicit init call is needed here. Standing up the
    // TCP listener on `inspect_port` is the embedder's job (a later task).
    if let Some(port) = config.inspect_port {
        tracing::info!("plugin runtime inspector enabled (intended port {port})");
    }

    // Report a successful startup, handing the owner a thread-safe handle to
    // this isolate so teardown can interrupt a wedged script. If the owner is
    // already gone, there is nothing left to serve.
    if ready
        .send(Ok(runtime.v8_isolate().thread_safe_handle()))
        .is_err()
    {
        return;
    }

    // Serve commands until every sender is dropped.
    while let Ok(command) = receiver.recv() {
        match command {
            Command::EvalScript { code, reply } => {
                let result = eval_script(&mut runtime, &tokio_rt, &code);
                let _ = reply.send(result);
            }
            Command::LoadModule {
                specifier,
                source,
                lifecycle_export,
                reply,
            } => {
                let result = load_and_run_module(
                    &mut runtime,
                    &tokio_rt,
                    &specifier,
                    &source,
                    lifecycle_export.as_deref(),
                );
                let _ = reply.send(result);
            }
        }

        // If the just-finished command tripped the heap-limit callback (or any
        // other terminate), clear the isolate's termination flag so the next
        // command starts from a clean state. A heap-OOM kill is thus contained
        // to the one offending plugin call rather than poisoning the isolate.
        runtime.v8_isolate().cancel_terminate_execution();
    }

    tracing::debug!("plugin runtime worker shutting down");
}

/// Evaluate a JavaScript snippet as a classic script and return its JSON value.
fn eval_script(
    runtime: &mut JsRuntime,
    tokio_rt: &tokio::runtime::Runtime,
    code: &str,
) -> Result<serde_json::Value> {
    let global = runtime
        .execute_script("<plugin-eval>", code.to_string())
        .map_err(|e| Error::Runtime(e.exception_message.clone()))?;

    let json = global_to_json(runtime, &global)?;

    // Settle any promise jobs the snippet scheduled before returning.
    drain_event_loop(runtime, tokio_rt);

    Ok(json)
}

/// Transpile a TypeScript module, evaluate it, and optionally call an export.
///
/// The module is loaded as the isolate's main module from transpiled source.
/// When `lifecycle_export` is `Some`, the named export is fetched from the
/// module namespace and called; its return value (awaited if it is a promise)
/// is converted to JSON. When it is `None`, the function returns JSON `null`.
fn load_and_run_module(
    runtime: &mut JsRuntime,
    tokio_rt: &tokio::runtime::Runtime,
    specifier: &str,
    source: &str,
    lifecycle_export: Option<&str>,
) -> Result<serde_json::Value> {
    let module_specifier = ModuleSpecifier::parse(specifier)
        .map_err(|e| Error::Runtime(format!("invalid module specifier '{specifier}': {e}")))?;

    // Transpile TypeScript to JavaScript. This is syntactic only — a type
    // error in `source` does not fail here.
    let transpiled = transpile::transpile_typescript(&module_specifier, source)?;

    // Load and evaluate the module on the worker's Tokio runtime, then run the
    // event loop so module-level promise jobs settle.
    let module_id = tokio_rt
        .block_on(runtime.load_main_es_module_from_code(&module_specifier, transpiled.code))
        .map_err(|e| Error::Runtime(format!("failed to load module: {e}")))?;

    let evaluation = runtime.mod_evaluate(module_id);
    tokio_rt
        .block_on(runtime.run_event_loop(PollEventLoopOptions::default()))
        .map_err(|e| Error::Runtime(format!("module event loop error: {e}")))?;
    tokio_rt
        .block_on(evaluation)
        .map_err(|e| Error::Runtime(format!("module evaluation failed: {e}")))?;

    let Some(export) = lifecycle_export else {
        return Ok(serde_json::Value::Null);
    };

    call_module_export(runtime, tokio_rt, module_id, export)
}

/// Fetch an exported function from an evaluated module and call it.
fn call_module_export(
    runtime: &mut JsRuntime,
    tokio_rt: &tokio::runtime::Runtime,
    module_id: deno_core::ModuleId,
    export: &str,
) -> Result<serde_json::Value> {
    let namespace = runtime
        .get_module_namespace(module_id)
        .map_err(|e| Error::Runtime(format!("cannot read module exports: {e}")))?;

    // Pull the named export out of the namespace object as a global function
    // handle. The scope is dropped before the async call below.
    let function = {
        deno_core::scope!(scope, runtime);
        let namespace = v8::Local::new(scope, &namespace);
        let key = v8::String::new(scope, export)
            .ok_or_else(|| Error::Runtime(format!("cannot allocate export name '{export}'")))?;
        let value = namespace
            .get(scope, key.into())
            .ok_or_else(|| Error::Runtime(format!("module has no export named '{export}'")))?;
        let function = v8::Local::<v8::Function>::try_from(value)
            .map_err(|_| Error::Runtime(format!("export '{export}' is not a function")))?;
        v8::Global::new(scope, function)
    };

    // Call the function, running the event loop so a returned promise settles.
    let call = runtime.call_with_args(&function, &[]);
    let result = tokio_rt
        .block_on(runtime.with_event_loop_promise(call, PollEventLoopOptions::default()))
        .map_err(|e| Error::Runtime(format!("lifecycle function '{export}' failed: {e}")))?;

    global_to_json(runtime, &result)
}

/// Convert a V8 value handle to a `serde_json::Value`.
///
/// `undefined`, `null`, and functions map to JSON `null`; every other value
/// round-trips through the engine's own `JSON.stringify`. A `JSON.stringify`
/// that throws (a symbol, a circular structure) also collapses to `null`
/// rather than poisoning the isolate.
fn global_to_json(
    runtime: &mut JsRuntime,
    value: &v8::Global<v8::Value>,
) -> Result<serde_json::Value> {
    deno_core::scope!(scope, runtime);
    let local = v8::Local::new(scope, value);

    if local.is_undefined() || local.is_null() || local.is_function() {
        return Ok(serde_json::Value::Null);
    }

    let rust_string = {
        v8::tc_scope!(let tc, scope);
        match v8::json::stringify(tc, local) {
            Some(s) => s.to_rust_string_lossy(tc),
            None => return Ok(serde_json::Value::Null),
        }
    };

    if rust_string.is_empty() || rust_string == "undefined" {
        return Ok(serde_json::Value::Null);
    }

    serde_json::from_str(&rust_string)
        .map_err(|e| Error::Runtime(format!("cannot convert result to JSON: {e}")))
}

/// Drain pending promise jobs, microtasks, and dynamic imports.
///
/// `deno_core` advances promise resolution only while the event loop is
/// polled. Running it to completion lets a script's promise side effects
/// settle before the result is returned.
fn drain_event_loop(runtime: &mut JsRuntime, tokio_rt: &tokio::runtime::Runtime) {
    if let Err(e) = tokio_rt.block_on(runtime.run_event_loop(PollEventLoopOptions::default())) {
        tracing::warn!("error draining plugin event loop: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `.ts` snippet with a type annotation and an exported function. The
    /// function, when called, returns a value the test asserts on.
    #[tokio::test]
    async fn transpiles_and_runs_typescript_module() {
        let runtime = PluginRuntime::new(RuntimeConfig::default()).expect("runtime should start");

        // `count: number` is TypeScript-only syntax; the export returns a
        // value so the test can observe that the module actually ran.
        let ts =
            "export function activate(): number { const count: number = 7; return count * 6; }";
        let result = runtime
            .call_lifecycle("file:///plugin/a.ts", ts, "activate")
            .await
            .expect("lifecycle call should succeed");

        assert_eq!(result, serde_json::json!(42));
    }

    /// Two isolates are independent: a global set in one is invisible in the
    /// other.
    #[tokio::test]
    async fn isolates_are_independent() {
        let first = PluginRuntime::new(RuntimeConfig::default()).expect("first runtime");
        let second = PluginRuntime::new(RuntimeConfig::default()).expect("second runtime");

        // Set a global only in the first isolate.
        first
            .eval("globalThis.shared_marker = 'first-only'; globalThis.shared_marker")
            .await
            .expect("first eval should succeed");

        // The first isolate sees its own global.
        let in_first = first
            .eval("typeof globalThis.shared_marker")
            .await
            .expect("first readback should succeed");
        assert_eq!(in_first, serde_json::json!("string"));

        // The second isolate must NOT see it — separate V8 isolate, separate
        // global object.
        let in_second = second
            .eval("typeof globalThis.shared_marker")
            .await
            .expect("second readback should succeed");
        assert_eq!(in_second, serde_json::json!("undefined"));
    }

    /// A type-incorrect but syntactically valid `.ts` still transpiles and
    /// runs: transpilation does not type-check.
    #[tokio::test]
    async fn type_incorrect_typescript_still_runs() {
        let runtime = PluginRuntime::new(RuntimeConfig::default()).expect("runtime should start");

        // `const n: number = "not a number"` is a type ERROR but is
        // syntactically valid TypeScript. After type erasure it is just
        // `const n = "not a number"`, which runs fine. The function returns
        // the string's length to prove the module executed.
        let ts =
            "export function activate(): number { const n: number = \"hello\"; return n.length; }";
        let result = runtime
            .call_lifecycle("file:///plugin/bad-types.ts", ts, "activate")
            .await
            .expect("type-incorrect TS should still transpile and run");

        assert_eq!(result, serde_json::json!(5));
    }

    /// The runtime can evaluate a plain module without a lifecycle export.
    #[tokio::test]
    async fn loads_module_without_lifecycle_export() {
        let runtime = PluginRuntime::new(RuntimeConfig::default()).expect("runtime should start");

        let ts = "const greeting: string = 'hi'; globalThis.loaded_marker = greeting;";
        runtime
            .load_module("file:///plugin/side-effect.ts", ts)
            .await
            .expect("module should load");

        let marker = runtime
            .eval("globalThis.loaded_marker")
            .await
            .expect("readback should succeed");
        assert_eq!(marker, serde_json::json!("hi"));
    }

    /// A TypeScript syntax error fails the load with a transpile error.
    #[tokio::test]
    async fn syntax_error_fails_to_load() {
        let runtime = PluginRuntime::new(RuntimeConfig::default()).expect("runtime should start");

        let result = runtime
            .load_module("file:///plugin/broken.ts", "export function broken( {")
            .await;
        assert!(
            matches!(result, Err(Error::Transpile(_))),
            "a syntax error should surface as Error::Transpile, got: {result:?}"
        );
    }

    /// A runtime exception thrown by a lifecycle function is surfaced.
    #[tokio::test]
    async fn lifecycle_exception_is_reported() {
        let runtime = PluginRuntime::new(RuntimeConfig::default()).expect("runtime should start");

        let ts = "export function activate(): void { throw new Error('boom'); }";
        let result = runtime
            .call_lifecycle("file:///plugin/throws.ts", ts, "activate")
            .await;
        assert!(
            matches!(result, Err(Error::Runtime(_))),
            "a thrown exception should surface as Error::Runtime, got: {result:?}"
        );
    }

    /// Asking for an export that does not exist is an error.
    #[tokio::test]
    async fn missing_lifecycle_export_is_reported() {
        let runtime = PluginRuntime::new(RuntimeConfig::default()).expect("runtime should start");

        let ts = "export function activate(): number { return 1; }";
        let result = runtime
            .call_lifecycle("file:///plugin/c.ts", ts, "deactivate")
            .await;
        assert!(
            matches!(result, Err(Error::Runtime(_))),
            "a missing export should surface as Error::Runtime, got: {result:?}"
        );
    }

    /// An explicit shutdown joins the worker thread cleanly.
    #[tokio::test]
    async fn explicit_shutdown_is_clean() {
        let runtime = PluginRuntime::new(RuntimeConfig::default()).expect("runtime should start");
        runtime
            .eval("1 + 1")
            .await
            .expect("eval before shutdown should succeed");
        runtime.shutdown().expect("shutdown should be clean");
    }

    /// A runtime created with an inspector port still loads and runs modules.
    #[tokio::test]
    async fn inspector_enabled_runtime_runs_modules() {
        let config = RuntimeConfig {
            inspect_port: Some(9229),
        };
        let runtime = PluginRuntime::new(config).expect("inspector runtime should start");

        let ts = "export function activate(): number { return 99; }";
        let result = runtime
            .call_lifecycle("file:///plugin/inspect.ts", ts, "activate")
            .await
            .expect("module should run with inspector enabled");
        assert_eq!(result, serde_json::json!(99));
    }

    /// Tearing down a runtime whose isolate is wedged in a non-terminating
    /// script must not hang.
    ///
    /// The worker is handed a `while (true) {}` script and left running. With
    /// no way to interrupt the isolate, `Drop`'s `join` would block forever
    /// (the worker never returns to `recv`). Because `PluginRuntime` retains
    /// the isolate's `v8::IsolateHandle` and `terminate_execution`s it before
    /// joining, the wedged script unwinds and the worker exits promptly — so
    /// this test completes well within its bounded timeout.
    #[tokio::test]
    async fn teardown_does_not_hang_on_wedged_isolate() {
        // Spawning the runtime on a blocking thread keeps the test's async
        // executor free; the runtime itself owns its own worker thread.
        let runtime = PluginRuntime::new(RuntimeConfig::default()).expect("runtime should start");

        // Fire a non-terminating script at the worker. `eval` will never
        // answer (the worker is stuck in the loop), so the reply is dropped on
        // the floor — the point is only to wedge the isolate.
        let runtime = std::sync::Arc::new(runtime);
        let wedge = {
            let runtime = std::sync::Arc::clone(&runtime);
            tokio::spawn(async move {
                let _ = runtime.eval("while (true) {}").await;
            })
        };

        // Give the worker time to actually enter the infinite loop.
        tokio::time::sleep(Duration::from_millis(250)).await;

        // Drop the background task's handle on the runtime so the only
        // remaining owner is this test, then tear the runtime down on a
        // blocking thread and assert the teardown finishes quickly.
        wedge.abort();
        let _ = wedge.await;

        let teardown = tokio::task::spawn_blocking(move || {
            // Dropping the last `Arc` runs `PluginRuntime::drop`, which
            // terminates the wedged isolate and joins the worker.
            drop(runtime);
        });

        tokio::time::timeout(Duration::from_secs(10), teardown)
            .await
            .expect("teardown must not hang on a wedged isolate")
            .expect("teardown task should not panic");
    }

    /// A plugin that allocates past the heap cap has its script terminated
    /// rather than aborting the host process.
    ///
    /// The near-heap-limit callback registered on the isolate fires as the
    /// plugin nears [`HEAP_MAX_BYTES`], terminates execution, and bumps the
    /// limit so V8 can unwind cleanly. The runaway allocation therefore
    /// surfaces as an [`Error::Runtime`] (or a timeout) — and crucially the
    /// test process survives to make this assertion at all.
    #[tokio::test]
    async fn heap_exhaustion_terminates_script_not_host() {
        let runtime = PluginRuntime::new(RuntimeConfig::default()).expect("runtime should start");

        // Grow an array without bound. This blows past the 64 MiB cap; the
        // heap-limit callback must terminate it instead of letting V8 abort
        // the whole process.
        let result = runtime
            .eval("const sink = []; while (true) { sink.push(new Array(100000).fill(7)); }")
            .await;

        assert!(
            matches!(result, Err(Error::Runtime(_)) | Err(Error::RuntimeTimeout)),
            "a runaway allocation should surface as a runtime error, got: {result:?}"
        );

        // The isolate's termination flag is cleared between commands, so the
        // runtime is still usable for a subsequent, well-behaved script.
        let after = runtime
            .eval("1 + 1")
            .await
            .expect("runtime should still serve commands after a contained OOM");
        assert_eq!(after, serde_json::json!(2));
    }
}
