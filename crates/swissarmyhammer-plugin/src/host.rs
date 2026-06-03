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
//! # Discovery
//!
//! On top of explicit `load` / `unload`, the host scans its layer roots for
//! plugins on disk: [`discover_and_load_all`](PluginHost::discover_and_load_all)
//! is a point-in-time scan that resolves, per plugin id, the highest-precedence
//! copy across layers (project shadows user shadows builtin) and loads it. The
//! read-only builtin layer is the lowest-precedence floor — an embedder ships
//! it via [`new`](PluginHost::new) — under the writable user and project
//! layers. The scan is all-or-nothing: a mid-scan load failure rolls back
//! every plugin the scan already loaded, so a failed scan leaves the host
//! unchanged.
//!
//! # Hot reload
//!
//! On top of point-in-time discovery, the host can *react* to plugin files
//! changing on disk: [`watch_plugins`](PluginHost::watch_plugins) starts the
//! `swissarmyhammer-directory` stack-aware watcher on the `plugins/`
//! subdirectory and spawns a task that drains its event stream, translating
//! each [`StackedEvent`](swissarmyhammer_directory::StackedEvent) into a load,
//! reload, or unload — see [`reload`](crate::reload) for the seams hot reload
//! exposes, and the methods further down this module for the translation
//! rules.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Map, Value};
use swissarmyhammer_directory::{DirectoryConfig, FileSource, StackedEvent, Watcher};
use tokio::sync::mpsc;

use crate::codegen::TypesEmitter;
use crate::discovery::{
    discover_plugins, resolve_index_entry, DiscoveredPlugin, LayerRoot, PLUGINS_SUBDIR,
};
use crate::error::{Error, Result};
use crate::ledger::{CallbackId, PluginLedger, RegistrationHandle};
use crate::notify::NotificationBridge;
use crate::registry::{
    RegisterOutcome, ServerName, ServerRegistry, ServerSource, ServerStatus, UnregisterOutcome,
};
use crate::reload::ReloadStatus;
use crate::runtime::{HostDispatcher, PluginLifecycle, PluginRuntime, RuntimeConfig};
use crate::server::{CallerId, CliServer, McpServer, PluginId, ToolMetadata, UrlServer};

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
            .field("builtin_root", &self.inner.builtin_root)
            .field("user_root", &self.inner.user_root)
            .field("project_root", &self.inner.project_root)
            .field("cwd", &self.inner.cwd)
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
    /// The read-only builtin-layer plugin root, when the embedder ships one.
    ///
    /// This is the lowest-precedence discovery layer: the embedder owns
    /// assembly extraction (the kanban app `include_dir!`-bundles
    /// `builtin/plugins/` and extracts it to a cache), and hands the resulting
    /// directory here. [`discovery_layers`](PluginHost::discovery_layers)
    /// prepends it as a [`FileSource::Builtin`] layer, so
    /// [`discover_and_load_all`](PluginHost::discover_and_load_all) scans
    /// builtin → user → project. It is never watched for hot reload — the
    /// builtin layer ships frozen with the binary.
    builtin_root: Option<PathBuf>,

    /// The writable user-layer plugin root supplied at construction.
    user_root: PathBuf,

    /// The writable project-layer plugin root, when the embedder has one.
    project_root: Option<PathBuf>,

    /// The working directory every plugin isolate this host spawns reports to
    /// plugin code as `Deno.cwd()`.
    ///
    /// The process CWD is global, so a single process cannot give each per-board
    /// host its own working directory through it. Instead each host carries its
    /// configured directory here — a per-board host its board dir, the boardless
    /// global host the process cwd — and threads it into every
    /// [`RuntimeConfig`] it builds (see [`load_resolved`](PluginHost::load_resolved)),
    /// so a plugin's `Deno.cwd()` resolves against *this* host's board. It is
    /// immutable after construction and so sits outside the mutex.
    cwd: PathBuf,

    /// Source of stable, per-host-unique plugin ids.
    next_plugin_seq: AtomicU64,

    /// The emitter that keeps a generated `.d.ts` file in sync with the live
    /// registry.
    ///
    /// The host drives it as servers register, unregister, and change their
    /// tool sets, and flushes it at plugin load/unload boundaries. The emitter
    /// carries its own dev-mode flag — set at construction — so a production
    /// host (flag off) drives the emitter exactly the same way but writes no
    /// file. It is [`Clone`] and internally synchronized, so it sits outside
    /// the host mutex alongside the other immutable-after-construction fields.
    types_emitter: TypesEmitter,

    /// The one long-lived Tokio runtime every bridge call's host-async work
    /// runs on.
    ///
    /// A bridge call ([`HostBridge::dispatch`]) runs synchronously on a
    /// plugin's isolate worker thread, which is itself already inside that
    /// worker's own `block_on` — so the host's async work cannot nest there
    /// and needs a separate runtime. This runtime is that separate runtime,
    /// created **once**, at host construction, and alive for the host's whole
    /// lifetime.
    ///
    /// The lifetime is what makes the `cli`/`url` transports correct. A
    /// [`CliServer`]/[`UrlServer`] connected during a `register` bridge call
    /// holds an `rmcp` `RunningService` whose background service loop is a task
    /// spawned on whatever runtime drove the `connect`. Because every bridge
    /// call — `register`, `toolsCall`, `unregister`, callbacks — is routed onto
    /// *this* runtime rather than a per-call throwaway, that service loop keeps
    /// running between calls, so a `toolsCall` after a `register` still reaches
    /// a live peer instead of one whose loop was torn down with an ephemeral
    /// runtime.
    bridge_runtime: BridgeRuntime,

    /// The app-wide MCP notification bridge.
    ///
    /// The platform-layer face of the app's in-process event buses: the
    /// wiring layer's per-bus fan-in adapters publish normalized
    /// [`McpNotification`](crate::notify::McpNotification)s here, and every
    /// client — the in-process webview/host and external agents over a
    /// transport — subscribes through it. Held on the host (rather than
    /// threaded separately) because every transport and every wiring layer
    /// already holds the host, so this is the one canonical instance they
    /// share; the bridge is itself `Clone` over one broadcast channel and one
    /// subscription registry, so it sits with the immutable-after-construction
    /// fields outside the host mutex. A `NotificationBridge` with no
    /// subscribers is inert, so a host whose embedder never wires the bus
    /// adapters pays nothing.
    notification_bridge: NotificationBridge,

    /// The mutable host state guarded by one mutex.
    state: Mutex<HostState>,
}

/// The host's one long-lived bridge runtime, owned on a dedicated thread.
///
/// Every bridge call routes its host-async work onto this runtime — see
/// [`HostBridge::block_on`] and [`bridge_runtime`](HostInner::bridge_runtime)
/// for why a single persistent runtime is what keeps the `cli`/`url`
/// transports' `rmcp` `RunningService` loops alive between calls.
///
/// # Why the runtime lives on its own thread
///
/// A multi-thread [`tokio::runtime::Runtime`] must not be *dropped* from inside
/// an async context: its drop blocks to shut the worker pool down, and Tokio
/// panics if that blocking drop runs on a thread already inside a runtime.
/// Because [`PluginHost`] is freely cloned and the last clone is commonly
/// dropped inside the embedder's own `async fn` (every `#[tokio::test]` does
/// exactly this), storing a bare `Runtime` here would be a latent panic.
///
/// So the `Runtime` is *moved onto a dedicated parked thread* and only its
/// [`Handle`](tokio::runtime::Handle) is kept for spawning. On drop, this type
/// signals that thread to wake and drop the `Runtime` there — on a plain OS
/// thread that is never inside any runtime — so the blocking shutdown is
/// always sound.
struct BridgeRuntime {
    /// A spawn handle into the runtime, used by every bridge call.
    handle: tokio::runtime::Handle,

    /// Signals the owner thread to drop the `Runtime`. `Some` until [`Drop`]
    /// takes it; sending (or dropping the sender) wakes the owner thread.
    shutdown: Option<std::sync::mpsc::Sender<()>>,

    /// Join handle for the owner thread, taken and joined in [`Drop`] so the
    /// runtime's shutdown completes before the host is fully gone.
    owner: Option<std::thread::JoinHandle<()>>,
}

impl BridgeRuntime {
    /// Builds the bridge runtime and parks it on its own dedicated thread.
    ///
    /// The runtime is a multi-thread runtime so a bridge call that itself
    /// triggers another bridge call — a host operation that routes back
    /// through a plugin — does not starve on a single worker thread.
    ///
    /// # Panics
    ///
    /// Panics if the Tokio runtime cannot be built or its owner thread cannot
    /// be spawned. The host cannot function without its bridge runtime, so a
    /// failure here is unrecoverable — like a poisoned host mutex. In practice
    /// neither step fails: each only allocates threads.
    fn new() -> Self {
        let (handle_tx, handle_rx) = std::sync::mpsc::channel();
        let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel();
        let owner = std::thread::Builder::new()
            .name("plugin-host-bridge-rt".to_string())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .thread_name("plugin-host-bridge")
                    .build()
                    .expect("the plugin host's bridge runtime must build");
                // Hand the spawn handle back, then park until shutdown. The
                // `Runtime` is dropped here, on this plain OS thread, so its
                // blocking shutdown never runs inside another runtime.
                let _ = handle_tx.send(runtime.handle().clone());
                let _ = shutdown_rx.recv();
            })
            .expect("the plugin host's bridge runtime thread must spawn");
        let handle = handle_rx
            .recv()
            .expect("the bridge runtime thread must report its handle");
        Self {
            handle,
            shutdown: Some(shutdown_tx),
            owner: Some(owner),
        }
    }

    /// The spawn handle every bridge call submits its future to.
    fn handle(&self) -> &tokio::runtime::Handle {
        &self.handle
    }
}

impl Drop for BridgeRuntime {
    /// Shuts the bridge runtime down on its own owner thread.
    ///
    /// Dropping the shutdown sender wakes the parked owner thread, which drops
    /// the `Runtime` there — on a plain OS thread, never inside another
    /// runtime — and exits. Joining that thread makes host teardown wait for
    /// the runtime's shutdown to finish.
    fn drop(&mut self) {
        // Drop the sender to wake the owner thread out of `recv()`.
        self.shutdown.take();
        if let Some(owner) = self.owner.take() {
            let _ = owner.join();
        }
    }
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

    /// The currently active copy of each plugin id, keyed by the plugin's
    /// bundle-directory id.
    ///
    /// Hot reload reasons in terms of plugin identity and layer precedence,
    /// not the host-minted [`PluginId`]: this map records, per plugin id,
    /// which layer's copy is live and the internal id it loaded under, so a
    /// [`StackedEvent`](swissarmyhammer_directory::StackedEvent) can be
    /// translated against the active layer. A plugin appears here exactly while
    /// it is loaded; discovery and the watcher both populate it; an unload
    /// removes it.
    active_plugins: HashMap<String, ActivePlugin>,

    /// The outcome of the most recent reload of each plugin id, keyed by the
    /// plugin's bundle-directory id.
    ///
    /// A failed v2 load leaves the plugin unloaded; the failure is recorded
    /// here so a caller — the settings UI, a test — can observe it. A
    /// successful load records [`ReloadStatus::Healthy`].
    reload_status: HashMap<String, ReloadStatus>,
}

/// A content fingerprint of a plugin bundle's source, used to tell a *genuine*
/// change of the active copy apart from a no-op reconcile.
///
/// Hot reload re-runs full discovery and reconciles *every* plugin id on any
/// watcher event, so [`reconcile_id`](PluginHost::reconcile_id) is reached for
/// an id whose active copy did not change — for example when a *shadowed*
/// lower-layer copy of the same id was edited. Without a way to see that the
/// winning copy's source is byte-for-byte unchanged, the reconcile would tear
/// the active isolate down and re-`load()` it for nothing, losing class-field
/// state. The fingerprint is that signal: it is computed from the bytes of the
/// winning copy's bundle, so it changes exactly when the winning copy's source
/// changes.
///
/// # What is fingerprinted
///
/// The fingerprint hashes the bytes of the bundle's resolved entry module, so
/// an edit to the plugin's source is caught.
///
/// A bundle whose entry cannot be read fingerprints to [`Self::Unreadable`],
/// which compares **unequal to every fingerprint including another
/// `Unreadable`** — so a bundle the host cannot fingerprint is always treated
/// as changed and reloaded. The reconcile fails *toward* a reload, never toward
/// leaving a possibly-stale plugin in place.
#[derive(Debug, Clone)]
enum PluginFingerprint {
    /// A 64-bit content hash of the bundle's entry module.
    Hashed(u64),

    /// The bundle's files could not be read, so no content hash exists.
    ///
    /// Carries a unit field purely so the [`PartialEq`] impl can make every
    /// `Unreadable` compare unequal — including to another `Unreadable` — which
    /// forces a fingerprint-guarded reload to proceed when the source state is
    /// unknown.
    Unreadable,
}

impl PartialEq for PluginFingerprint {
    /// Two fingerprints are equal only when both are [`Hashed`](Self::Hashed)
    /// with the same hash.
    ///
    /// An [`Unreadable`](Self::Unreadable) is never equal to anything — not
    /// even another `Unreadable` — so a fingerprint comparison involving an
    /// unreadable bundle always reports "changed" and a reload proceeds.
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Hashed(left), Self::Hashed(right)) => left == right,
            _ => false,
        }
    }
}

impl PluginFingerprint {
    /// Computes the fingerprint of a discovered plugin's bundle.
    ///
    /// Reads and hashes the bundle's entry module; any read failure yields
    /// [`Self::Unreadable`]. The hash is a `DefaultHasher` digest — this is a
    /// same-process change-detection signal, not a security boundary, so a
    /// fast non-cryptographic hash is the right tool.
    ///
    /// # Parameters
    ///
    /// - `plugin` — the discovered copy whose bundle is fingerprinted.
    fn of(plugin: &DiscoveredPlugin) -> Self {
        use std::hash::{Hash, Hasher};

        let mut hasher = std::collections::hash_map::DefaultHasher::new();

        let Ok(entry_bytes) = std::fs::read(&plugin.entry) else {
            return Self::Unreadable;
        };
        entry_bytes.hash(&mut hasher);

        Self::Hashed(hasher.finish())
    }
}

/// The active copy of a plugin id, as the host tracks it for hot reload.
///
/// One `ActivePlugin` is the *currently live* copy of a plugin `id`. The
/// watcher's translation logic uses it to answer "which layer is active for
/// this id" and "which internal plugin id do I unload to reload it".
#[derive(Debug, Clone)]
struct ActivePlugin {
    /// The host-minted internal id the active copy loaded under.
    plugin_id: PluginId,

    /// The layer the active copy was loaded from.
    layer: FileSource,

    /// A content fingerprint of the active copy's bundle at the time it was
    /// loaded.
    ///
    /// Compared against a freshly re-discovered winner's fingerprint in
    /// [`reconcile_id`](PluginHost::reconcile_id): when the winner is the same
    /// layer *and* the same fingerprint, the active copy's source did not
    /// change and the reconcile is a no-op — so a `Modified` to a *shadowed*
    /// lower-layer copy never tears down the active copy.
    fingerprint: PluginFingerprint,
}

/// A plugin the host has loaded: its isolate plus where its bundle lives.
///
/// The bundle directory and entry file are retained so [`PluginHost::unload`]
/// can re-reach the bundle's optional `unload` export before the host disposes
/// the plugin's registrations and tears the isolate down.
struct LoadedPlugin {
    /// The plugin's V8 isolate, on its own worker thread.
    runtime: PluginRuntime,

    /// The plugin's bundle directory, as passed to [`PluginHost::load`].
    bundle_dir: PathBuf,

    /// The plugin's entry file, exactly as it was passed to the runtime when
    /// the plugin's `load` export was driven.
    ///
    /// Retained so [`PluginHost::unload`] re-resolves the `unload` export
    /// against the **identical** entry path the `load` export used. The entry
    /// is canonicalized by `resolve_index_entry` when it is resolved, so the
    /// isolate's module map keys the entry's main module under that canonical
    /// URL. Re-deriving the entry for unload would join it onto a
    /// *non*-canonical bundle directory and ask the isolate to create a
    /// second "main" module under a URL that differs only by an unresolved
    /// symlink — which `deno_core` rejects, silently skipping the `unload`
    /// hook. Reusing the stored entry keeps the two lifecycle calls addressing
    /// the one module.
    entry_file: String,
}

/// A live plugin watcher: hold it to keep hot reload running.
///
/// Returned by [`PluginHost::watch_plugins`]. While a `PluginWatcher` is held,
/// the host reacts to plugin files changing on disk by loading, reloading, or
/// unloading the affected plugins. Dropping it stops every underlying
/// filesystem watcher and aborts the task draining their events — hot reload
/// stops, but the plugins already loaded keep running.
pub struct PluginWatcher {
    /// The underlying per-layer stack-aware watchers; kept alive to keep
    /// watching. The concrete [`DirectoryConfig`] is erased because the drain
    /// task already closed over it — each watcher value only needs to stay
    /// alive.
    _watchers: Vec<Box<dyn std::any::Any + Send>>,

    /// The task draining the merged [`StackedEvent`] stream; aborted on drop
    /// so no reconcile runs after the watchers are gone.
    drain: tokio::task::JoinHandle<()>,
}

impl std::fmt::Debug for PluginWatcher {
    /// `Debug` is hand-written because the boxed watcher is an opaque
    /// `dyn Any`. The impl reports only that the watcher is live.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginWatcher").finish_non_exhaustive()
    }
}

impl Drop for PluginWatcher {
    /// Aborts the drain task so no reconcile outlives the watcher.
    ///
    /// The inner watcher stops on its own drop; aborting the drain task
    /// ensures the host performs no further reconciliation once hot reload is
    /// torn down.
    fn drop(&mut self) {
        self.drain.abort();
    }
}

impl PluginHost {
    /// Creates a host for tests with explicit plugin-layer roots.
    ///
    /// The platform stays host-agnostic: a test supplies the roots it wants
    /// rather than the host reaching for global configuration.
    ///
    /// The host's [`TypesEmitter`] is constructed with **dev-mode off**: a test
    /// host drives the emitter on every registry event exactly as a production
    /// host does, but writes no `.d.ts` file, so the test working directory
    /// stays clean. A test that wants to observe the generated file uses
    /// [`with_types_dev_mode`](Self::with_types_dev_mode) to point a dev-mode
    /// emitter at a temp directory.
    ///
    /// # Parameters
    ///
    /// - `user_root` — the writable user-layer plugin directory.
    /// - `project_root` — the writable project-layer plugin directory, or
    ///   `None` when the test models a host with no project layer.
    pub fn for_tests(user_root: PathBuf, project_root: Option<PathBuf>) -> Self {
        let types_emitter = TypesEmitter::new(&user_root, false);
        Self::with_roots(
            None,
            user_root,
            project_root,
            default_isolate_cwd(),
            types_emitter,
        )
    }

    /// Record that a loaded plugin's isolate has crashed.
    ///
    /// Mirrors [`unload`](Self::unload) but skips the plugin's own
    /// `unload()` lifecycle hook — the isolate is dead, the hook cannot run.
    /// All of the plugin's registrations are disposed (so subsequent calls to
    /// its servers fail with [`Error::ServerUnavailable`]), the runtime is
    /// dropped, and [`ReloadStatus::Crashed`] is recorded against the
    /// plugin's active disk id.
    ///
    /// The plugin does **not** auto-restart. The watcher only fires on file
    /// changes; a crash is not a file change. The plugin stays
    /// [`ReloadStatus::Crashed`] until the user touches the bundle (which
    /// triggers a normal reload) or calls [`load`](Self::load) directly.
    ///
    /// # Parameters
    ///
    /// - `plugin_id` — the id of the plugin to mark crashed.
    /// - `error` — a human-readable description of the crash, surfaced through
    ///   [`reload_status`](Self::reload_status). Typically the `Display` of
    ///   the runtime error that detected the dead isolate.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnknownPlugin`] when no plugin is loaded under
    /// `plugin_id`. A successful call sets the crashed status — the plugin
    /// is now unloaded, no servers, no runtime.
    pub async fn record_crashed(&self, plugin_id: &PluginId, error: &str) -> Result<()> {
        // Take the loaded plugin out. `PluginRuntime` is not `Sync`, so it is
        // held in this local — never across the host mutex — while disposal
        // runs.
        let Some(plugin) = self.lock().plugins.remove(plugin_id) else {
            return Err(Error::UnknownPlugin);
        };

        // Skip `run_plugin_unload` — the isolate is dead. Disposal still runs
        // against the (about-to-drop) runtime; callback disposal logs and
        // continues if the runtime cannot answer.
        self.dispose_registrations(plugin_id, &plugin.runtime).await;
        self.inner.types_emitter.flush();
        drop(plugin);

        // Record Crashed against the active disk id, if the plugin had one
        // registered. A plugin loaded outside the discovery scan (a direct
        // `load(path)` from a test or an embedder) has no `active_plugins`
        // entry, so there is no `ReloadStatus` surface for it to populate —
        // the registrations are still disposed and the runtime still
        // dropped, but `reload_status(...)` will return `None` afterward.
        // Logging the silent path keeps the asymmetry observable in
        // operator-facing logs rather than appearing as a phantom success.
        let mut state = self.lock();
        let disk_id = state
            .active_plugins
            .iter()
            .find_map(|(disk, active)| (active.plugin_id == *plugin_id).then(|| disk.clone()));
        match disk_id {
            Some(disk_id) => {
                state.active_plugins.remove(&disk_id);
                state.reload_status.insert(
                    disk_id,
                    ReloadStatus::Crashed {
                        error: error.to_string(),
                    },
                );
            }
            None => {
                tracing::warn!(
                    plugin = %plugin_id.as_str(),
                    %error,
                    "record_crashed disposed registrations but found no active-plugin record; \
                     ReloadStatus will not be surfaced for this plugin id"
                );
            }
        }
        Ok(())
    }

    /// Marks `name` as in the hot-reload window — *test-only*.
    ///
    /// Used by integration tests that need to exercise the
    /// [`Error::PluginReloaded`] surface deterministically without driving a
    /// real v1→v2 source rewrite + watcher debounce. The reload path
    /// (`reload_active`) uses the same registry primitive internally; this
    /// shim exposes it under `pub` so an external `tests/*_e2e.rs` file can
    /// stage the window directly.
    #[doc(hidden)]
    pub async fn mark_reloading_for_test(&self, name: &str) {
        self.lock().registry.mark_reloading(name);
    }

    /// Clears the hot-reload marker for `name` — *test-only*. See
    /// [`mark_reloading_for_test`](Self::mark_reloading_for_test).
    #[doc(hidden)]
    pub async fn clear_reloading_for_test(&self, name: &str) {
        self.lock().registry.clear_reloading(name);
    }

    /// Creates a test host with an explicit read-only builtin layer root.
    ///
    /// Like [`for_tests`](Self::for_tests) but with a builtin layer added
    /// underneath the user and project layers — the lowest-precedence
    /// discovery layer. A test points `builtin_root` at a fixture tree (the
    /// committed `test/builtin/plugins/` tree, or a temp tree it staged) whose
    /// `plugins/` subdirectory holds builtin bundles, so
    /// [`discover_and_load_all`](Self::discover_and_load_all) discovers them
    /// tagged [`FileSource::Builtin`], stacking below user and project.
    ///
    /// The [`TypesEmitter`] is constructed with dev-mode off, exactly as
    /// [`for_tests`](Self::for_tests) does.
    ///
    /// # Parameters
    ///
    /// - `builtin_root` — the read-only builtin-layer plugin directory; its
    ///   `plugins/` subdirectory holds the builtin bundles.
    /// - `user_root` — the writable user-layer plugin directory.
    /// - `project_root` — the writable project-layer plugin directory, or
    ///   `None` when the test models a host with no project layer.
    pub fn for_tests_with_builtin(
        builtin_root: PathBuf,
        user_root: PathBuf,
        project_root: Option<PathBuf>,
    ) -> Self {
        let types_emitter = TypesEmitter::new(&user_root, false);
        Self::with_roots(
            Some(builtin_root),
            user_root,
            project_root,
            default_isolate_cwd(),
            types_emitter,
        )
    }

    /// Creates a test host whose [`TypesEmitter`] writes to `types_dir`.
    ///
    /// Like [`for_tests`](Self::for_tests) but with the emitter in **dev mode**
    /// and writing under the supplied directory, so a test can load a plugin
    /// and then assert the generated declaration file on disk. Production hosts
    /// reach the same posture through [`new`](Self::new)'s `dev_mode` argument.
    ///
    /// # Parameters
    ///
    /// - `user_root` — the writable user-layer plugin directory.
    /// - `project_root` — the writable project-layer plugin directory, or
    ///   `None` when the test models a host with no project layer.
    /// - `types_dir` — the base directory the generated `.d.ts` is written
    ///   under, joined with [`DEFAULT_TYPES_PATH`](crate::codegen::DEFAULT_TYPES_PATH).
    pub fn with_types_dev_mode(
        user_root: PathBuf,
        project_root: Option<PathBuf>,
        types_dir: PathBuf,
    ) -> Self {
        let types_emitter = TypesEmitter::new(&types_dir, true);
        Self::with_roots(
            None,
            user_root,
            project_root,
            default_isolate_cwd(),
            types_emitter,
        )
    }

    /// Creates a production host from the read-only builtin layer root and the
    /// writable layer roots.
    ///
    /// The embedder — the kanban app, the CLI, the TUI — computes its own
    /// directories and passes them in; the platform hardcodes none. The builtin
    /// root is the lowest-precedence discovery layer: the embedder owns
    /// assembly extraction (the kanban app `include_dir!`-bundles
    /// `builtin/plugins/` and extracts it to a cache) and hands the resulting
    /// directory in. Builtins are not loaded eagerly by `new` — a later
    /// [`discover_and_load_all`](Self::discover_and_load_all) scans the builtin
    /// layer alongside the user and project layers, so the embedder can expose
    /// any host modules a builtin needs before discovery runs.
    ///
    /// The host's [`TypesEmitter`] is constructed from the `dev_mode` flag and
    /// the `types_dir` the embedder supplies: a development embedder passes
    /// `dev_mode: true` so the generated `.d.ts` is kept in sync on disk; a
    /// production embedder passes `false` so the host drives the emitter but
    /// writes nothing. The host hardcodes neither — both are caller-supplied,
    /// keeping the platform host-agnostic.
    ///
    /// # Parameters
    ///
    /// - `builtin_root` — the read-only builtin-layer plugin directory, or
    ///   `None` when the embedder ships no builtins; its `plugins/`
    ///   subdirectory holds the builtin bundles.
    /// - `user_root` — the writable user-layer plugin directory.
    /// - `project_root` — the writable project-layer plugin directory, when the
    ///   embedder has a project layer.
    /// - `cwd` — the working directory every plugin isolate this host spawns
    ///   reports as `Deno.cwd()`. The process CWD is global, so a per-board
    ///   embedder passes its board dir here to give that board's plugins their
    ///   own working directory; the boardless embedder passes the process cwd.
    ///   See [`HostInner::cwd`].
    /// - `dev_mode` — `true` to write the generated plugin-types `.d.ts` on
    ///   every registry change, `false` for the production posture (no file).
    /// - `types_dir` — the base directory the generated `.d.ts` is written
    ///   under, joined with [`DEFAULT_TYPES_PATH`](crate::codegen::DEFAULT_TYPES_PATH).
    ///   Consulted only when `dev_mode` is `true`.
    pub fn new(
        builtin_root: Option<PathBuf>,
        user_root: PathBuf,
        project_root: Option<PathBuf>,
        cwd: PathBuf,
        dev_mode: bool,
        types_dir: PathBuf,
    ) -> Self {
        let types_emitter = TypesEmitter::new(&types_dir, dev_mode);
        Self::with_roots(builtin_root, user_root, project_root, cwd, types_emitter)
    }

    /// Builds a host with the given roots, isolate cwd, types emitter, and empty
    /// state.
    ///
    /// This is also where the host's one long-lived
    /// [`bridge_runtime`](HostInner::bridge_runtime) is created — once, here,
    /// for the host's whole lifetime — so every bridge call routes its
    /// host-async work onto the same persistent runtime and the `cli`/`url`
    /// transports' background service loops survive between calls.
    ///
    /// `cwd` is the working directory every plugin isolate this host spawns
    /// reports as `Deno.cwd()` — a per-board host's board dir, the boardless
    /// host's process cwd. See [`HostInner::cwd`].
    fn with_roots(
        builtin_root: Option<PathBuf>,
        user_root: PathBuf,
        project_root: Option<PathBuf>,
        cwd: PathBuf,
        types_emitter: TypesEmitter,
    ) -> Self {
        let bridge_runtime = BridgeRuntime::new();
        Self {
            inner: Arc::new(HostInner {
                builtin_root,
                user_root,
                project_root,
                cwd,
                next_plugin_seq: AtomicU64::new(0),
                types_emitter,
                bridge_runtime,
                notification_bridge: NotificationBridge::new(),
                state: Mutex::new(HostState {
                    registry: ServerRegistry::new(),
                    ledger: PluginLedger::new(),
                    modules: HashMap::new(),
                    plugins: HashMap::new(),
                    active_plugins: HashMap::new(),
                    reload_status: HashMap::new(),
                }),
            }),
        }
    }

    /// The [`ReloadStatus`] of the most recent reload of the plugin whose
    /// id is `plugin_id`.
    ///
    /// Returns `None` when the host has performed no reload-path lifecycle
    /// action for that id — neither a watcher-driven load nor a discovery
    /// scan recorded a status. A loaded, never-reloaded plugin reports
    /// [`ReloadStatus::Healthy`]; a plugin whose reload failed reports the
    /// corresponding variant, so a caller can surface a plugin that needs
    /// attention.
    ///
    /// # Parameters
    ///
    /// - `plugin_id` — the id (bundle directory name) of the plugin to
    ///   inspect.
    pub async fn reload_status(&self, plugin_id: &str) -> Option<ReloadStatus> {
        self.lock().reload_status.get(plugin_id).cloned()
    }

    /// The writable user-layer plugin root this host was given.
    pub fn user_root(&self) -> &Path {
        &self.inner.user_root
    }

    /// The app-wide MCP notification bridge.
    ///
    /// Returns a clone of the host's one [`NotificationBridge`]; every clone
    /// shares the same broadcast channel and subscription registry, so a
    /// notification published on any clone reaches every subscriber. This is
    /// the canonical instance the embedder hands to the wiring layer (whose
    /// per-bus fan-in adapters [`publish`](NotificationBridge::publish) into
    /// it) and that each transport subscribes through to deliver
    /// server→client `notifications/…`.
    pub fn notification_bridge(&self) -> NotificationBridge {
        self.inner.notification_bridge.clone()
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

    /// Reports whether a module is currently exposed under `id` in the
    /// available-modules table — that is, available to be activated by a
    /// plugin's `register(name, { rust: id })`.
    ///
    /// A `true` return covers the window between
    /// [`expose_rust_module`](Self::expose_rust_module) and a plugin's
    /// activation: activation moves the module out of the table, so this
    /// reports `false` afterward. Used by the command-service bootstrap
    /// tests to assert that `expose_rust_module("commands", ...)` actually
    /// landed, without needing a plugin to drive the activation step.
    ///
    /// # Parameters
    ///
    /// - `id` — the module id to look up.
    pub async fn has_exposed_module(&self, id: &str) -> bool {
        self.lock().modules.contains_key(id)
    }

    /// Loads the plugin whose bundle is the directory `plugin_dir`.
    ///
    /// Creates a fresh [`PluginRuntime`] isolate for the plugin, wires its SDK
    /// bridge to a host dispatcher scoped to the new plugin's id, loads the
    /// bundle's entry module through the module loader, and runs the exported
    /// `load` lifecycle function. Every server the plugin registers during
    /// `load()` is inserted into the live registry and recorded in the plugin's
    /// ledger.
    ///
    /// # The entry module
    ///
    /// A bundle's entry module is its `index.ts` (preferred) or `index.js`,
    /// found by the same convention discovery uses and containment-checked
    /// the same way.
    ///
    /// # Parameters
    ///
    /// - `plugin_dir` — the plugin's bundle directory; it must contain an
    ///   `index.ts` or `index.js` entry module.
    ///
    /// # Returns
    ///
    /// The [`PluginId`] the host assigned the freshly loaded plugin.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BundleError`] when the bundle has no `index.{ts,js}`
    /// entry module or the entry escapes the bundle directory,
    /// [`Error::RuntimeStartup`] when the isolate cannot be created,
    /// [`Error::Transpile`] or [`Error::Runtime`] when the bundle fails to
    /// load or its `load()` throws, or any error a `register` made during
    /// `load()` surfaced. A plugin that fails to load is removed from the
    /// host, so a failed load leaves no half-initialized plugin behind.
    pub async fn load(&self, plugin_dir: impl AsRef<Path>) -> Result<PluginId> {
        let plugin_dir = plugin_dir.as_ref().to_path_buf();

        // The entry module is the bundle's `index.ts` (or `index.js`),
        // resolved and containment-checked the same way a discovery scan
        // resolves it.
        let entry_file = resolve_index_entry(&plugin_dir)?
            .ok_or_else(|| {
                Error::BundleError(format!(
                    "plugin bundle at {} has no index.ts or index.js entry module",
                    plugin_dir.display(),
                ))
            })?
            .to_string_lossy()
            .into_owned();
        self.load_resolved(&plugin_dir, entry_file).await
    }

    /// Loads a plugin whose entry module is already resolved.
    ///
    /// Shared by [`load`](Self::load), which resolves a bundle's entry from
    /// disk, and [`discover_and_load_all`](Self::discover_and_load_all), which
    /// already holds the entry from the discovery scan.
    ///
    /// # Parameters
    ///
    /// - `plugin_dir` — the plugin's bundle directory.
    /// - `entry_file` — the resolved, bundle-contained entry module path the
    ///   runtime evaluates. The caller is responsible for the sandbox check:
    ///   it is a discovery-resolved `index.{ts,js}` path, already
    ///   containment-checked.
    async fn load_resolved(&self, plugin_dir: &Path, entry_file: String) -> Result<PluginId> {
        let plugin_id = self.mint_plugin_id();

        // The bridge dispatcher is scoped to this plugin's id: every call it
        // forwards is attributed to `CallerId::Plugin(plugin_id)`.
        let bridge = Arc::new(HostBridge::new(self.clone(), plugin_id.clone()));
        let runtime = PluginRuntime::new(RuntimeConfig {
            dispatcher: Some(bridge),
            // Every isolate this host spawns reports the host's configured
            // working directory as `Deno.cwd()`, so a per-board host's plugins
            // resolve cwd-relative paths against their own board.
            cwd: self.inner.cwd.clone(),
            ..Default::default()
        })?;

        // Track the plugin before running its `load()` so the `register` calls
        // that `load()` makes — which arrive over the bridge while the call
        // below is awaiting — have a ledger vec to append to.
        {
            let mut state = self.lock();
            state.ledger.track(plugin_id.clone());
        }

        // Drive the plugin's lifecycle on its own isolate. `PluginRuntime` is
        // not `Sync`, so the handle is held in this local — never across the
        // host mutex — while `load()` runs. The entry path is retained so a
        // later `unload` re-resolves the `unload` export against the identical
        // module URL the `load` export used.
        let load_result = runtime
            .call_plugin_lifecycle(plugin_dir, entry_file.clone(), PluginLifecycle::Load)
            .await;

        match load_result {
            Ok(_) => {
                // The plugin is fully loaded: retain its isolate, bundle dir,
                // and entry file so a later `unload` can reach its `unload`
                // export and tear the isolate down.
                self.lock().plugins.insert(
                    plugin_id.clone(),
                    LoadedPlugin {
                        runtime,
                        bundle_dir: plugin_dir.to_path_buf(),
                        entry_file,
                    },
                );
                // A `load()` is a flush boundary for the types emitter: the
                // plugin's burst of `register` calls debounced their writes,
                // so flushing now settles the whole burst into one `.d.ts`
                // write the moment the load completes.
                self.inner.types_emitter.flush();
                Ok(plugin_id)
            }
            Err(error) => {
                // A failed load must not leave a half-initialized plugin: undo
                // every registration it managed to make. The isolate is still
                // alive here — it is torn down as `runtime` drops at the end
                // of this scope — so callback handles can be disposed on it.
                self.dispose_registrations(&plugin_id, &runtime).await;
                Err(error)
            }
        }
    }

    /// Discovers every plugin on disk across the host's layer roots and loads
    /// the highest-precedence copy of each.
    ///
    /// This is the point-in-time counterpart to [`load`](Self::load): rather
    /// than naming one bundle, it scans the host's writable layer roots — the
    /// user layer and, when the host has one, the project layer — for plugin
    /// bundles under each layer's `plugins/` subdirectory. When a plugin `id`
    /// appears in more than one layer, the project copy shadows the user copy;
    /// the winning copy is the one loaded.
    ///
    /// # Type Parameters
    ///
    /// - `C` — the host's [`DirectoryConfig`]. It parameterizes the directory
    ///   resolution so the platform stays host-agnostic: the config names where
    ///   a host's layers live; no `.sah`-specific path is baked in.
    ///
    /// # Atomicity
    ///
    /// The scan is all-or-nothing. If any discovered plugin fails to load, the
    /// host unloads every plugin this call already loaded — in reverse order,
    /// the same discipline [`unload`](Self::unload) uses for a single plugin's
    /// ledger — and then returns the `Err`. So a failed scan leaves the host
    /// exactly as it found it, with none of the partially-loaded plugins live.
    /// This mirrors the contract of [`new`](Self::new): a host is never left
    /// silently half-populated, because a caller that got an `Err` has no way
    /// to know what loaded or to unload it.
    ///
    /// # Returns
    ///
    /// The [`PluginId`] of every plugin loaded, in the order discovery resolved
    /// them — one per distinct plugin id. Returned only when *every* discovered
    /// plugin loaded.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BundleError`] when a discovered bundle's entry module
    /// cannot be resolved, or any error [`load`](Self::load) surfaces for a
    /// plugin that fails to load. On any such error every plugin loaded
    /// earlier in the scan has already been unloaded, so no plugin from a
    /// failed scan stays live.
    pub async fn discover_and_load_all<C: DirectoryConfig>(&self) -> Result<Vec<PluginId>> {
        let discovered = discover_plugins::<C>(&self.discovery_layers())?;

        let mut loaded = Vec::with_capacity(discovered.len());
        for plugin in discovered {
            // Hold the identity, source, and content fingerprint the
            // active-plugin record needs before the entry is moved into
            // `load_resolved`.
            let id = plugin.id.clone();
            let source = plugin.source.clone();
            let fingerprint = PluginFingerprint::of(&plugin);
            let entry_file = plugin.entry.to_string_lossy().into_owned();
            match self.load_resolved(&plugin.directory, entry_file).await {
                Ok(plugin_id) => {
                    // Record the active copy by plugin id so the watcher can
                    // later translate events against the layer this scan
                    // resolved as the winner.
                    self.record_active(&id, &plugin_id, source, fingerprint);
                    loaded.push(plugin_id);
                }
                Err(error) => {
                    // A mid-scan failure must not leave a partially populated
                    // host: unload everything this call loaded, newest first,
                    // before surfacing the error. `load_resolved` already
                    // cleaned up the plugin that failed, so only the earlier
                    // successes need rolling back.
                    self.rollback_loaded(&loaded).await;
                    return Err(error);
                }
            }
        }
        Ok(loaded)
    }

    /// Records the active copy of a plugin id.
    ///
    /// Inserts (or replaces) the [`ActivePlugin`] entry for `id` and marks
    /// the plugin [`ReloadStatus::Healthy`], because reaching this point
    /// means the copy loaded and is serving. The `fingerprint` is the content
    /// fingerprint of the copy's bundle at load time — [`reconcile_id`](Self::reconcile_id)
    /// compares it against a re-discovered winner's fingerprint to tell a
    /// genuine source change apart from a no-op reconcile.
    fn record_active(
        &self,
        id: &str,
        plugin_id: &PluginId,
        layer: FileSource,
        fingerprint: PluginFingerprint,
    ) {
        let mut state = self.lock();
        state.active_plugins.insert(
            id.to_string(),
            ActivePlugin {
                plugin_id: plugin_id.clone(),
                layer,
                fingerprint,
            },
        );
        state
            .reload_status
            .insert(id.to_string(), ReloadStatus::Healthy);
    }

    /// Drops every hot-reload record of the plugin whose internal id is
    /// `plugin_id`.
    ///
    /// Used by the discovery-scan rollback: the scan keyed its records by
    /// plugin id, but the rollback holds only the host-minted [`PluginId`]s,
    /// so the matching plugin id is found by scanning for that internal id.
    /// Both the [`ActivePlugin`] entry **and** the [`ReloadStatus`] entry —
    /// `record_active` inserts a `Healthy` status alongside the active record —
    /// are removed, so a rolled-back plugin reports `None` from
    /// [`reload_status`](Self::reload_status) and a failed scan leaves no stale
    /// hot-reload state behind.
    fn forget_active_by_plugin_id(&self, plugin_id: &PluginId) {
        let mut state = self.lock();
        let id = state
            .active_plugins
            .iter()
            .find(|(_, active)| &active.plugin_id == plugin_id)
            .map(|(id, _)| id.clone());
        if let Some(id) = id {
            state.active_plugins.remove(&id);
            // `record_active` paired this id with a `Healthy` reload status;
            // drop it too so a rolled-back plugin has no lingering status.
            state.reload_status.remove(&id);
        }
    }

    /// Unloads, newest first, every plugin a failed discovery scan had loaded.
    ///
    /// Called only from [`discover_and_load_all`](Self::discover_and_load_all)
    /// when a discovered plugin fails to load: it undoes the scan's earlier
    /// successes so the host is left exactly as the scan found it — including
    /// the active-plugin records *and* the [`ReloadStatus`] entries the scan
    /// inserted (both dropped by
    /// [`forget_active_by_plugin_id`](Self::forget_active_by_plugin_id)), so a
    /// failed scan leaves no stale hot-reload state behind. An individual
    /// [`unload`](Self::unload) failure is logged rather than propagated — the
    /// scan's outcome is the original load error, and the rollback proceeds to
    /// the remaining plugins regardless so none is left behind.
    async fn rollback_loaded(&self, loaded: &[PluginId]) {
        for plugin_id in loaded.iter().rev() {
            // Drop the active record and reload status this scan inserted for
            // this plugin.
            self.forget_active_by_plugin_id(plugin_id);
            if let Err(error) = self.unload(plugin_id).await {
                tracing::warn!(
                    plugin = %plugin_id.as_str(),
                    %error,
                    "rolling back a plugin after a failed discovery scan failed"
                );
            }
        }
    }

    /// The host's discovery layer roots, lowest precedence first.
    ///
    /// Discovery scans these in order: the read-only builtin layer when the
    /// embedder ships one, then the user layer, then the project layer when the
    /// host has one. A later layer shadows an earlier one, so this order
    /// encodes "project shadows user shadows builtin" — the builtin layer is
    /// the floor every other layer stacks on top of.
    fn discovery_layers(&self) -> Vec<LayerRoot> {
        let mut layers = Vec::with_capacity(3);
        if let Some(builtin_root) = &self.inner.builtin_root {
            layers.push(LayerRoot::new(builtin_root.clone(), FileSource::Builtin));
        }
        layers.push(LayerRoot::new(
            self.inner.user_root.clone(),
            FileSource::User,
        ));
        if let Some(project_root) = &self.inner.project_root {
            layers.push(LayerRoot::new(project_root.clone(), FileSource::Local));
        }
        layers
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

        // An `unload()` is a flush boundary for the types emitter: the
        // disposal above debounced the `server_unregistered` calls for every
        // server the plugin had registered, so flushing now settles them into
        // one `.d.ts` write the moment the unload completes.
        self.inner.types_emitter.flush();

        // Dropping the runtime tears the isolate's worker thread down.
        drop(plugin);
        Ok(())
    }

    /// Starts watching the host's writable plugin layers and reloading on
    /// change.
    ///
    /// Starts the `swissarmyhammer-directory` stack-aware
    /// [`Watcher`](swissarmyhammer_directory::Watcher) on the `plugins/`
    /// subdirectory of *each writable layer root the host was constructed
    /// with* — the user root and, when the host has one, the project root —
    /// then spawns a task that drains every watcher's
    /// [`StackedEvent`](swissarmyhammer_directory::StackedEvent) stream and
    /// reconciles the host against the disk on every event. Watching the
    /// host's own roots (rather than re-deriving roots from ambient XDG / git
    /// state) keeps hot reload watching the exact directories the host
    /// discovers from, which is also what makes the watcher test-isolatable.
    ///
    /// The returned [`PluginWatcher`] **must be kept alive**: dropping it stops
    /// the watchers and the drain task. The host should already have run
    /// [`discover_and_load_all`](Self::discover_and_load_all) so the watcher
    /// reconciles against a known baseline.
    ///
    /// # Type Parameters
    ///
    /// - `C` — the host's [`DirectoryConfig`]. It parameterizes the watcher and
    ///   the discovery rescan exactly as it parameterizes
    ///   [`discover_and_load_all`](Self::discover_and_load_all), so the platform
    ///   stays host-agnostic.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Runtime`] when a layer's underlying filesystem watcher
    /// cannot be started.
    pub async fn watch_plugins<C: DirectoryConfig + 'static>(&self) -> Result<PluginWatcher> {
        // The watcher's `StackedEvent`s only *trigger* a reconcile; the
        // reconcile itself re-runs discovery over the host's real roots, so
        // every layer's events are merged into one channel and the per-event
        // layer attribution is not relied upon.
        let (event_tx, event_rx) = mpsc::channel::<StackedEvent>(64);
        let mut watchers: Vec<Box<dyn std::any::Any + Send>> = Vec::new();

        for root in self.watch_roots() {
            let (watcher, mut receiver) = Watcher::<C>::watch_in(&root, PLUGINS_SUBDIR)
                .await
                .map_err(|error| {
                    Error::Runtime(format!(
                        "could not start the plugin watcher for {}: {error}",
                        root.display()
                    ))
                })?;
            watchers.push(Box::new(watcher));
            // Forward this layer's events into the single merged channel.
            let event_tx = event_tx.clone();
            tokio::spawn(async move {
                while let Some(event) = receiver.recv().await {
                    if event_tx.send(event).await.is_err() {
                        return;
                    }
                }
            });
        }

        let drain = self.spawn_drain::<C>(event_rx);
        Ok(PluginWatcher {
            _watchers: watchers,
            drain,
        })
    }

    /// The *writable* layer base directories this host watches for hot reload.
    ///
    /// These are the host's writable roots — the user root and, when present,
    /// the project root. The watcher creates and watches the `plugins/`
    /// subdirectory inside each. The read-only builtin layer is deliberately
    /// *not* watched: it ships frozen with the binary and never changes on disk
    /// under a running host, so watching it would only burn an OS watcher.
    /// `discover_and_load_all` still scans the builtin layer (via
    /// [`discovery_layers`](Self::discovery_layers)), and a watcher-driven
    /// reconcile re-runs that full scan — so a builtin copy still participates
    /// in layer precedence even though its directory is not itself watched.
    fn watch_roots(&self) -> Vec<PathBuf> {
        let mut roots = vec![self.inner.user_root.clone()];
        if let Some(project_root) = &self.inner.project_root {
            roots.push(project_root.clone());
        }
        roots
    }

    /// Spawns the task that drains the merged watcher event stream.
    ///
    /// The merged channel delivers a [`StackedEvent`] for every change in any
    /// watched layer. Each event is handled by re-reconciling the host against
    /// the disk; the task ends when every watcher is dropped and the channel
    /// closes.
    fn spawn_drain<C: DirectoryConfig + 'static>(
        &self,
        mut receiver: mpsc::Receiver<StackedEvent>,
    ) -> tokio::task::JoinHandle<()> {
        let host = self.clone();
        tokio::spawn(async move {
            while let Some(event) = receiver.recv().await {
                host.handle_stacked_event::<C>(event).await;
            }
        })
    }

    /// Handles one [`StackedEvent`] by reconciling the affected plugin id.
    ///
    /// The watcher's `name` is the on-disk *directory* name, which is the
    /// plugin's identity. Rather than trust the event's `name` directly, the
    /// host re-runs point-in-time discovery — the same scan
    /// [`discover_and_load_all`](Self::discover_and_load_all) uses — to learn
    /// the current highest-precedence copy of every id, then reconciles each
    /// plugin id whose state the event could have changed against what is
    /// currently active. The event's
    /// [`LayerChange`](swissarmyhammer_directory::LayerChange) only
    /// selects *how much* to reconcile:
    ///
    /// - `Added`/`Modified` — reconcile every id, because a directory rename
    ///   can move identity between directories.
    /// - `Removed` — likewise reconcile every id, since a removal can re-emerge
    ///   a shadowed lower-layer copy under a different directory name.
    ///
    /// Reconciliation per id then applies the precise load / reload / unload
    /// rules; see [`reconcile_id`](Self::reconcile_id).
    async fn handle_stacked_event<C: DirectoryConfig>(&self, event: StackedEvent) {
        let layer = describe_layer(event.change.layer());
        tracing::debug!(
            subdirectory = %event.subdirectory,
            name = %event.name,
            %layer,
            "plugin watcher event; reconciling against disk",
        );

        let discovered = match discover_plugins::<C>(&self.discovery_layers()) {
            Ok(discovered) => discovered,
            Err(error) => {
                // A broken bundle fails the scan; the host is left as it was
                // rather than guessing. The next event re-attempts the scan.
                tracing::warn!(%error, "plugin watcher rescan failed; host left unchanged");
                return;
            }
        };
        self.reconcile_all(discovered).await;
    }

    /// Reconciles every plugin id across the discovered set and the active
    /// set.
    ///
    /// The union of ids present on disk and ids currently active is the set
    /// the host must reconcile: an id only on disk is a load, an id only
    /// active is an unload, an id in both is a possible reload or a
    /// layer-precedence change. Each id is reconciled independently by
    /// [`reconcile_id`](Self::reconcile_id).
    async fn reconcile_all(&self, discovered: Vec<DiscoveredPlugin>) {
        // Index the scan by plugin id so each id is reconciled once.
        let on_disk: HashMap<String, DiscoveredPlugin> = discovered
            .into_iter()
            .map(|plugin| (plugin.id.clone(), plugin))
            .collect();

        // The active ids, snapshotted under the lock so the await below does
        // not hold it.
        let active_ids: Vec<String> = self.lock().active_plugins.keys().cloned().collect();

        let mut ids: Vec<String> = on_disk.keys().cloned().collect();
        for id in active_ids {
            if !on_disk.contains_key(&id) {
                ids.push(id);
            }
        }

        for id in ids {
            self.reconcile_id(&id, on_disk.get(&id)).await;
        }
    }

    /// Reconciles a single plugin id against its highest-precedence copy on
    /// disk.
    ///
    /// This is the translation of a [`StackedEvent`] into a lifecycle action,
    /// expressed as the difference between *what is active* and *what discovery
    /// now resolves as the winner*:
    ///
    /// - **Nothing active, a copy on disk** — equivalent to an `Added` that
    ///   becomes the highest-precedence layer: load it.
    /// - **A copy active, nothing on disk** — equivalent to a `Removed` of the
    ///   last layer: unload it.
    /// - **A copy active, the *same layer* still the winner** — the winner's
    ///   content fingerprint decides: if it differs from the active copy's
    ///   recorded fingerprint the active source was edited, so reload it in
    ///   place (the common save-while-running path); if the fingerprint is
    ///   unchanged the reconcile is a no-op. The fingerprint guard is what
    ///   keeps a `Modified` to a *shadowed* lower-layer copy from spuriously
    ///   reloading the active copy: a shadowed-copy change leaves the winning
    ///   copy's bytes — and so its fingerprint — untouched.
    /// - **A copy active, a *different layer* now the winner** — equivalent to
    ///   an `Added` of a higher layer or a `Removed` of the active layer that
    ///   re-emerged a lower one: unload the old copy, then load the new one.
    ///
    /// A copy on disk that is *not* the winner — a shadowed lower-layer copy —
    /// changes only the override stack and does not touch the active copy: it
    /// is recorded by virtue of the next reconcile picking it up if the
    /// shadowing copy ever disappears, and needs no action now. Because the
    /// watcher re-runs full discovery on every event and reconciles every id,
    /// this method *is* reached for an id whose shadowed copy changed — the
    /// fingerprint comparison above is what makes that reconcile a no-op.
    async fn reconcile_id(&self, id: &str, on_disk: Option<&DiscoveredPlugin>) {
        let active = self.lock().active_plugins.get(id).cloned();
        match (active, on_disk) {
            (None, None) => {}
            (None, Some(winner)) => {
                // Added: a new highest-precedence copy. Load it.
                self.load_active_copy(winner).await;
            }
            (Some(active), None) => {
                // Removed with no lower layer left: unload entirely.
                self.unload_active(id, &active.plugin_id).await;
            }
            (Some(active), Some(winner)) => {
                if winner.source == active.layer {
                    // The active layer is still the winner. Only a genuine
                    // change to the winning copy's own source warrants a
                    // reload: compare the freshly re-discovered winner's
                    // content fingerprint against the active copy's. An
                    // unchanged fingerprint means this event was about some
                    // other copy (a shadowed lower layer), so tearing the
                    // active isolate down would be a needless reload that
                    // discards its class-field state — skip it.
                    if PluginFingerprint::of(winner) == active.fingerprint {
                        tracing::debug!(
                            plugin = %id,
                            "reconcile: active copy unchanged; no reload",
                        );
                    } else {
                        self.reload_active(id, &active, winner).await;
                    }
                } else {
                    // A different layer is now the winner: a higher layer
                    // appeared, or the active layer was removed and a lower
                    // one re-emerged. Tear down the old copy, load the new.
                    self.unload_active(id, &active.plugin_id).await;
                    self.load_active_copy(winner).await;
                }
            }
        }
    }

    /// Loads the discovered `winner` as the active copy of its plugin id.
    ///
    /// Used for an `Added` reconcile and for the load half of a layer change.
    /// A successful load records the active copy and [`ReloadStatus::Healthy`];
    /// a failed load records [`ReloadStatus::Failed`] and leaves the id
    /// unloaded — there is no fallback, matching the failed-v2 contract.
    async fn load_active_copy(&self, winner: &DiscoveredPlugin) {
        let id = winner.id.clone();
        let fingerprint = PluginFingerprint::of(winner);
        let entry_file = winner.entry.to_string_lossy().into_owned();
        match self.load_resolved(&winner.directory, entry_file).await {
            Ok(plugin_id) => {
                self.record_active(&id, &plugin_id, winner.source.clone(), fingerprint);
                tracing::info!(plugin = %id, "plugin loaded by the watcher");
            }
            Err(error) => {
                self.record_failure(&id, &error);
                tracing::warn!(
                    plugin = %id,
                    %error,
                    "plugin failed to load on a watcher event; left unloaded",
                );
            }
        }
    }

    /// Reloads the active copy of `id` from the discovered `winner`.
    ///
    /// The reload mechanism is "dispose old, load new" for the same plugin
    /// id, reusing the existing [`unload`](Self::unload) and
    /// [`load_resolved`](Self::load_resolved) machinery:
    ///
    /// 1. The old isolate is torn down and every ledger registration disposed —
    ///    so any in-flight call into the old copy's servers fails (a disposed
    ///    server resolves [`Error::ServerUnavailable`]); a call already past
    ///    routing into the old isolate fails as the worker stops.
    /// 2. A fresh isolate is created, the source re-transpiled and re-loaded,
    ///    and the new `load()` run.
    ///
    /// Class-field state from the old copy is intentionally lost — a fresh
    /// isolate keeps nothing from the old one.
    ///
    /// A failed v2 load leaves the plugin unloaded and records
    /// [`ReloadStatus::Failed`].
    async fn reload_active(&self, id: &str, active: &ActivePlugin, winner: &DiscoveredPlugin) {
        // Capture the set of server names v1 holds and open a hot-reload
        // window covering them. The guard owns the marker lifetime: it marks
        // every name `Reloading` on construction and clears the markers on
        // drop. Drop runs on the normal return path, on a panic, AND on a
        // mid-await task cancellation (e.g. `PluginWatcher::drop` aborting
        // the drain task) — so an aborted reload never leaves a name
        // permanently `Reloading` and unreachable.
        //
        // v2's `register` calls of the same names clear individual markers
        // as those servers go live again; the guard's drop is idempotent
        // (the registry just has nothing more to remove).
        let names = self.server_names_held_by(&active.plugin_id);
        let _marker_guard = ReloadingMarkerGuard::new(self.inner.clone(), names);

        // Dispose the old copy: tear the isolate down, drain the ledger.
        self.unload_active(id, &active.plugin_id).await;
        // Load the new copy fresh. v2's `register` calls clear individual
        // markers for the names it re-registers; the guard's drop covers any
        // remaining ones.
        self.load_active_copy(winner).await;
    }

    /// Collects the set of server names a plugin currently holds in its ledger.
    ///
    /// Used by the reload path to stage the in-flight `Reloading` markers
    /// before v1's unregister runs. A plugin id with no ledger entry returns
    /// the empty vector — the caller treats "no recorded names" the same as
    /// "no servers to mark".
    fn server_names_held_by(&self, plugin_id: &PluginId) -> Vec<String> {
        self.lock()
            .ledger
            .server_names(plugin_id)
            .unwrap_or_default()
    }

    /// Unloads the active copy of `id` and drops its active record.
    ///
    /// Reuses [`unload`](Self::unload) for the isolate-and-ledger teardown,
    /// then removes the [`ActivePlugin`] entry so the id is no longer
    /// considered active. An [`Error::UnknownPlugin`] from `unload` — the
    /// plugin was already gone — is logged and ignored, because the goal
    /// state (id not active) is reached either way.
    async fn unload_active(&self, id: &str, plugin_id: &PluginId) {
        if let Err(error) = self.unload(plugin_id).await {
            tracing::debug!(
                plugin = %id,
                %error,
                "unloading the active copy during reconcile reported an error; continuing",
            );
        }
        self.lock().active_plugins.remove(id);
    }

    /// Records that a reload-path load of `id` failed.
    ///
    /// Drops any active record for the id — a failed load leaves nothing
    /// active — and stores [`ReloadStatus::Failed`] carrying the surfaced
    /// error so a caller can see the plugin needs a manual reload.
    fn record_failure(&self, id: &str, error: &Error) {
        let mut state = self.lock();
        state.active_plugins.remove(id);
        state.reload_status.insert(
            id.to_string(),
            ReloadStatus::Failed {
                error: error.to_string(),
            },
        );
    }

    /// Runs the plugin's optional `unload` lifecycle hook, ignoring failures.
    ///
    /// The host instantiated the bundle's default-exported `Plugin` subclass on
    /// load and stored the wrapped instance on the isolate; this drives the
    /// matching [`PluginLifecycle::Unload`] transition, which runs that stored
    /// instance's `unload()` hook. A plugin that does not override `unload()`
    /// inherits the base class's no-op-plus-`track`-cleanup default, so the call
    /// succeeds either way — overriding `unload()` is optional by contract. Any
    /// error the hook throws is logged at debug level rather than failing the
    /// unload, since host-side disposal must still run.
    ///
    /// The lifecycle call reuses the entry path the load transition was driven
    /// with — [`LoadedPlugin::entry_file`] — so the unload resolves against the
    /// *same* main module the isolate already loaded, rather than a re-derived
    /// path the isolate's module map would reject as a duplicate "main" module.
    async fn run_plugin_unload(&self, plugin_id: &PluginId, plugin: &LoadedPlugin) {
        let result = plugin
            .runtime
            .call_plugin_lifecycle(
                &plugin.bundle_dir,
                plugin.entry_file.as_str(),
                PluginLifecycle::Unload,
            )
            .await;
        if let Err(error) = result {
            tracing::debug!(
                plugin = %plugin_id.as_str(),
                %error,
                "plugin unload() hook failed; continuing host-side disposal"
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
                let outcome = self.lock().registry.unregister(&name);
                if matches!(outcome, UnregisterOutcome::NotRegistered) {
                    tracing::debug!(
                        plugin = %plugin_id.as_str(),
                        server = %name,
                        "ledger server handle had no live registration to dispose"
                    );
                }
                // Disposing a ledger server handle drops one caller's hold on
                // the server. The registry only removes it when the LAST
                // holder unregisters — that is the moment the emitter is
                // told. A `Decremented` outcome leaves the server live (and
                // its tools still reachable through the registered name) for
                // the remaining holders, so the emitter is left alone. Driven
                // after the host mutex is released; the unload that called
                // this then flushes the emitter so the burst settles into one
                // write.
                if matches!(outcome, UnregisterOutcome::Removed(_)) {
                    self.inner.types_emitter.server_unregistered(name);
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
    /// Returns [`Error::PluginReloaded`] when `server`'s backing plugin is
    /// mid-hot-reload (the caller should retry once v2 settles),
    /// [`Error::ServerUnavailable`] when `server` was registered and has
    /// since been disposed, [`Error::UnknownServer`] when `server` was
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
    /// Returns [`Error::PluginReloaded`] when `server`'s backing plugin is
    /// mid-hot-reload (the caller should retry once v2 settles),
    /// [`Error::ServerUnavailable`] when `server` was registered and has
    /// since been disposed, [`Error::UnknownServer`] when `server` was
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
            ServerStatus::Reloading => return Err(Error::PluginReloaded),
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

    /// Records an [`RegistrationHandle::Opaque`] dispose hook on `plugin_id`'s
    /// ledger.
    ///
    /// Higher-tier services — the command service in particular — wire
    /// per-plugin disposal here: when a plugin first creates a service-owned
    /// resource (a registered command, for example), the service installs an
    /// opaque hook so [`PluginHost::unload`]'s ledger drain calls it on the
    /// way out. The hook runs on the platform's unload path, after the
    /// plugin's own `unload()` and before the isolate is torn down, so it
    /// can free service-owned state in step with the host's authoritative
    /// cleanup.
    ///
    /// Multiple hooks for the same plugin are appended in registration order
    /// and drained last-first by [`PluginLedger::drain`], matching the
    /// disposal discipline for every other handle kind.
    ///
    /// Synchronous because the only work it performs is briefly holding the
    /// host's state mutex — no async I/O, no await. Verb handlers in
    /// service crates can therefore call it from sync contexts (the
    /// command-service register path does exactly this).
    ///
    /// # Parameters
    ///
    /// - `plugin_id` — the id of the plugin to attach the hook to.
    /// - `hook` — a one-shot dispose function the unload path will run.
    ///
    /// # Returns
    ///
    /// `true` when the hook was recorded; `false` when `plugin_id` is not
    /// currently tracked (the plugin is unloaded or was never loaded), in
    /// which case the hook is dropped to avoid creating an orphan ledger
    /// entry.
    pub fn record_unload_hook(&self, plugin_id: &PluginId, hook: Box<dyn FnOnce() + Send>) -> bool {
        let mut state = self.lock();
        state
            .ledger
            .record(plugin_id, RegistrationHandle::Opaque(hook))
    }

    /// Invokes a callback in a loaded plugin's isolate.
    ///
    /// Resolves `plugin_id` to its loaded [`PluginRuntime`], clones a
    /// [`crate::CallbackInvoker`] out under the host's state mutex (so the
    /// mutex is dropped before the awaited reply), and dispatches the
    /// `notifications/callbacks/invoke` to the worker thread. The callback's
    /// settled return value (or an isolate-level failure) is returned to the
    /// caller verbatim.
    ///
    /// This is the seam higher-tier services use to route a callback back to
    /// the registering plugin without depending on the [`PluginRuntime`]
    /// type or the host's private state.
    ///
    /// # Parameters
    ///
    /// - `plugin_id` — the id of the plugin whose isolate holds the callback.
    /// - `callback_id` — the SDK-assigned callback id (e.g. `"cb_42"`).
    /// - `args` — the positional arguments JSON, conventionally an array.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnknownPlugin`] when no plugin is loaded under
    /// `plugin_id`, [`Error::Runtime`] when the callback is missing or the
    /// stored function throws, [`Error::RuntimeTimeout`] when the worker
    /// does not answer within the command timeout, or [`Error::RuntimeStopped`]
    /// when the worker channel is closed.
    pub async fn invoke_plugin_callback(
        &self,
        plugin_id: &PluginId,
        callback_id: impl Into<String>,
        args: Value,
    ) -> Result<Value> {
        // Take the runtime's callback invoker out under the lock, then drop
        // the lock before awaiting the reply. The invoker owns its own
        // channel sender clone, so it survives the lock release.
        let invoker = self
            .lock()
            .plugins
            .get(plugin_id)
            .map(|plugin| plugin.runtime.callback_invoker())
            .ok_or(Error::UnknownPlugin)?;

        invoker.invoke(callback_id, args).await
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

/// RAII guard that owns the lifetime of `Reloading` markers across a reload.
///
/// Built by `reload_active` from the set of server names v1 holds. The
/// constructor takes the lock once, marks every name `Reloading`, and
/// returns the guard. The destructor takes the lock again and clears any
/// markers that are still set — names v2 re-registered during the reload
/// window cleared their own markers via `ServerRegistry::register`, so the
/// drop is idempotent against those.
///
/// Drop runs on:
///
/// - **Normal return** — the markers cleared after `load_active_copy`
///   resolves.
/// - **Panic** — markers cleared as the panic unwinds through
///   `reload_active`.
/// - **Task cancellation** — when an outer holder of the drain task
///   (`PluginWatcher`) calls `.abort()`, every `await` in the task may be
///   torn down. The guard's stack slot is dropped synchronously as part of
///   that teardown, so the markers do not get stuck.
///
/// Without this guard, an abort between mark-reloading and clear-reloading
/// would leave names permanently resolving as `Reloading` — every
/// subsequent call would return `Error::PluginReloaded` with no in-crate
/// way to recover.
struct ReloadingMarkerGuard {
    /// Shared host inner — held so the guard's drop can re-acquire the
    /// state lock without going through `&PluginHost`.
    inner: Arc<HostInner>,
    /// The names that were marked `Reloading` at construction, in the
    /// order the ledger reports them. Cleared on drop.
    names: Vec<String>,
}

impl ReloadingMarkerGuard {
    /// Marks every name in `names` as in the hot-reload window and returns
    /// a guard whose drop clears the markers.
    fn new(inner: Arc<HostInner>, names: Vec<String>) -> Self {
        if !names.is_empty() {
            let mut state = inner.state.lock().expect("host state mutex poisoned");
            for name in &names {
                state.registry.mark_reloading(name);
            }
        }
        Self { inner, names }
    }
}

impl Drop for ReloadingMarkerGuard {
    fn drop(&mut self) {
        if self.names.is_empty() {
            return;
        }
        // A poisoned host mutex is unrecoverable — every other lock call
        // also panics — so unwrap is consistent with the rest of the host.
        let mut state = self.inner.state.lock().expect("host state mutex poisoned");
        for name in &self.names {
            state.registry.clear_reloading(name);
        }
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
/// `url` server — are spawned as a task on the host's one long-lived
/// [`bridge_runtime`](HostInner::bridge_runtime) and the worker blocks, with a
/// [`BRIDGE_TIMEOUT`] bound, for the reply.
///
/// Routing every bridge call onto that single persistent runtime — rather than
/// a per-call throwaway — is what keeps the `cli`/`url` transports alive: a
/// [`CliServer`]/[`UrlServer`] connected during a `register` call holds an
/// `rmcp` `RunningService` whose background service loop is a task on the
/// runtime that drove the `connect`. Because that runtime is the host's own
/// and outlives every bridge call, the service loop is still running when a
/// later `toolsCall` reaches the peer.
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
    /// The bridge op runs on the plugin's isolate worker thread, which is
    /// itself already inside that worker's own `block_on` — so the host's
    /// async work can neither nest there nor make the op async without a
    /// `deno_core` seam change. Instead the future is spawned as a task on the
    /// host's one long-lived [`bridge_runtime`](HostInner::bridge_runtime) and
    /// its result is sent back over a channel; the worker thread blocks on that
    /// channel with a [`BRIDGE_TIMEOUT`] bound, so a host that never answers
    /// fails the call rather than wedging the isolate worker forever.
    ///
    /// Because the runtime is the host's own — created once at construction
    /// and never dropped per call — any `rmcp` `RunningService` background loop
    /// a `cli`/`url` transport spawns on it survives across bridge calls.
    fn block_on<F, T>(&self, future: F) -> std::result::Result<T, String>
    where
        F: std::future::Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        let (tx, rx) = std::sync::mpsc::channel();
        // Spawn onto the host's persistent runtime — never a per-call runtime
        // — so transport service loops spawned by this future keep running
        // after the call returns. The worker thread is the receiver below.
        self.host.inner.bridge_runtime.handle().spawn(async move {
            let _ = tx.send(future.await);
        });

        // The isolate worker thread blocks here, but bounded: a host task that
        // never answers — a wedged future, a dropped sender — becomes a prompt
        // error instead of a hung worker. The spawned task is detached; on a
        // timeout it keeps running on the bridge runtime and simply finds the
        // receiver gone when it finally tries to send.
        rx.recv_timeout(BRIDGE_TIMEOUT)
            .map_err(|_| "host did not answer the plugin's bridge call in time".to_string())
    }

    /// Handles a `toolsList` envelope: lists a registered server's tools.
    fn tools_list(&self, payload: &Value) -> std::result::Result<Value, String> {
        let server = envelope_str(payload, "server")?;
        let host = self.host.clone();
        let tools = self.block_on(async move {
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

    /// Handles a `toolsCall` envelope: records any callback markers in the
    /// plugin's ledger, then routes the call at the registry.
    ///
    /// The SDK's `toolsCall` envelope carries an arguments map that has already
    /// passed through `marshalCallbacks`: any function value anywhere in the
    /// payload was replaced with a `{ "$callback": "cb_..." }` marker before
    /// the call left the isolate. The host treats those markers as opaque
    /// handles — it does not invoke them here, the target tool does — but
    /// every marker id is recorded as a [`RegistrationHandle::Callback`] in
    /// the plugin's ledger so that [`PluginHost::unload`] disposes the stored
    /// functions on the way down. A `tools/call` whose arguments carry no
    /// markers (the common verbatim path to a URL- or CLI-sourced server)
    /// adds nothing to the ledger and is otherwise unchanged.
    fn tools_call(&self, payload: &Value) -> std::result::Result<Value, String> {
        let server = envelope_str(payload, "server")?;
        let tool = envelope_str(payload, "tool")?;
        let arguments = payload
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| Value::Object(Map::new()));

        // Record every callback marker reachable in the arguments map so the
        // plugin's ledger can drain them on unload. The collect-then-record
        // split keeps the ledger mutation under a single lock and out of the
        // routing path's async hot loop.
        let mut callback_ids = Vec::new();
        collect_callback_ids(&arguments, &mut callback_ids);
        if !callback_ids.is_empty() {
            let mut state = self.host.lock();
            for id in callback_ids {
                // The plugin is tracked from `load`, so this append cannot orphan.
                state.ledger.record(
                    &self.plugin_id,
                    RegistrationHandle::Callback(CallbackId::new(id)),
                );
            }
        }

        let host = self.host.clone();
        let caller = CallerId::Plugin(self.plugin_id.clone());
        // The call is routed at the live registry and attributed to the plugin
        // this bridge is scoped to.
        self.block_on(async move { host.route(caller, &server, &tool, arguments).await })?
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
        self.block_on(async move { host.connect_and_register(&plugin_id, name, source).await })?
            .map_err(|error| error.to_string())?;
        Ok(Value::Object(Map::new()))
    }

    /// Handles an `unregister` envelope: drops one caller's hold on a server
    /// and consumes its ledger entry.
    ///
    /// Refcounted unregister: the registration is torn down only when the
    /// LAST holder is gone. A `Decremented` outcome leaves the live server
    /// reachable for the remaining holders; a `Removed` outcome is the one
    /// that drives the types-emitter `server_unregistered` event.
    ///
    /// A plugin unregistering a name it never registered — or already
    /// unregistered — is not an error, but it is a sign of a buggy plugin, so
    /// the case is logged at debug level. This mirrors the diagnostic
    /// [`PluginHost::dispose_handle`] emits when a ledger handle has no live
    /// registration to dispose.
    fn unregister(&self, payload: &Value) -> std::result::Result<Value, String> {
        let name = envelope_str(payload, "name")?;
        let server_was_removed;
        {
            let mut state = self.host.lock();
            let outcome = state.registry.unregister(&name);
            let consumed = state.ledger.consume_server(&self.plugin_id, &name);
            server_was_removed = matches!(outcome, UnregisterOutcome::Removed(_));
            let had_registration = !matches!(outcome, UnregisterOutcome::NotRegistered);
            if !had_registration || !consumed {
                tracing::debug!(
                    plugin = %self.plugin_id.as_str(),
                    server = %name,
                    had_registration,
                    had_ledger_entry = consumed,
                    "plugin unregistered a server it did not have registered"
                );
            }
        }

        // The registry tore a server down: keep the generated `.d.ts` in sync.
        // A `Decremented` outcome leaves the server live — no event — so the
        // emitter is only told when the last caller has unregistered. Driven
        // after the host mutex is released so the emitter's own lock never
        // nests under it.
        if server_was_removed {
            self.host.inner.types_emitter.server_unregistered(name);
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
    /// # Idempotent share fast-path
    ///
    /// Two plugins that both register the same `(name, source)` must NOT
    /// pay the cost of a second `connect_source` — for a `{ cli }` source
    /// that would spawn a duplicate subprocess; for a `{ rust }` source the
    /// already-activated module would be missing from the available-modules
    /// table and the second activation would fail. So before connecting,
    /// the host checks the registry: if `name` is already live with a
    /// structurally-equal source, the call records the ledger entry, bumps
    /// the registry refcount, and returns — no new connect, no duplicate
    /// types-emitter event.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ServerNameTaken`] when `name` is already registered
    /// against a DIFFERENT source, [`Error::UnknownServer`] when a `rust` id
    /// names no exposed module, or [`Error::ServerUnavailable`] when a `cli`
    /// or `url` source cannot be connected or the source shape is not one of
    /// the three kinds.
    async fn connect_and_register(
        &self,
        plugin_id: &PluginId,
        name: ServerName,
        source: Value,
    ) -> Result<()> {
        let parsed_source = ServerSource::from_json(&source).ok_or(Error::ServerUnavailable)?;

        // Fast path: the registry already holds a structurally-equal source
        // under `name`. Skip `connect_source` — its side effects (spawning a
        // subprocess, consuming the activate-once `{ rust }` module) must not
        // run twice for one shared registration.
        {
            let mut state = self.lock();
            match state.registry.source_for(&name) {
                Some(existing) if existing == &parsed_source => {
                    // Share path: bump the refcount through `register`, which
                    // also handles a tombstone/reloading marker the same way
                    // the slow path would. The supplied `server` is irrelevant
                    // here — the registry keeps the already-live one — so any
                    // `Arc<dyn McpServer>` clone of the live server is passed
                    // to satisfy the API, and the outcome is verified to be
                    // `AlreadyRegistered` rather than panicked-asserted to
                    // keep this branch a strict no-op on the live server.
                    let live = state
                        .registry
                        .get(&name)
                        .expect("source_for matched, so a live server is present");
                    let outcome =
                        state
                            .registry
                            .register(name.clone(), parsed_source.clone(), live)?;
                    debug_assert!(
                        matches!(outcome, RegisterOutcome::AlreadyRegistered),
                        "a matching source must produce AlreadyRegistered"
                    );
                    // The plugin is tracked from `load`, so this append cannot orphan.
                    state
                        .ledger
                        .record(plugin_id, RegistrationHandle::Server(name));
                    return Ok(());
                }
                Some(_) => {
                    // Name is live with a DIFFERENT source — surface the
                    // collision now without spending the connect budget.
                    return Err(Error::ServerNameTaken(name));
                }
                None => {
                    // Vacant or tombstoned — drop the lock and connect.
                }
            }
        }

        let server = self.connect_source(&source).await?;

        // Snapshot the server's tools before it is moved into the registry so
        // the types emitter can be told the new server's namespace — done
        // outside the host mutex below.
        let tools = server.tools();

        let registered_fresh = {
            let mut state = self.lock();
            // Re-check under the lock: another plugin may have raced this
            // call. `register` itself handles all three cases (vacant, same
            // source — refcount bump, different source — error).
            let outcome = state
                .registry
                .register(name.clone(), parsed_source, server)?;
            // The plugin is tracked from `load`, so this append cannot orphan.
            state
                .ledger
                .record(plugin_id, RegistrationHandle::Server(name.clone()));
            matches!(outcome, RegisterOutcome::Registered)
        };

        // The registry gained a server: keep the generated `.d.ts` in sync.
        // Only emit on a FRESH registration; a raced `AlreadyRegistered`
        // share dropped the server we connected — its tools are already
        // reflected in the emitter from the original registration.
        // The emitter is internally synchronized, so this is called after the
        // host mutex is dropped — the debounce collapses a `load()` burst of
        // registrations into a single write at the flush boundary.
        if registered_fresh {
            self.inner.types_emitter.server_registered(name, tools);
        }
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

/// The isolate cwd for a host that does not serve a single board.
///
/// The test and dev-mode constructors model the boardless posture, where the
/// process CWD *is* the right working directory; this returns it, falling back
/// to the system temp dir if the process has no readable cwd. A per-board host
/// never uses this — it is built through [`PluginHost::new`] with its board dir
/// passed explicitly.
fn default_isolate_cwd() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir())
}

/// Renders a [`FileSource`] as a short label for watcher log lines.
fn describe_layer(source: &FileSource) -> &'static str {
    match source {
        FileSource::Builtin => "builtin",
        FileSource::User => "user",
        FileSource::Local => "project",
        FileSource::Dynamic => "dynamic",
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

    /// A background task spawned on the host's bridge runtime during one
    /// `HostBridge::block_on` call is still running for a *later* `block_on`
    /// call — the long-lived-runtime invariant the `cli`/`url` transports
    /// depend on.
    ///
    /// This is the unit-level analogue of the T21 e2e tests: a `cli`/`url`
    /// transport's `rmcp` `RunningService` loop is exactly such a background
    /// task, spawned during a `register` call's `block_on`. If the runtime
    /// were per-call — the bug this fixes — the task would die when the
    /// `register` call returned and the second `block_on` would observe a
    /// counter that never advanced. Routing both calls onto the host's one
    /// persistent runtime keeps the task running, so the counter climbs.
    #[test]
    fn a_task_spawned_in_one_bridge_call_outlives_it() {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::sync::Arc;

        let host = super::PluginHost::for_tests(
            std::env::temp_dir().join("plugin-host-runtime-invariant-user"),
            None,
        );
        let bridge = super::HostBridge::new(host.clone(), super::PluginId::new("test-plugin"));

        // First bridge call: spawn a detached background task on the host's
        // runtime that ticks a shared counter forever. A per-call runtime
        // would be dropped the instant this `block_on` returns, killing it.
        let ticks = Arc::new(AtomicU64::new(0));
        let task_ticks = Arc::clone(&ticks);
        bridge
            .block_on(async move {
                tokio::spawn(async move {
                    loop {
                        task_ticks.fetch_add(1, Ordering::SeqCst);
                        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
                    }
                });
            })
            .expect("the first bridge call must run on the host runtime");

        // A second, independent bridge call. It only has to reach the same
        // live runtime; sleeping inside it gives the background task time to
        // tick. If the runtime had been torn down with the first call, the
        // task would be gone and the counter frozen at its first-call value.
        bridge
            .block_on(async {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            })
            .expect("the second bridge call must run on the same host runtime");

        assert!(
            ticks.load(Ordering::SeqCst) > 0,
            "a task spawned during the first bridge call must still be \
             running for the second — the host runtime outlives every call"
        );
    }

    /// A host built with a builtin layer root reports its discovery layers as
    /// builtin → user → project: the read-only builtin layer is the
    /// lowest-precedence floor, tagged [`FileSource::Builtin`], with the
    /// writable user and project layers stacked on top.
    #[test]
    fn discovery_layers_stack_builtin_below_user_and_project() {
        use swissarmyhammer_directory::FileSource;

        let base = std::env::temp_dir().join("plugin-host-discovery-layers");
        let host = super::PluginHost::for_tests_with_builtin(
            base.join("builtin"),
            base.join("user"),
            Some(base.join("project")),
        );

        let layers = host.discovery_layers();
        let sources: Vec<FileSource> = layers.iter().map(|l| l.source.clone()).collect();
        assert_eq!(
            sources,
            vec![FileSource::Builtin, FileSource::User, FileSource::Local],
            "discovery must scan builtin → user → project, lowest precedence first"
        );
        assert_eq!(
            layers[0].root,
            base.join("builtin"),
            "the first (lowest-precedence) layer must be the builtin root"
        );
    }

    /// A host built with [`for_tests`](super::PluginHost::for_tests) has no
    /// builtin layer: `discovery_layers` reports only the user layer, so the
    /// existing two-layer callers are unaffected.
    #[test]
    fn for_tests_host_has_no_builtin_layer() {
        use swissarmyhammer_directory::FileSource;

        let host = super::PluginHost::for_tests(
            std::env::temp_dir().join("plugin-host-no-builtin-user"),
            None,
        );

        let layers = host.discovery_layers();
        let sources: Vec<FileSource> = layers.iter().map(|l| l.source.clone()).collect();
        assert_eq!(
            sources,
            vec![FileSource::User],
            "a `for_tests` host with no project layer discovers only the user layer"
        );
    }
}
