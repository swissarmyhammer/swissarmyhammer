//! The plugin host: load and unload plugins, route every call, dispose
//! cleanly.
//!
//! [`PluginHost`] is the top-level object that ties the platform together. It
//! owns the live [`ServerRegistry`], the per-plugin [`PluginLedger`], a table
//! of host-exposed Rust modules, and one [`PluginRuntime`] isolate per loaded
//! plugin. It has three jobs:
//!
//! - **Lifecycle** — [`load`](PluginHost::load) creates a fresh isolate for a
//!   plugin, evaluates its entry module, and runs its `load()`;
//!   [`unload`](PluginHost::unload) disposes everything the plugin registered
//!   and tears the isolate down.
//! - **Bridging** — each loaded plugin's SDK transport calls back into the host
//!   over the runtime's bridge op. The host installs a [`HostBridge`]
//!   dispatcher that answers the SDK's wire envelopes (`toolsList`,
//!   `toolsCall`, `register`, `unregister`, `log`) and routes them at the live
//!   registry, attributing every call to the originating plugin.
//! - **Disposal without cooperation** — every long-lived registration a plugin
//!   makes is recorded in its [`PluginLedger`] vec. Unload drains that vec in
//!   reverse and disposes each handle, so the platform reclaims a plugin's
//!   state whether or not the plugin's own `unload()` does anything.
//!
//! # Threading
//!
//! Host state — the registry, the ledger, the module table, the loaded-plugin
//! map — lives behind a single [`Mutex`] inside a shared [`HostInner`]. Every
//! host operation and every bridge call locks that mutex only for the brief,
//! synchronous span it needs (a registry mutation, a ledger append, cloning a
//! server handle) and **never** holds it across an `.await`. A plugin's
//! `load()` runs on that plugin's own isolate worker thread; the bridge calls
//! it makes are serviced concurrently because each is its own async task that
//! takes the lock independently.
//!
//! # Host-agnostic by construction
//!
//! `PluginHost` hardcodes no global configuration and no host-specific
//! directories. Its constructors take the writable layer roots explicitly:
//! [`for_tests`](PluginHost::for_tests) for tests and
//! [`new`](PluginHost::new) for production embedders, each of which computes
//! its own roots and passes them in.
//!
//! Scope: this module delivers explicit [`load`](PluginHost::load) /
//! [`unload`](PluginHost::unload) APIs. Discovering plugins by scanning the
//! layer roots, and triggering reloads from a filesystem watcher, are separate
//! later tasks.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Map, Value};

use crate::error::{Error, Result};
use crate::ledger::{CallbackId, PluginLedger, RegistrationHandle};
use crate::registry::{ServerName, ServerRegistry, ServerStatus};
use crate::runtime::{HostDispatcher, PluginRuntime, RuntimeConfig};
use crate::server::{CallerId, CliServer, McpServer, PluginId, ToolMetadata, UrlServer};

/// The filename of a plugin bundle's entry module.
///
/// A plugin bundle is a directory; its entry TypeScript module is always named
/// `entry.ts`. The host loads exactly this file as the plugin's main module.
const ENTRY_FILE: &str = "entry.ts";

/// The exported lifecycle function the host calls to load a plugin.
///
/// A plugin bundle's `entry.ts` exports a `load` function that constructs the
/// `Plugin` subclass — wrapped in the SDK's dispatch Proxy — and awaits its
/// `load()` hook. The host invokes this export after evaluating the module.
const LOAD_EXPORT: &str = "load";

/// The exported lifecycle function the host calls to unload a plugin.
///
/// A plugin bundle's `entry.ts` exports an `unload` function only when the
/// author wrote one. The host calls it best-effort before disposing the
/// plugin's registrations; a bundle that does not export it is the normal
/// case, not an error.
const UNLOAD_EXPORT: &str = "unload";

/// How long the bridge waits for the host to answer one SDK call.
///
/// A plugin's bridge call crosses from the isolate worker thread to a host
/// async task and back. A bounded wait turns a host that never answers — a
/// dropped channel, a wedged task — into a prompt error instead of a hung
/// isolate worker.
///
/// This is deliberately longer than the runtime's `COMMAND_TIMEOUT` (30s): a
/// bridge call can itself trigger a runtime command, so the two timers can
/// race. Giving the bridge the longer budget lets the inner command timeout
/// win deterministically — the call surfaces the precise
/// [`Error::RuntimeTimeout`] rather than a nondeterministic mix of the inner
/// timeout and the outer "host did not answer" error.
const BRIDGE_TIMEOUT: Duration = Duration::from_secs(35);

/// The top-level plugin host.
///
/// Owns the live [`ServerRegistry`], the per-plugin [`PluginLedger`], the table
/// of host-exposed Rust modules, and one [`PluginRuntime`] per loaded plugin.
/// See the [module documentation](self) for the threading model and the
/// host-agnostic constructor contract.
///
/// A `PluginHost` is cheap to clone: the clone shares the same underlying host
/// state, so two clones see one registry and one set of loaded plugins.
#[derive(Clone)]
pub struct PluginHost {
    /// The shared host state, behind one mutex.
    inner: Arc<HostInner>,
}

/// `Debug` is written by hand because [`HostInner`] holds trait objects
/// (`Arc<dyn McpServer>`, `PluginRuntime`) that carry no `Debug` bound. The
/// impl reports the host's writable layer roots and how many plugins are
/// loaded — the meaningful, printable state.
impl std::fmt::Debug for PluginHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = self.inner.state.lock().expect("host state mutex poisoned");
        f.debug_struct("PluginHost")
            .field("user_root", &self.inner.user_root)
            .field("project_root", &self.inner.project_root)
            .field("loaded_plugins", &state.plugins.len())
            .finish()
    }
}

/// The host's shared, internally-mutable state.
///
/// Held behind an [`Arc`] so a [`PluginHost`] is cheaply clonable, and the
/// mutable part behind a [`Mutex`] so concurrent host operations and bridge
/// calls serialize. The layer roots are immutable after construction and so sit
/// outside the mutex.
struct HostInner {
    /// The writable user-layer plugin root supplied at construction.
    user_root: PathBuf,

    /// The writable project-layer plugin root, when the embedder has one.
    project_root: Option<PathBuf>,

    /// Source of stable, per-host-unique plugin ids.
    next_plugin_seq: AtomicU64,

    /// The mutable host state guarded by one mutex.
    state: Mutex<HostState>,
}

/// The mutable core of the host, guarded by [`HostInner`]'s mutex.
struct HostState {
    /// The live registry of MCP servers calls are routed against.
    registry: ServerRegistry,

    /// The per-plugin ledger of long-lived registrations.
    ledger: PluginLedger,

    /// Rust modules exposed by the host but not yet activated under a name.
    ///
    /// A `register({ rust: id })` from a plugin moves the matching module out
    /// of this table and into the live [`registry`](Self::registry) under the
    /// plugin's chosen name. This decouples compiled-in Rust code from which
    /// servers are live and under what names.
    modules: HashMap<String, Arc<dyn McpServer>>,

    /// The loaded plugins, keyed by plugin id.
    plugins: HashMap<PluginId, LoadedPlugin>,
}

/// A plugin the host has loaded: its isolate plus where its bundle lives.
///
/// The bundle directory is retained so [`PluginHost::unload`] can re-reach the
/// bundle's optional `unload` export before the host disposes the plugin's
/// registrations and tears the isolate down.
struct LoadedPlugin {
    /// The plugin's V8 isolate, on its own worker thread.
    runtime: PluginRuntime,

    /// The plugin's bundle directory, as passed to [`PluginHost::load`].
    bundle_dir: PathBuf,
}

impl PluginHost {
    /// Creates a host for tests with explicit plugin-layer roots.
    ///
    /// The platform stays host-agnostic: a test supplies the roots it wants
    /// rather than the host reaching for global configuration.
    ///
    /// # Parameters
    ///
    /// - `user_root` — the writable user-layer plugin directory.
    /// - `project_root` — the writable project-layer plugin directory, or
    ///   `None` when the test models a host with no project layer.
    pub fn for_tests(user_root: PathBuf, project_root: Option<PathBuf>) -> Self {
        Self::with_roots(user_root, project_root)
    }

    /// Creates a production host from the builtin plugin set and the writable
    /// layer roots.
    ///
    /// The embedder — the kanban app, the CLI, the TUI — computes its own
    /// directories and passes them in; the platform hardcodes none. Each
    /// builtin plugin directory is loaded immediately, in the given order, so a
    /// host built with `new` comes up with its builtins already running.
    ///
    /// # Parameters
    ///
    /// - `builtins` — directories of plugins shipped with the embedder, loaded
    ///   at construction in order.
    /// - `user_root` — the writable user-layer plugin directory.
    /// - `project_root` — the writable project-layer plugin directory, when the
    ///   embedder has a project layer.
    ///
    /// # Errors
    ///
    /// Returns the first error encountered loading a builtin plugin. A host
    /// whose builtins fail to load is not returned, because a partially
    /// initialized host would silently miss tools the embedder shipped.
    pub async fn new(
        builtins: Vec<PathBuf>,
        user_root: PathBuf,
        project_root: Option<PathBuf>,
    ) -> Result<Self> {
        let host = Self::with_roots(user_root, project_root);
        for builtin in builtins {
            host.load(&builtin).await?;
        }
        Ok(host)
    }

    /// Builds a host with the given roots and empty state.
    fn with_roots(user_root: PathBuf, project_root: Option<PathBuf>) -> Self {
        Self {
            inner: Arc::new(HostInner {
                user_root,
                project_root,
                next_plugin_seq: AtomicU64::new(0),
                state: Mutex::new(HostState {
                    registry: ServerRegistry::new(),
                    ledger: PluginLedger::new(),
                    modules: HashMap::new(),
                    plugins: HashMap::new(),
                }),
            }),
        }
    }

    /// The writable user-layer plugin root this host was given.
    pub fn user_root(&self) -> &Path {
        &self.inner.user_root
    }

    /// The writable project-layer plugin root, if the host has one.
    pub fn project_root(&self) -> Option<&Path> {
        self.inner.project_root.as_deref()
    }

    /// Exposes a Rust [`McpServer`] under `id` in the available-modules table.
    ///
    /// This does **not** make the server live: it records the server in a
    /// separate table keyed by `id`, from which a plugin's
    /// `register(name, { rust: id })` can later activate it under a name of the
    /// plugin's choosing. Decoupling exposure from activation lets compiled-in
    /// Rust code be offered to plugins without dictating whether — or under what
    /// name — it is registered.
    ///
    /// # Parameters
    ///
    /// - `id` — the module id a plugin addresses with `{ rust: id }`.
    /// - `server` — the in-process server to expose.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ServerNameTaken`] — carrying `id` — when a module is
    /// already exposed under that id.
    pub async fn expose_rust_module(
        &self,
        id: impl Into<String>,
        server: Arc<dyn McpServer>,
    ) -> Result<()> {
        let id = id.into();
        let mut state = self.lock();
        if state.modules.contains_key(&id) {
            return Err(Error::ServerNameTaken(id));
        }
        state.modules.insert(id, server);
        Ok(())
    }

    /// Loads the plugin whose bundle is the directory `plugin_dir`.
    ///
    /// Creates a fresh [`PluginRuntime`] isolate for the plugin, wires its SDK
    /// bridge to a host dispatcher scoped to the new plugin's id, loads the
    /// bundle's `entry.ts` through the module loader, and runs the exported
    /// `load` lifecycle function. Every server the plugin registers during
    /// `load()` is inserted into the live registry and recorded in the plugin's
    /// ledger.
    ///
    /// # Parameters
    ///
    /// - `plugin_dir` — the plugin's bundle directory; it must contain an
    ///   `entry.ts` entry module.
    ///
    /// # Returns
    ///
    /// The [`PluginId`] the host assigned the freshly loaded plugin.
    ///
    /// # Errors
    ///
    /// Returns [`Error::RuntimeStartup`] when the isolate cannot be created,
    /// [`Error::Transpile`] or [`Error::Runtime`] when the bundle fails to load
    /// or its `load()` throws, or any error a `register` made during `load()`
    /// surfaced. A plugin that fails to load is removed from the host, so a
    /// failed load leaves no half-initialized plugin behind.
    pub async fn load(&self, plugin_dir: impl AsRef<Path>) -> Result<PluginId> {
        let plugin_dir = plugin_dir.as_ref().to_path_buf();
        let plugin_id = self.mint_plugin_id();

        // The bridge dispatcher is scoped to this plugin's id: every call it
        // forwards is attributed to `CallerId::Plugin(plugin_id)`.
        let bridge = Arc::new(HostBridge::new(self.clone(), plugin_id.clone()));
        let runtime = PluginRuntime::new(RuntimeConfig {
            dispatcher: Some(bridge),
            ..Default::default()
        })?;

        // Track the plugin before running its `load()` so the `register` calls
        // that `load()` makes — which arrive over the bridge while the call
        // below is awaiting — have a ledger vec to append to.
        self.lock().ledger.track(plugin_id.clone());

        // Drive the plugin's lifecycle on its own isolate. `PluginRuntime` is
        // not `Sync`, so the handle is held in this local — never across the
        // host mutex — while `load()` runs.
        let load_result = runtime
            .call_plugin_lifecycle(&plugin_dir, ENTRY_FILE, LOAD_EXPORT)
            .await;

        match load_result {
            Ok(_) => {
                // The plugin is fully loaded: retain its isolate and bundle dir
                // so a later `unload` can reach its `unload` export and tear
                // the isolate down.
                self.lock().plugins.insert(
                    plugin_id.clone(),
                    LoadedPlugin {
                        runtime,
                        bundle_dir: plugin_dir,
                    },
                );
                Ok(plugin_id)
            }
            Err(error) => {
                // A failed load must not leave a half-initialized plugin: undo
                // every registration it managed to make. The isolate is still
                // alive here — it is torn down as `runtime` drops at the end of
                // this scope — so callback handles can be disposed on it.
                self.dispose_registrations(&plugin_id, &runtime).await;
                Err(error)
            }
        }
    }

    /// Unloads the plugin identified by `plugin_id`.
    ///
    /// Disposal does not need the plugin's cooperation. The plugin's optional
    /// `unload()` hook is invoked first — best-effort, only for external side
    /// effects it may want to perform — and then the host drains the plugin's
    /// ledger vec **in reverse** and disposes every handle: registered servers
    /// are unregistered from the live registry, callbacks are dropped, and
    /// opaque dispose functions are run. Finally the plugin's isolate is torn
    /// down.
    ///
    /// # Parameters
    ///
    /// - `plugin_id` — the id of the plugin to unload.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnknownPlugin`] when no plugin is loaded under
    /// `plugin_id` — a stale or never-loaded id. A failure of the plugin's own
    /// `unload()` hook is logged and does not abort disposal — the host
    /// reclaims the plugin's state regardless.
    pub async fn unload(&self, plugin_id: &PluginId) -> Result<()> {
        // Take the loaded plugin out of the host. `PluginRuntime` is not
        // `Sync`, so it is held in this local — never across the host mutex —
        // while the optional `unload()` hook runs.
        let Some(plugin) = self.lock().plugins.remove(plugin_id) else {
            return Err(Error::UnknownPlugin);
        };

        // Best-effort: give the plugin a chance to run its own `unload()` for
        // external side effects before the host disposes its registrations.
        self.run_plugin_unload(plugin_id, &plugin).await;

        // Authoritative cleanup: undo every registration the plugin made. The
        // isolate is still alive, so callback handles can be disposed on it
        // before it is torn down.
        self.dispose_registrations(plugin_id, &plugin.runtime).await;

        // Dropping the runtime tears the isolate's worker thread down.
        drop(plugin);
        Ok(())
    }

    /// Runs the plugin's optional `unload` lifecycle hook, ignoring failures.
    ///
    /// The bundle's `entry.ts` exports an `unload` function only when the
    /// plugin author wrote one. A bundle with no such export surfaces as an
    /// [`Error::Runtime`] from `call_plugin_lifecycle`, which is the expected
    /// case and is logged at debug level rather than failing the unload —
    /// `unload()` is optional by contract.
    async fn run_plugin_unload(&self, plugin_id: &PluginId, plugin: &LoadedPlugin) {
        let result = plugin
            .runtime
            .call_plugin_lifecycle(&plugin.bundle_dir, ENTRY_FILE, UNLOAD_EXPORT)
            .await;
        if let Err(error) = result {
            tracing::debug!(
                plugin = %plugin_id.as_str(),
                %error,
                "plugin unload() hook absent or failed; continuing host-side disposal"
            );
        }
    }

    /// Disposes every registration a plugin made, draining its ledger.
    ///
    /// Drains the plugin's ledger vec in reverse and disposes each handle —
    /// the host's authoritative cleanup, run whether or not the plugin's own
    /// `unload()` did anything. A [`RegistrationHandle::Callback`] is disposed
    /// against the still-alive `runtime`, so the stored function is dropped
    /// from the isolate's callback table. Tearing the isolate down is the
    /// caller's job; this method only undoes the host-side and isolate-side
    /// registration state.
    async fn dispose_registrations(&self, plugin_id: &PluginId, runtime: &PluginRuntime) {
        let handles = self.lock().ledger.drain(plugin_id).unwrap_or_default();

        // Handles are already reversed by `PluginLedger::drain`: the last
        // registration is disposed first.
        for handle in handles {
            self.dispose_handle(plugin_id, runtime, handle).await;
        }
    }

    /// Disposes one [`RegistrationHandle`] drained from a plugin's ledger.
    ///
    /// `runtime` is the disposed plugin's still-alive isolate: a `Callback`
    /// handle is disposed by driving the isolate to drop the stored function.
    async fn dispose_handle(
        &self,
        plugin_id: &PluginId,
        runtime: &PluginRuntime,
        handle: RegistrationHandle,
    ) {
        match handle {
            RegistrationHandle::Server(name) => {
                let removed = self.lock().registry.unregister(&name);
                if removed.is_none() {
                    tracing::debug!(
                        plugin = %plugin_id.as_str(),
                        server = %name,
                        "ledger server handle had no live registration to dispose"
                    );
                }
            }
            RegistrationHandle::Callback(id) => {
                // Drop the stored function from the isolate's callback table so
                // the id no longer resolves. A failure is logged, not fatal —
                // the isolate is torn down right after this regardless.
                if let Err(error) = runtime.dispose_callback(id.as_str()).await {
                    tracing::debug!(
                        plugin = %plugin_id.as_str(),
                        callback = %id.as_str(),
                        %error,
                        "disposing a callback handle on the isolate failed"
                    );
                }
            }
            RegistrationHandle::Opaque(dispose) => dispose(),
        }
    }

    /// Routes a call to a registered server through the live registry.
    ///
    /// This is the host's own dispatch entry point — the path host code (and
    /// the tests) use to call a registered server directly. A plugin's calls
    /// reach the same registry through the SDK bridge instead, and both share
    /// the [`route`](Self::route) helper. Routing is by `(server, tool)`;
    /// `input` is forwarded verbatim.
    ///
    /// # Parameters
    ///
    /// - `caller` — who issued the call, threaded through to the server.
    /// - `server` — the registry name of the server to route to.
    /// - `tool` — the tool to invoke on that server.
    /// - `input` — the `tools/call` arguments, forwarded verbatim.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ServerUnavailable`] when `server` was registered and
    /// has since been disposed, [`Error::UnknownServer`] when `server` was
    /// never registered, or any error the target server's `invoke` produces.
    pub async fn call(
        &self,
        caller: CallerId,
        server: &str,
        tool: &str,
        input: Value,
    ) -> Result<Value> {
        self.route(caller, server, tool, input).await
    }

    /// Resolves `server` against the live registry and invokes `tool` on it.
    ///
    /// This is the single routing primitive every call in the host flows
    /// through — `PluginHost::call` for host code and the SDK bridge's
    /// `toolsCall` for plugins. It is the same `(server, tool)` resolution a
    /// [`Dispatcher`](crate::dispatcher::Dispatcher) performs: the bare
    /// `Dispatcher` type cannot be reused as the host's live router because it
    /// holds an immutable `Arc<ServerRegistry>`, whereas the host's registry is
    /// mutated in place as plugins register and unregister servers. The server
    /// handle is cloned out under the lock, which is then released before the
    /// `invoke` await so a concurrent registration is never blocked.
    ///
    /// Routing consults the registry's tombstones so the failure modes stay
    /// distinguishable: a name disposed out from under the caller fails with
    /// [`Error::ServerUnavailable`], while a name that was never registered
    /// fails with [`Error::UnknownServer`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::ServerUnavailable`] when `server` was registered and
    /// has since been disposed, [`Error::UnknownServer`] when `server` was
    /// never registered, or any error the target server's `invoke` produces.
    async fn route(
        &self,
        caller: CallerId,
        server: &str,
        tool: &str,
        input: Value,
    ) -> Result<Value> {
        // Resolve and clone the handle out under the lock, then release it
        // before the `invoke` await so a concurrent registration is not
        // blocked.
        let target = match self.lock().registry.resolve(server) {
            ServerStatus::Live(target) => target,
            ServerStatus::Disposed => return Err(Error::ServerUnavailable),
            ServerStatus::Unknown => return Err(Error::UnknownServer),
        };
        target.invoke(caller, tool, input).await
    }

    /// Returns the number of ledger handles recorded for a plugin.
    ///
    /// `None` when no plugin is loaded under `plugin_id`. A loaded plugin that
    /// has registered nothing returns `Some(0)`, distinguishing it from an
    /// unloaded plugin.
    ///
    /// # Parameters
    ///
    /// - `plugin_id` — the id of the plugin to inspect.
    pub async fn ledger_len(&self, plugin_id: &PluginId) -> Option<usize> {
        self.lock().ledger.len(plugin_id)
    }

    /// Mints a fresh, host-unique [`PluginId`].
    fn mint_plugin_id(&self) -> PluginId {
        let seq = self.inner.next_plugin_seq.fetch_add(1, Ordering::Relaxed);
        PluginId::new(format!("plugin-{seq}"))
    }

    /// Locks the host's mutable state.
    fn lock(&self) -> std::sync::MutexGuard<'_, HostState> {
        self.inner.state.lock().expect("host state mutex poisoned")
    }
}

/// The host-side dispatcher wired to one plugin's SDK transport.
///
/// One `HostBridge` is created per loaded plugin and installed as the
/// [`HostDispatcher`] of that plugin's isolate. It answers the JSON wire
/// envelopes the SDK's `HostBridge` transport emits over `op_host_dispatch` and
/// routes them at the host, attributing every routed call to the plugin it is
/// scoped to.
///
/// The `dispatch` method runs synchronously on the plugin's isolate worker
/// thread. Calls that need async work — a `tools/call`, connecting a `cli` or
/// `url` server — are sent to a fresh task on the host's runtime and the worker
/// blocks, with a [`BRIDGE_TIMEOUT`] bound, for the reply.
struct HostBridge {
    /// The host this bridge routes calls into.
    host: PluginHost,

    /// The id of the plugin this bridge is scoped to.
    plugin_id: PluginId,
}

impl HostBridge {
    /// Creates a bridge that routes `plugin_id`'s calls into `host`.
    fn new(host: PluginHost, plugin_id: PluginId) -> Self {
        Self { host, plugin_id }
    }

    /// Runs an async host operation to completion from the sync bridge op.
    ///
    /// The bridge op runs on the plugin's isolate worker thread, which has no
    /// async context of its own. The operation is spawned on a fresh
    /// current-thread Tokio runtime on a scratch thread and joined with a
    /// [`BRIDGE_TIMEOUT`] bound, so a host that never answers fails the call
    /// rather than wedging the isolate worker.
    fn block_on<F, T>(future: F) -> std::result::Result<T, String>
    where
        F: std::future::Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        let (tx, rx) = std::sync::mpsc::channel();
        // A scratch thread with its own runtime: the isolate worker's runtime
        // is already inside a `block_on`, so the host work cannot nest there.
        let join = std::thread::Builder::new()
            .name("plugin-host-bridge".to_string())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        let _ = tx.send(Err(format!("bridge runtime unavailable: {error}")));
                        return;
                    }
                };
                let _ = tx.send(Ok(runtime.block_on(future)));
            })
            .map_err(|error| format!("could not start bridge worker: {error}"))?;

        let outcome = rx
            .recv_timeout(BRIDGE_TIMEOUT)
            .map_err(|_| "host did not answer the plugin's bridge call in time".to_string());
        let _ = join.join();
        outcome?
    }

    /// Handles a `toolsList` envelope: lists a registered server's tools.
    fn tools_list(&self, payload: &Value) -> std::result::Result<Value, String> {
        let server = envelope_str(payload, "server")?;
        let host = self.host.clone();
        let tools = Self::block_on(async move {
            host.lock()
                .registry
                .get(&server)
                .map(|server| server.tools())
        })?;
        let tools = tools.ok_or_else(|| "unknown server".to_string())?;
        Ok(Value::Object(Map::from_iter([(
            "tools".to_string(),
            tools_to_json(&tools),
        )])))
    }

    /// Handles a `toolsCall` envelope: routes a `tools/call` at the registry.
    fn tools_call(&self, payload: &Value) -> std::result::Result<Value, String> {
        let server = envelope_str(payload, "server")?;
        let tool = envelope_str(payload, "tool")?;
        let arguments = payload
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| Value::Object(Map::new()));

        let host = self.host.clone();
        let caller = CallerId::Plugin(self.plugin_id.clone());
        // The call is routed at the live registry and attributed to the plugin
        // this bridge is scoped to.
        Self::block_on(async move { host.route(caller, &server, &tool, arguments).await })?
            .map_err(|error| error.to_string())
    }

    /// Handles a `callbackDispatch` envelope: records the plugin's callbacks.
    ///
    /// A `callbackDispatch` is the callback-bearing transport path: the SDK has
    /// already marshalled every function in the payload into a
    /// `{ "$callback": id }` marker. The host treats those markers as opaque
    /// handles — it does not invoke them here — but every callback id is
    /// recorded as a [`RegistrationHandle::Callback`] in the plugin's ledger so
    /// that [`PluginHost::unload`] disposes the stored functions. The payload is
    /// otherwise returned to the SDK unchanged so a future service layer can
    /// consume it.
    ///
    /// `tools/call` payloads never reach this handler — they cross verbatim via
    /// the `toolsCall` envelope — so no tool call is ever scanned for markers.
    fn callback_dispatch(&self, payload: &Value) -> std::result::Result<Value, String> {
        let inner = payload
            .get("payload")
            .ok_or_else(|| "callbackDispatch envelope missing 'payload'".to_string())?;

        let mut ids = Vec::new();
        collect_callback_ids(inner, &mut ids);

        let mut state = self.host.lock();
        for id in ids {
            // The plugin is tracked from `load`, so this append cannot orphan.
            state.ledger.record(
                &self.plugin_id,
                RegistrationHandle::Callback(CallbackId::new(id)),
            );
        }
        Ok(Value::Object(Map::new()))
    }

    /// Handles a `register` envelope: connects a server source and registers
    /// it under the chosen name, recording it in the plugin's ledger.
    fn register(&self, payload: &Value) -> std::result::Result<Value, String> {
        let name = envelope_str(payload, "name")?;
        let source = payload
            .get("source")
            .ok_or_else(|| "register envelope missing 'source'".to_string())?
            .clone();

        let host = self.host.clone();
        let plugin_id = self.plugin_id.clone();
        Self::block_on(async move { host.connect_and_register(&plugin_id, name, source).await })?
            .map_err(|error| error.to_string())?;
        Ok(Value::Object(Map::new()))
    }

    /// Handles an `unregister` envelope: removes a server and consumes its
    /// ledger entry.
    ///
    /// A plugin unregistering a name it never registered — or already
    /// unregistered — is not an error, but it is a sign of a buggy plugin, so
    /// the case is logged at debug level. This mirrors the diagnostic
    /// [`PluginHost::dispose_handle`] emits when a ledger handle has no live
    /// registration to dispose.
    fn unregister(&self, payload: &Value) -> std::result::Result<Value, String> {
        let name = envelope_str(payload, "name")?;
        let mut state = self.host.lock();
        let removed = state.registry.unregister(&name);
        let consumed = state.ledger.consume_server(&self.plugin_id, &name);
        if removed.is_none() || !consumed {
            tracing::debug!(
                plugin = %self.plugin_id.as_str(),
                server = %name,
                had_registration = removed.is_some(),
                had_ledger_entry = consumed,
                "plugin unregistered a server it did not have registered"
            );
        }
        Ok(Value::Object(Map::new()))
    }

    /// Handles a `log` envelope: forwards the record to `tracing`.
    fn log(&self, payload: &Value) -> std::result::Result<Value, String> {
        let level = payload
            .get("level")
            .and_then(Value::as_str)
            .unwrap_or("info");
        let message = payload.get("message").and_then(Value::as_str).unwrap_or("");
        let fields = payload.get("fields").cloned().unwrap_or(Value::Null);
        let plugin = self.plugin_id.as_str();
        match level {
            "debug" => tracing::debug!(%plugin, ?fields, "{message}"),
            "warn" => tracing::warn!(%plugin, ?fields, "{message}"),
            "error" => tracing::error!(%plugin, ?fields, "{message}"),
            _ => tracing::info!(%plugin, ?fields, "{message}"),
        }
        Ok(Value::Null)
    }
}

impl HostDispatcher for HostBridge {
    /// Answers one SDK wire envelope.
    ///
    /// Reads the envelope's `kind` and routes to the matching handler. An
    /// unrecognized `kind`, or a handler failure, becomes an `Err` string the
    /// op surfaces to the plugin as a thrown JavaScript exception.
    fn dispatch(&self, payload: Value) -> std::result::Result<Value, String> {
        let kind = payload
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| "bridge envelope missing 'kind'".to_string())?;
        match kind {
            "toolsList" => self.tools_list(&payload),
            "toolsCall" => self.tools_call(&payload),
            "callbackDispatch" => self.callback_dispatch(&payload),
            "register" => self.register(&payload),
            "unregister" => self.unregister(&payload),
            "log" => self.log(&payload),
            other => Err(format!("unsupported bridge envelope kind '{other}'")),
        }
    }
}

impl PluginHost {
    /// Connects a [`ServerSource`] and registers the resulting server.
    ///
    /// The source's kind selects the transport: a `rust` id activates a module
    /// from the available-modules table, a `cli` array spawns a [`CliServer`],
    /// and a `url` connects a [`UrlServer`]. The connected server is inserted
    /// into the live registry under `name` and a [`RegistrationHandle::Server`]
    /// is appended to the plugin's ledger.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ServerNameTaken`] when `name` is already registered,
    /// [`Error::UnknownServer`] when a `rust` id names no exposed module, or
    /// [`Error::ServerUnavailable`] when a `cli` or `url` source cannot be
    /// connected or the source shape is not one of the three kinds.
    async fn connect_and_register(
        &self,
        plugin_id: &PluginId,
        name: ServerName,
        source: Value,
    ) -> Result<()> {
        let server = self.connect_source(&source).await?;

        let mut state = self.lock();
        state.registry.register(name.clone(), server)?;
        // The plugin is tracked from `load`, so this append cannot be orphaned.
        state
            .ledger
            .record(plugin_id, RegistrationHandle::Server(name));
        Ok(())
    }

    /// Connects the [`McpServer`] a [`ServerSource`] describes.
    async fn connect_source(&self, source: &Value) -> Result<Arc<dyn McpServer>> {
        if let Some(id) = source.get("rust").and_then(Value::as_str) {
            return self.activate_rust_module(id);
        }
        if let Some(cli) = source.get("cli") {
            return self.connect_cli(cli, source).await;
        }
        if let Some(url) = source.get("url").and_then(Value::as_str) {
            return self.connect_url(url, source).await;
        }
        Err(Error::ServerUnavailable)
    }

    /// Activates a Rust module from the available-modules table.
    ///
    /// The module is moved out of the table — activation is one-shot — so a
    /// second `register` of the same id fails with [`Error::UnknownServer`].
    fn activate_rust_module(&self, id: &str) -> Result<Arc<dyn McpServer>> {
        self.lock().modules.remove(id).ok_or(Error::UnknownServer)
    }

    /// Connects a [`CliServer`] from a `cli` source.
    async fn connect_cli(&self, cli: &Value, source: &Value) -> Result<Arc<dyn McpServer>> {
        let command: Vec<String> = cli
            .as_array()
            .ok_or(Error::ServerUnavailable)?
            .iter()
            .map(|value| value.as_str().map(str::to_string))
            .collect::<Option<Vec<_>>>()
            .ok_or(Error::ServerUnavailable)?;
        let env = source.get("env").and_then(json_string_map);
        let cwd = source.get("cwd").and_then(Value::as_str).map(PathBuf::from);
        let server = CliServer::connect(command, env, cwd).await?;
        Ok(Arc::new(server))
    }

    /// Connects a [`UrlServer`] from a `url` source.
    async fn connect_url(&self, url: &str, source: &Value) -> Result<Arc<dyn McpServer>> {
        let headers = source.get("headers").and_then(Value::as_object).cloned();
        let server = UrlServer::connect(url.to_string(), headers).await?;
        Ok(Arc::new(server))
    }
}

/// Collects every `$callback` marker id reachable in `value` into `ids`.
///
/// The SDK marshals a function anywhere in a callback-bearing payload into a
/// `{ "$callback": "cb_xxxx" }` marker. This walks `value` to any depth — into
/// arrays and object fields — so a marker is found in any payload position,
/// and appends each marker's id to `ids` in document order.
///
/// The walk is iterative: it uses an explicit heap-allocated work-stack rather
/// than recursion, so a hostile-depth payload — legal but deeply nested JSON
/// from a compromised isolate — costs heap, not call frames, and cannot
/// overflow the host worker thread's stack regardless of nesting depth.
/// Children are pushed in reverse so the LIFO stack still pops them in document
/// order, keeping the appended ids in the same order a depth-first recursion
/// would have produced.
fn collect_callback_ids(value: &Value, ids: &mut Vec<String>) {
    let mut stack: Vec<&Value> = vec![value];
    while let Some(node) = stack.pop() {
        match node {
            Value::Object(map) => {
                // A single-key `{ "$callback": "<id>" }` object is a marker;
                // its id is collected and its (only) field is not descended.
                if map.len() == 1 {
                    if let Some(Value::String(id)) = map.get("$callback") {
                        ids.push(id.clone());
                        continue;
                    }
                }
                stack.extend(map.values().rev());
            }
            Value::Array(items) => {
                stack.extend(items.iter().rev());
            }
            _ => {}
        }
    }
}

/// Reads a required string field from a bridge envelope.
fn envelope_str(payload: &Value, field: &str) -> std::result::Result<String, String> {
    payload
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("bridge envelope missing string field '{field}'"))
}

/// Converts a JSON object of string values into a `HashMap`.
///
/// Returns `None` when `value` is not an object or any value is not a string,
/// so a malformed `env` or header map is rejected rather than partly applied.
fn json_string_map(value: &Value) -> Option<HashMap<String, String>> {
    let object = value.as_object()?;
    let mut map = HashMap::with_capacity(object.len());
    for (key, value) in object {
        map.insert(key.clone(), value.as_str()?.to_string());
    }
    Some(map)
}

/// Renders a server's [`ToolMetadata`] list as the `tools` array of a
/// `toolsList` response.
///
/// Serializing an [`rmcp::model::Tool`] should never fail in practice; if it
/// ever did, the tool is rendered as `Value::Null` and the failure is logged
/// at warn level so the (unexpected) dropped tool is observable rather than
/// silent.
fn tools_to_json(tools: &[ToolMetadata]) -> Value {
    Value::Array(
        tools
            .iter()
            .map(|tool| {
                let tool = tool.as_tool();
                serde_json::to_value(tool).unwrap_or_else(|error| {
                    tracing::warn!(
                        %error,
                        "failed to serialize a tool for a toolsList response; dropping it"
                    );
                    Value::Null
                })
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::collect_callback_ids;
    use serde_json::{json, Map, Value};

    /// `collect_callback_ids` appends every `$callback` id in document order,
    /// descending through both objects and arrays.
    #[test]
    fn collects_callback_ids_in_document_order() {
        let payload = json!({
            "first": { "$callback": "cb_one" },
            "list": [
                { "$callback": "cb_two" },
                { "nested": { "$callback": "cb_three" } },
            ],
            "plain": "not a marker",
        });

        let mut ids = Vec::new();
        collect_callback_ids(&payload, &mut ids);

        assert_eq!(
            ids,
            vec![
                "cb_one".to_string(),
                "cb_two".to_string(),
                "cb_three".to_string(),
            ],
            "ids must be appended in document order"
        );
    }

    /// A multi-key object whose extra key happens to be `$callback` is not a
    /// marker: its fields are descended instead of the object being collected.
    #[test]
    fn a_multi_key_object_is_not_treated_as_a_marker() {
        let payload = json!({
            "$callback": "cb_decoy",
            "also": { "$callback": "cb_real" },
        });

        let mut ids = Vec::new();
        collect_callback_ids(&payload, &mut ids);

        assert_eq!(
            ids,
            vec!["cb_real".to_string()],
            "only the single-key marker nested under `also` is a real callback"
        );
    }

    /// Builds `{"n":{"n":{ … {"$callback":"cb_deep"} … }}}` nested `depth`
    /// levels deep, one level per loop iteration so construction itself never
    /// recurses.
    fn deeply_nested_marker(depth: usize) -> Value {
        let mut node = json!({ "$callback": "cb_deep" });
        for _ in 0..depth {
            let mut wrapper = Map::with_capacity(1);
            wrapper.insert("n".to_string(), node);
            node = Value::Object(wrapper);
        }
        node
    }

    /// Dismantles a `{"n":{"n":{ … }}}` chain iteratively so the value's
    /// recursive `Drop` never has to unwind the whole nesting at once.
    fn unnest(mut node: Value) {
        while let Value::Object(mut map) = node {
            match map.remove("n") {
                Some(inner) => node = inner,
                None => break,
            }
        }
    }

    /// A hostile-depth payload — legal JSON nested far past any sane recursion
    /// limit — is walked without overflowing the worker thread's stack.
    ///
    /// The walk runs on a thread with a deliberately small (256 KiB) stack:
    /// the iterative work-stack keeps depth on the heap, so it passes, whereas
    /// the recursive predecessor of `collect_callback_ids` — one call frame per
    /// level — would overflow a stack this size. The nested value is built and
    /// torn down iteratively (see {@link unnest}) so neither construction nor
    /// `Drop` recursion can mask the property under test.
    #[test]
    fn a_deeply_nested_payload_does_not_overflow_the_stack() {
        // Far deeper than 256 KiB of recursive `collect_callback_ids` frames
        // could survive, but the iterative walk costs only heap.
        const DEPTH: usize = 100_000;
        const SMALL_STACK: usize = 256 * 1024;

        let walker = std::thread::Builder::new()
            .name("deep-callback-walk".to_string())
            .stack_size(SMALL_STACK)
            .spawn(|| {
                let payload = deeply_nested_marker(DEPTH);
                let mut ids = Vec::new();
                collect_callback_ids(&payload, &mut ids);
                unnest(payload);
                ids
            })
            .expect("the walker thread should spawn");

        let ids = walker
            .join()
            .expect("the iterative walk must not overflow a 256 KiB stack");
        assert_eq!(
            ids,
            vec!["cb_deep".to_string()],
            "the marker buried {DEPTH} levels deep must still be collected"
        );
    }
}
