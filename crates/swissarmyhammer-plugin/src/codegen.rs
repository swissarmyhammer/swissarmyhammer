//! Code generation for plugin scaffolding and bindings.
//!
//! This module owns the [`TypesEmitter`]: the host-side component that keeps a
//! generated TypeScript declaration file (`.d.ts`) in sync with the live
//! [`ServerRegistry`](crate::registry::ServerRegistry). Plugin authors get
//! editor autocomplete for every registered server's tools with no build step
//! and no CLI to invoke — the host maintains the file automatically as servers
//! register, unregister, and change their tool sets.
//!
//! # Mechanism
//!
//! The host feeds the emitter [registry events](TypesEmitter#registry-events)
//! by direct method call — a server registered, a server unregistered, a
//! server's tools changed, a plugin lifecycle flush boundary. The emitter keeps
//! a snapshot of every registered server's tools and, on each event, schedules
//! a debounced regeneration of the `.d.ts` file. The debounce (~100ms) collapses
//! the burst of `server_registered` calls a plugin makes during `load()` into a
//! single file write. The write itself is atomic — the text is written to a
//! sibling temp file and then renamed over the destination — so a language
//! server watching the file never observes a half-written declaration.
//!
//! # What gets emitted
//!
//! One nested namespace per registered server, hung on an `App` interface. For
//! each tool the emitter walks its [`Tool`](rmcp::model::Tool) definition:
//!
//! - A **flat tool** — one with no `io.swissarmyhammer/operations` entry in its
//!   `_meta` — emits a single method named for the tool, whose input type is
//!   derived from the tool's `inputSchema`.
//! - An **operation tool** — one whose `_meta` carries the operations tree —
//!   emits one `<noun>.<verb>(input)` method per leaf of that tree, with the
//!   `input` type built from the leaf's `parameters` map.
//!
//! The emission is pure metadata → types copying: the `_meta` tree's shape is
//! mirrored verbatim into the namespace shape. There is no schema inference and
//! no runtime coupling — a stale `.d.ts` degrades only to a missing-autocomplete
//! or a spurious red squiggle, never to an incorrect call.
//!
//! # Production vs development
//!
//! Generated types are a development convenience. The emitter writes the file
//! only when its dev-mode flag is set; a production host (flag off) regenerates
//! the in-memory text but writes nothing to disk.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Map, Value};

use crate::registry::ServerName;
use crate::server::ToolMetadata;

/// The default path the generated declaration file is written to.
///
/// Relative to the active plugin development directory. The emitter's output
/// path is configurable at construction; this is the value
/// [`TypesEmitter::new`] uses when an embedder does not override it.
pub const DEFAULT_TYPES_PATH: &str = ".swissarmyhammer/types/app.d.ts";

/// The `_meta` key under which an operation tool carries its operations tree.
///
/// A [`Tool`](rmcp::model::Tool) whose `_meta` map contains this key is an
/// *operation tool*: its value is the noun → verb → `{op, description,
/// parameters}` tree produced by `generate_operations_meta`. A tool without
/// this key is a *flat tool*.
const OPERATIONS_META_KEY: &str = "io.swissarmyhammer/operations";

/// The `_meta` key carrying a tool's notification tree.
///
/// A [`Tool`](rmcp::model::Tool) whose `_meta` map contains this key declares
/// notifications a plugin can subscribe to: its value is the
/// `event → {method, description, parameters}` tree produced by
/// `generate_notifications_meta`. The emitter renders each event as a typed
/// `on(event, cb)` overload on the server namespace.
const NOTIFICATIONS_META_KEY: &str = "io.swissarmyhammer/notifications";

/// How long the emitter waits after the last registry event before it
/// regenerates the declaration file.
///
/// A plugin's `load()` typically registers several servers in a burst; without
/// a debounce each registration would trigger its own file write. Holding for
/// this window after the *last* event in a burst collapses the burst into a
/// single regeneration.
const DEBOUNCE: Duration = Duration::from_millis(100);

/// A registry event the host feeds the [`TypesEmitter`].
///
/// The host owns the live [`ServerRegistry`](crate::registry::ServerRegistry),
/// so it is the host that notices a server registering, a server unregistering,
/// a server's tool set changing, and a plugin lifecycle boundary. Each such
/// change is delivered to the emitter as one of these events, either through
/// [`TypesEmitter::handle`] or through the convenience methods that wrap it.
#[derive(Debug, Clone)]
pub enum RegistryEvent {
    /// A server was registered under `name`, advertising `tools`.
    ///
    /// The emitter records the server's tools and schedules a regeneration so
    /// the new server's namespace appears in the declaration file.
    ServerRegistered {
        /// The name the server was registered under.
        name: ServerName,
        /// The tools the server advertised at registration, as a `tools/list`
        /// would return them.
        tools: Vec<ToolMetadata>,
    },

    /// The server registered under `name` was unregistered.
    ///
    /// The emitter drops the server from its snapshot and schedules a
    /// regeneration so the server's namespace disappears from the file.
    ServerUnregistered {
        /// The name of the server that was unregistered.
        name: ServerName,
    },

    /// The server registered under `name` changed its tool set.
    ///
    /// Mirrors an MCP `notifications/tools/list_changed` for one server: the
    /// emitter replaces that server's recorded tools with `tools` and schedules
    /// a regeneration.
    ToolsChanged {
        /// The name of the server whose tools changed.
        name: ServerName,
        /// The server's current tool set, as a fresh `tools/list` returns it.
        tools: Vec<ToolMetadata>,
    },

    /// A plugin finished loading or unloading — a flush boundary.
    ///
    /// The emitter regenerates immediately, collapsing any regeneration the
    /// debounce window still holds pending into one write at the boundary.
    Flush,
}

/// The mutable state of a [`TypesEmitter`], guarded by one mutex.
struct EmitterState {
    /// The snapshot of every registered server's tools, keyed by server name.
    ///
    /// A [`BTreeMap`] so the emitted namespaces have a stable, deterministic
    /// order regardless of registration order — a determinism the
    /// declaration-file tests and a diff-friendly output both rely on.
    servers: BTreeMap<ServerName, Vec<ToolMetadata>>,
}

/// The shared inner state of a [`TypesEmitter`].
///
/// Held behind an [`Arc`] so a debounce task spawned for one event can outlive
/// the call that scheduled it and still reach the emitter's state, output path,
/// and dev-mode flag.
struct EmitterInner {
    /// The path the declaration file is written to.
    output_path: PathBuf,

    /// Whether the emitter writes the file to disk.
    ///
    /// `true` in development — the file is written on every regeneration.
    /// `false` in production — the text is still generated in memory, but
    /// nothing is written to disk.
    dev_mode: bool,

    /// The registered-server snapshot, behind a mutex so registry events and a
    /// firing debounce task serialize their access.
    state: Mutex<EmitterState>,

    /// A monotonically increasing generation counter.
    ///
    /// Every registry event bumps this and captures the new value; the debounce
    /// task it spawns writes the file only if the counter is unchanged when its
    /// window elapses. A later event therefore cancels an earlier event's
    /// pending write, which is what makes a burst collapse into one write.
    generation: AtomicU64,

    /// Counts how many times the file has actually been written to disk.
    ///
    /// Exposed through [`TypesEmitter::write_count`] so a test can assert the
    /// debounce collapsed a burst into a single write.
    write_count: AtomicU64,
}

/// The host component that keeps a generated `.d.ts` file in sync with the
/// live server registry.
///
/// A `TypesEmitter` is cheap to clone — every clone shares one underlying
/// snapshot, one output path, and one dev-mode flag, so the host can hand
/// clones to wherever registry events originate. See the [module
/// documentation](self) for the debounce, atomic-write, and emission mechanics.
///
/// # Registry events
///
/// The host drives the emitter by calling, for each change to the registry,
/// one of [`server_registered`](Self::server_registered),
/// [`server_unregistered`](Self::server_unregistered),
/// [`tools_changed`](Self::tools_changed), or [`flush`](Self::flush) — or the
/// general [`handle`](Self::handle) with a [`RegistryEvent`]. Every one of
/// these schedules a debounced regeneration; `flush` forces it immediately.
#[derive(Clone)]
pub struct TypesEmitter {
    /// The shared emitter state.
    inner: Arc<EmitterInner>,
}

impl std::fmt::Debug for TypesEmitter {
    /// Reports the output path, the dev-mode flag, and the registered-server
    /// count — the emitter's meaningful, printable state.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let server_count = self
            .inner
            .state
            .lock()
            .expect("types emitter state mutex poisoned")
            .servers
            .len();
        f.debug_struct("TypesEmitter")
            .field("output_path", &self.inner.output_path)
            .field("dev_mode", &self.inner.dev_mode)
            .field("servers", &server_count)
            .finish()
    }
}

impl TypesEmitter {
    /// Creates an emitter writing to [`DEFAULT_TYPES_PATH`] under `base_dir`.
    ///
    /// Most embedders want the default `.swissarmyhammer/types/app.d.ts`
    /// location; this constructor joins that relative path onto the plugin
    /// development directory the embedder supplies. Use
    /// [`with_output_path`](Self::with_output_path) when the file belongs
    /// somewhere else.
    ///
    /// # Parameters
    ///
    /// - `base_dir` — the plugin development directory the default types path
    ///   is resolved against.
    /// - `dev_mode` — `true` to write the file on every regeneration, `false`
    ///   to generate the text but write nothing (the production posture).
    pub fn new(base_dir: impl AsRef<Path>, dev_mode: bool) -> Self {
        Self::with_output_path(base_dir.as_ref().join(DEFAULT_TYPES_PATH), dev_mode)
    }

    /// Creates an emitter writing to an explicit `output_path`.
    ///
    /// # Parameters
    ///
    /// - `output_path` — the exact path the declaration file is written to.
    /// - `dev_mode` — `true` to write the file on every regeneration, `false`
    ///   to generate the text but write nothing.
    pub fn with_output_path(output_path: impl Into<PathBuf>, dev_mode: bool) -> Self {
        Self {
            inner: Arc::new(EmitterInner {
                output_path: output_path.into(),
                dev_mode,
                state: Mutex::new(EmitterState {
                    servers: BTreeMap::new(),
                }),
                generation: AtomicU64::new(0),
                write_count: AtomicU64::new(0),
            }),
        }
    }

    /// The path the declaration file is written to.
    pub fn output_path(&self) -> &Path {
        &self.inner.output_path
    }

    /// Whether this emitter writes the declaration file to disk.
    ///
    /// `true` in development, `false` in production — see [`new`](Self::new).
    pub fn is_dev_mode(&self) -> bool {
        self.inner.dev_mode
    }

    /// How many times the declaration file has actually been written to disk.
    ///
    /// A burst of registry events inside one debounce window collapses to a
    /// single increment; a production emitter (dev mode off) never increments
    /// this because it writes nothing.
    pub fn write_count(&self) -> u64 {
        self.inner.write_count.load(Ordering::SeqCst)
    }

    /// Records that the server `name` registered, advertising `tools`.
    ///
    /// Convenience wrapper over [`handle`](Self::handle) with a
    /// [`RegistryEvent::ServerRegistered`].
    ///
    /// # Parameters
    ///
    /// - `name` — the name the server was registered under.
    /// - `tools` — the tools the server advertised, as `tools/list` returns.
    pub fn server_registered(&self, name: impl Into<ServerName>, tools: Vec<ToolMetadata>) {
        self.handle(RegistryEvent::ServerRegistered {
            name: name.into(),
            tools,
        });
    }

    /// Records that the server `name` was unregistered.
    ///
    /// Convenience wrapper over [`handle`](Self::handle) with a
    /// [`RegistryEvent::ServerUnregistered`].
    ///
    /// # Parameters
    ///
    /// - `name` — the name of the server that was unregistered.
    pub fn server_unregistered(&self, name: impl Into<ServerName>) {
        self.handle(RegistryEvent::ServerUnregistered { name: name.into() });
    }

    /// Records that the server `name` changed its tool set to `tools`.
    ///
    /// Convenience wrapper over [`handle`](Self::handle) with a
    /// [`RegistryEvent::ToolsChanged`] — the emitter's response to an MCP
    /// `notifications/tools/list_changed` for one server.
    ///
    /// # Parameters
    ///
    /// - `name` — the name of the server whose tools changed.
    /// - `tools` — the server's current tool set, freshly listed.
    pub fn tools_changed(&self, name: impl Into<ServerName>, tools: Vec<ToolMetadata>) {
        self.handle(RegistryEvent::ToolsChanged {
            name: name.into(),
            tools,
        });
    }

    /// Flushes any pending regeneration, writing the file immediately.
    ///
    /// A plugin finishing `load()` or `unload()` is a flush boundary: rather
    /// than wait out the debounce window, the host calls this so the file is
    /// in sync the moment the lifecycle action completes. Convenience wrapper
    /// over [`handle`](Self::handle) with a [`RegistryEvent::Flush`].
    pub fn flush(&self) {
        self.handle(RegistryEvent::Flush);
    }

    /// Applies a [`RegistryEvent`] and schedules (or forces) a regeneration.
    ///
    /// A `ServerRegistered`, `ServerUnregistered`, or `ToolsChanged` mutates
    /// the registered-server snapshot and arms the ~100ms debounce: the file is
    /// regenerated once the events stop arriving. A `Flush` regenerates
    /// immediately, collapsing any debounce-pending write into the flush.
    ///
    /// This method does not block — the debounced write runs on a spawned task
    /// — and so must be called from within a Tokio runtime.
    ///
    /// # Parameters
    ///
    /// - `event` — the registry change to apply.
    pub fn handle(&self, event: RegistryEvent) {
        let force = matches!(event, RegistryEvent::Flush);
        {
            let mut state = self
                .inner
                .state
                .lock()
                .expect("types emitter state mutex poisoned");
            match event {
                RegistryEvent::ServerRegistered { name, tools }
                | RegistryEvent::ToolsChanged { name, tools } => {
                    state.servers.insert(name, tools);
                }
                RegistryEvent::ServerUnregistered { name } => {
                    state.servers.remove(&name);
                }
                RegistryEvent::Flush => {}
            }
        }

        if force {
            self.regenerate();
        } else {
            self.schedule_debounced();
        }
    }

    /// Arms a debounced regeneration, cancelling any earlier pending one.
    ///
    /// Bumps the generation counter and spawns a task that sleeps out the
    /// [`DEBOUNCE`] window, then regenerates only if no later event bumped the
    /// counter again. A burst of events therefore yields a single write: every
    /// event but the last has its task cancelled by a successor's bump.
    fn schedule_debounced(&self) {
        let scheduled = self.inner.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let inner = Arc::clone(&self.inner);
        tokio::spawn(async move {
            tokio::time::sleep(DEBOUNCE).await;
            // Write only if this task still owns the latest generation; a later
            // event would have bumped the counter past `scheduled`.
            if inner.generation.load(Ordering::SeqCst) == scheduled {
                Self::regenerate_inner(&inner);
            }
        });
    }

    /// Regenerates the declaration file now, bypassing the debounce.
    ///
    /// Bumps the generation counter so any debounce task still pending is
    /// cancelled — its write would be redundant with this one — and then
    /// regenerates synchronously.
    fn regenerate(&self) {
        self.inner.generation.fetch_add(1, Ordering::SeqCst);
        Self::regenerate_inner(&self.inner);
    }

    /// Renders the declaration text from the current snapshot and, in dev mode,
    /// writes it atomically to the output path.
    ///
    /// In production (dev mode off) the text is rendered but not written — the
    /// generated types are a development convenience with no runtime role.
    fn regenerate_inner(inner: &EmitterInner) {
        let text = {
            let state = inner
                .state
                .lock()
                .expect("types emitter state mutex poisoned");
            render_declaration(&state.servers)
        };

        if !inner.dev_mode {
            return;
        }

        if let Err(error) = atomic_write(&inner.output_path, &text) {
            // A failed types write is never fatal: generated types are a
            // development aid, decoupled from runtime. Log and carry on rather
            // than propagate — the host's lifecycle must not hinge on a `.d.ts`.
            tracing::warn!(
                path = %inner.output_path.display(),
                %error,
                "failed to write generated plugin types",
            );
            return;
        }
        inner.write_count.fetch_add(1, Ordering::SeqCst);
    }
}

/// Writes `contents` to `path` atomically, via a temp file and a rename.
///
/// The text is written in full to a sibling temp file, which is then renamed
/// over `path`. A rename within one directory is atomic on every platform the
/// host targets, so a reader — a TypeScript language server watching the file —
/// observes either the old file or the complete new one, never a partial write.
/// The temp file is removed on a write failure so a failed regeneration leaves
/// no debris next to the destination.
///
/// # Parameters
///
/// - `path` — the destination path; its parent directory is created if absent.
/// - `contents` — the full declaration text to write.
///
/// # Errors
///
/// Returns the underlying [`std::io::Error`] when the parent directory cannot
/// be created, the temp file cannot be written, or the rename fails.
fn atomic_write(path: &Path, contents: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // The temp file is a sibling of the destination so the rename stays within
    // one directory and is therefore atomic. Its name carries the emitter's
    // process id and a generation counter so two concurrent writers — or two
    // emitters sharing a directory — never collide on the temp path.
    let temp_path = temp_sibling(path);

    if let Err(error) = std::fs::write(&temp_path, contents) {
        // Best-effort cleanup so a failed write leaves no orphan temp file.
        let _ = std::fs::remove_file(&temp_path);
        return Err(error);
    }

    if let Err(error) = std::fs::rename(&temp_path, path) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(error);
    }

    Ok(())
}

/// Builds a unique temp-file path that is a sibling of `path`.
///
/// The temp file must share a directory with the destination so the rename
/// that publishes it is a same-directory rename — the only kind guaranteed
/// atomic. The name is unique per call so concurrent regenerations do not
/// stomp each other's temp files.
fn temp_sibling(path: &Path) -> PathBuf {
    /// A per-process counter making every temp filename unique.
    static SEQ: AtomicU64 = AtomicU64::new(0);

    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "app.d.ts".to_string());
    let temp_name = format!(".{file_name}.{}.{seq}.tmp", std::process::id());

    match path.parent() {
        Some(parent) => parent.join(temp_name),
        None => PathBuf::from(temp_name),
    }
}

/// Renders the full `app.d.ts` text for a snapshot of registered servers.
///
/// Emits an `App` interface with one nested namespace per server, followed by
/// the `declare global` block that types a plugin's dispatching proxy as `App`.
/// The servers are visited in [`BTreeMap`] key order, so the output is stable
/// and diff-friendly.
///
/// # Parameters
///
/// - `servers` — the registered servers, keyed by name, each with its tools.
fn render_declaration(servers: &BTreeMap<ServerName, Vec<ToolMetadata>>) -> String {
    let mut out = String::new();
    out.push_str("// Generated by the SwissArmyHammer plugin host — do not edit.\n");
    out.push_str("// This file is rewritten automatically as the server registry changes.\n\n");
    out.push_str("interface App {\n");

    for (name, tools) in servers {
        render_server_namespace(&mut out, name, tools);
    }

    out.push_str("}\n\n");
    out.push_str("declare global {\n");
    out.push_str("  interface Plugin {\n");
    out.push_str("    readonly [K in keyof App]: App[K];\n");
    out.push_str("  }\n");
    out.push_str("}\n");
    out
}

/// Emits the `<server>: { … }` member of the `App` interface for one server.
///
/// Each of the server's tools is emitted in turn: a flat tool as a single
/// method, an operation tool as a nested `tool: { noun: { verb(...) } }` block.
///
/// # Parameters
///
/// - `out` — the buffer the declaration text is appended to.
/// - `name` — the server's registered name, used as the namespace key.
/// - `tools` — the server's tools.
fn render_server_namespace(out: &mut String, name: &str, tools: &[ToolMetadata]) {
    out.push_str(&format!("  {}: {{\n", ts_key(name)));
    for tool in tools {
        match operations_meta(tool) {
            Some(operations) => render_operation_tool(out, tool.name(), operations),
            None => render_flat_tool(out, tool),
        }
    }
    // The event surface is server-scoped: `.on()` resolves across all of the
    // server's tools, so the notifications declared by every tool are emitted
    // as `on`/`subscribe`/`once` overloads on the server namespace itself.
    render_server_notifications(out, tools);
    out.push_str("  };\n");
}

/// Emits the typed event-subscription overloads for a server's declared
/// notifications.
///
/// Collects the `event → {method, description, parameters}` entries across
/// every tool on the server (the event API is server-scoped) and emits, for
/// each of `on` / `subscribe` (its alias) / `once`, one overload per event:
///
/// ```ts
/// on(event: "executed", cb: (params: { id: string; … }) => void): () => void;
/// ```
///
/// The `(params: …)` type is built from the event's declared `parameters` the
/// same way operation inputs are, and the `() => void` return is the
/// unsubscribe handle. A server whose tools declare no notifications emits
/// nothing, so its namespace is unchanged.
///
/// # Parameters
///
/// - `out` — the buffer the declaration text is appended to.
/// - `tools` — the server's tools, scanned for notification `_meta`.
fn render_server_notifications(out: &mut String, tools: &[ToolMetadata]) {
    let events: Vec<(&String, &Value)> = tools
        .iter()
        .filter_map(notifications_meta)
        .flat_map(|notes| notes.iter())
        .collect();
    if events.is_empty() {
        return;
    }

    // `subscribe` is an alias of `on`; `once` shares the same shape. All three
    // are real SDK surfaces, so all three are typed (the generated server type
    // is closed — an un-emitted method would be a type error at the call site).
    for method in ["on", "subscribe", "once"] {
        for (event, leaf) in &events {
            // Doc the event once, on the primary `on` overload.
            if method == "on" {
                if let Some(description) = leaf.get("description").and_then(Value::as_str) {
                    if !description.is_empty() {
                        render_doc_comment(out, 4, description);
                    }
                }
            }
            let parameters = leaf.get("parameters").and_then(Value::as_object);
            let params = ts_object_from_parameters(parameters);
            out.push_str(&format!(
                "    {}(event: {}, cb: (params: {}) => void): () => void;\n",
                method,
                ts_string_literal(event),
                params,
            ));
        }
    }
}

/// Returns a tool's `io.swissarmyhammer/notifications` `_meta` tree, if any.
///
/// The notification-side analogue of [`operations_meta`]: a tool whose `_meta`
/// carries [`NOTIFICATIONS_META_KEY`] declares events, and the value is the
/// `event → {method, description, parameters}` tree. Returns `None` for a tool
/// that declares no notifications.
fn notifications_meta(tool: &ToolMetadata) -> Option<&Map<String, Value>> {
    tool.as_tool()
        .meta
        .as_ref()
        .and_then(|meta| meta.get(NOTIFICATIONS_META_KEY))
        .and_then(Value::as_object)
}

/// Renders a string as a TypeScript string-literal type (double-quoted, escaped).
///
/// Used for the `event: "<name>"` literal in an `on` overload. JSON string
/// encoding matches TypeScript string-literal quoting, so a value with quotes
/// or backslashes is escaped correctly.
fn ts_string_literal(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| format!("\"{value}\""))
}

/// Returns a tool's `io.swissarmyhammer/operations` `_meta` tree, if it has one.
///
/// A tool whose `_meta` map carries the [`OPERATIONS_META_KEY`] is an operation
/// tool; the value under that key is the noun → verb → leaf tree. A tool with
/// no `_meta`, or a `_meta` without that key, is a flat tool and this returns
/// `None`.
///
/// # Parameters
///
/// - `tool` — the tool whose `_meta` is inspected.
fn operations_meta(tool: &ToolMetadata) -> Option<&Map<String, Value>> {
    tool.as_tool()
        .meta
        .as_ref()
        .and_then(|meta| meta.get(OPERATIONS_META_KEY))
        .and_then(Value::as_object)
}

/// Emits a single flat tool as one method on its server's namespace.
///
/// The method is named for the tool; its `input` parameter type is derived
/// from the tool's `inputSchema` by [`ts_type_from_schema`].
///
/// # Parameters
///
/// - `out` — the buffer the declaration text is appended to.
/// - `tool` — the flat tool to emit.
fn render_flat_tool(out: &mut String, tool: &ToolMetadata) {
    if let Some(description) = tool.description() {
        render_doc_comment(out, 4, description);
    }
    let input = ts_type_from_schema(tool.as_tool().input_schema.as_ref());
    out.push_str(&format!(
        "    {}(input: {}): Promise<unknown>;\n",
        ts_key(tool.name()),
        input,
    ));
}

/// Emits an operation tool as a nested `tool: { noun: { verb(...) } }` block.
///
/// The emitted shape mirrors the `_meta` operations tree exactly: the tool name
/// is the outer key, each noun is a nested namespace, and each verb is a method
/// whose `input` type is the object built from that verb leaf's `parameters`
/// map. No schema inference happens — this is a structural copy of the tree.
///
/// # Parameters
///
/// - `out` — the buffer the declaration text is appended to.
/// - `tool_name` — the operation tool's name, the outer key of the block.
/// - `operations` — the tool's noun → verb → leaf operations tree.
fn render_operation_tool(out: &mut String, tool_name: &str, operations: &Map<String, Value>) {
    out.push_str(&format!("    {}: {{\n", ts_key(tool_name)));
    for (noun, verbs) in operations {
        let Some(verbs) = verbs.as_object() else {
            continue;
        };
        out.push_str(&format!("      {}: {{\n", ts_key(noun)));
        for (verb, leaf) in verbs {
            render_operation_verb(out, verb, leaf);
        }
        out.push_str("      };\n");
    }
    out.push_str("    };\n");
}

/// Emits one `<verb>(input)` method from an operations-tree leaf.
///
/// The leaf is the `{op, description, parameters}` object a verb maps to. The
/// method's `input` type is the object type built from the leaf's `parameters`
/// map; the leaf's `description`, when present, becomes a doc comment.
///
/// # Parameters
///
/// - `out` — the buffer the declaration text is appended to.
/// - `verb` — the verb, used as the method name.
/// - `leaf` — the verb's `{op, description, parameters}` leaf object.
fn render_operation_verb(out: &mut String, verb: &str, leaf: &Value) {
    if let Some(description) = leaf.get("description").and_then(Value::as_str) {
        if !description.is_empty() {
            render_doc_comment(out, 8, description);
        }
    }
    let parameters = leaf.get("parameters").and_then(Value::as_object);
    let input = ts_object_from_parameters(parameters);
    out.push_str(&format!(
        "        {}(input: {}): Promise<unknown>;\n",
        ts_key(verb),
        input,
    ));
}

/// Builds a TypeScript input object type from an operation verb's `parameters`.
///
/// The `parameters` map is keyed by parameter name, each value a `{type,
/// required, …}` descriptor as `generate_operations_meta` produces. Each
/// parameter becomes one field: optional parameters (`required: false`) carry a
/// `?` suffix, and the field type is the TypeScript type for the descriptor's
/// `type`. An absent or empty `parameters` map yields the empty object `{}`.
///
/// # Parameters
///
/// - `parameters` — the verb leaf's `parameters` map, or `None` when absent.
fn ts_object_from_parameters(parameters: Option<&Map<String, Value>>) -> String {
    let Some(parameters) = parameters else {
        return "{}".to_string();
    };
    if parameters.is_empty() {
        return "{}".to_string();
    }

    let mut fields = Vec::with_capacity(parameters.len());
    for (param_name, descriptor) in parameters {
        let required = descriptor
            .get("required")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let type_name = descriptor
            .get("type")
            .and_then(Value::as_str)
            .map(|json_type| ts_type_from_json_type(json_type, descriptor))
            .unwrap_or_else(|| "unknown".to_string());
        let optional = if required { "" } else { "?" };
        fields.push(format!("{}{}: {}", ts_key(param_name), optional, type_name));
    }
    format!("{{ {} }}", fields.join("; "))
}

/// Builds a TypeScript input object type from a flat tool's `inputSchema`.
///
/// An MCP `inputSchema` is a JSON Schema object. The emitter copies its
/// `properties` into TypeScript fields, marking a field optional unless its
/// name appears in the schema's `required` array. A schema with no `properties`
/// — the common no-argument tool — yields the empty object `{}`.
///
/// This is a structural copy of the schema's surface, not a full JSON Schema
/// compiler: nested object and array element types resolve to their TypeScript
/// primitive, which is all editor autocomplete needs.
///
/// # Parameters
///
/// - `schema` — the tool's `inputSchema` JSON object.
fn ts_type_from_schema(schema: &Map<String, Value>) -> String {
    let properties = schema.get("properties").and_then(Value::as_object);
    let Some(properties) = properties else {
        return "{}".to_string();
    };
    if properties.is_empty() {
        return "{}".to_string();
    }

    let required: Vec<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|names| names.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();

    let mut fields = Vec::with_capacity(properties.len());
    for (field_name, field_schema) in properties {
        let type_name = field_schema
            .as_object()
            .and_then(|object| object.get("type").and_then(Value::as_str))
            .map(|json_type| ts_type_from_json_type(json_type, field_schema))
            .unwrap_or_else(|| "unknown".to_string());
        let optional = if required.contains(&field_name.as_str()) {
            ""
        } else {
            "?"
        };
        fields.push(format!("{}{}: {}", ts_key(field_name), optional, type_name));
    }
    format!("{{ {} }}", fields.join("; "))
}

/// Maps a JSON Schema `type` string to its TypeScript type.
///
/// For an `array` the element type is taken from the descriptor's `items.type`
/// when present — the operations `_meta` tree always carries `items: {type:
/// "string"}` for array parameters — and defaults to `unknown[]` otherwise. An
/// unrecognized type maps to `unknown`.
///
/// # Parameters
///
/// - `json_type` — the JSON Schema `type` value.
/// - `descriptor` — the full descriptor, consulted for an array's `items`.
fn ts_type_from_json_type(json_type: &str, descriptor: &Value) -> String {
    match json_type {
        "string" => "string".to_string(),
        "integer" | "number" => "number".to_string(),
        "boolean" => "boolean".to_string(),
        "object" => "Record<string, unknown>".to_string(),
        "array" => {
            let item_type = descriptor
                .get("items")
                .and_then(Value::as_object)
                .and_then(|items| items.get("type"))
                .and_then(Value::as_str)
                .map(|item| ts_type_from_json_type(item, &Value::Null))
                .unwrap_or_else(|| "unknown".to_string());
            format!("{item_type}[]")
        }
        _ => "unknown".to_string(),
    }
}

/// Renders `text` as a TypeScript doc comment, indented by `indent` spaces.
///
/// Multi-line descriptions are emitted as a `/** … */` block with each line
/// prefixed by ` * `; the comment is purely advisory and never affects the
/// emitted types.
///
/// # Parameters
///
/// - `out` — the buffer the comment is appended to.
/// - `indent` — the number of leading spaces for every line of the comment.
/// - `text` — the description text.
fn render_doc_comment(out: &mut String, indent: usize, text: &str) {
    let pad = " ".repeat(indent);
    out.push_str(&format!("{pad}/** {} */\n", text.replace('\n', " ")));
}

/// Renders an identifier as a TypeScript object key.
///
/// A name that is a valid TypeScript identifier — letters, digits, `_` or `$`,
/// not leading with a digit — is emitted bare; anything else is emitted as a
/// quoted string key so a server, tool, noun, verb, or parameter name with a
/// `/`, `-`, or `.` in it still produces valid TypeScript.
///
/// # Parameters
///
/// - `name` — the raw name to render as a key.
fn ts_key(name: &str) -> String {
    let is_identifier = !name.is_empty()
        && name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$');
    if is_identifier {
        name.to_string()
    } else {
        format!("\"{}\"", name.replace('"', "\\\""))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::{Meta, Tool};
    use serde_json::json;
    use std::time::Duration;
    use tempfile::tempdir;

    /// How long a test waits for a debounced regeneration to land.
    ///
    /// Comfortably longer than [`DEBOUNCE`] so the spawned debounce task has
    /// fired, but still short — every wait in these tests is bounded so a
    /// regression cannot hang the suite.
    const DEBOUNCE_WAIT: Duration = Duration::from_millis(400);

    /// Builds a flat tool: a [`ToolMetadata`] with an `inputSchema` and no
    /// `io.swissarmyhammer/operations` `_meta`.
    ///
    /// # Parameters
    ///
    /// - `name` — the tool's name.
    /// - `input_schema` — the JSON Schema object for the tool's input.
    fn flat_tool(name: &str, input_schema: Value) -> ToolMetadata {
        let schema = input_schema
            .as_object()
            .cloned()
            .expect("flat tool input schema must be a JSON object");
        ToolMetadata::new(Tool::new(name.to_string(), "a flat tool", schema))
    }

    /// Builds an operation tool: a [`ToolMetadata`] carrying an
    /// `io.swissarmyhammer/operations` `_meta` tree.
    ///
    /// The tree is attached under the exact `_meta` key the host uses, so this
    /// fixture exercises the real operation-tool path.
    ///
    /// # Parameters
    ///
    /// - `name` — the tool's name.
    /// - `operations` — the noun → verb → leaf operations tree.
    fn operation_tool(name: &str, operations: Value) -> ToolMetadata {
        let mut tool = Tool::new(name.to_string(), "an operation tool", Map::new());
        let mut meta = Meta::new();
        meta.insert(OPERATIONS_META_KEY.to_string(), operations);
        tool.meta = Some(meta);
        ToolMetadata::new(tool)
    }

    /// A real `io.swissarmyhammer/operations` tree shaped exactly as
    /// `generate_operations_meta` produces it: noun → verb → `{op, description,
    /// parameters}`, each parameter `{type, required, …}`.
    fn kanban_operations() -> Value {
        json!({
            "task": {
                "add": {
                    "op": "add task",
                    "description": "Create a new task",
                    "parameters": {
                        "title": { "type": "string", "required": true, "description": "Task title" },
                        "description": { "type": "string", "required": false }
                    }
                },
                "move": {
                    "op": "move task",
                    "description": "Move a task to a column",
                    "parameters": {
                        "id": { "type": "string", "required": true },
                        "column": { "type": "string", "required": true }
                    }
                }
            },
            "board": {
                "init": {
                    "op": "init board",
                    "description": "Initialize a board",
                    "parameters": {
                        "name": { "type": "string", "required": true }
                    }
                }
            }
        })
    }

    /// Builds a tool carrying an `io.swissarmyhammer/notifications` `_meta`
    /// tree, exercising the real notification-emission path.
    fn notification_tool(name: &str, notifications: Value) -> ToolMetadata {
        let mut tool = Tool::new(
            name.to_string(),
            "a notification-declaring tool",
            Map::new(),
        );
        let mut meta = Meta::new();
        meta.insert(NOTIFICATIONS_META_KEY.to_string(), notifications);
        tool.meta = Some(meta);
        ToolMetadata::new(tool)
    }

    /// A real `io.swissarmyhammer/notifications` tree shaped as
    /// `generate_notifications_meta` produces it: event → `{method, description,
    /// parameters}`.
    fn command_notifications() -> Value {
        json!({
            "executed": {
                "method": "notifications/commands/executed",
                "description": "A command executed.",
                "parameters": {
                    "id": { "type": "string", "required": true, "description": "The command id" },
                    "result": { "type": "string", "required": false }
                }
            }
        })
    }

    /// Reads the declaration file at `path`, failing the test if it is absent.
    fn read_types(path: &Path) -> String {
        std::fs::read_to_string(path)
            .expect("declaration file should exist after a debounced write")
    }

    #[tokio::test]
    async fn emits_typed_on_overloads_for_declared_notifications() {
        let dir = tempdir().expect("temp dir");
        let output = dir.path().join("app.d.ts");
        let emitter = TypesEmitter::with_output_path(&output, true);

        emitter.server_registered(
            "commands",
            vec![notification_tool("command", command_notifications())],
        );
        emitter.flush();

        let text = read_types(&output);

        // The declared event becomes a typed `on` overload: an event string
        // literal, a params type built from the declaration (required vs
        // optional honored), and the `() => void` off handle.
        assert!(
            text.contains(
                "on(event: \"executed\", cb: (params: { id: string; result?: string }) => void): () => void;"
            ),
            "typed on() overload missing from:\n{text}"
        );
        // `subscribe` (alias) and `once` share the same typed overload.
        assert!(
            text.contains("subscribe(event: \"executed\", cb: (params: { id: string; result?: string }) => void): () => void;"),
            "subscribe overload missing from:\n{text}"
        );
        assert!(
            text.contains("once(event: \"executed\", cb: (params: { id: string; result?: string }) => void): () => void;"),
            "once overload missing from:\n{text}"
        );
    }

    #[tokio::test]
    async fn server_without_notifications_emits_no_on_overload() {
        let dir = tempdir().expect("temp dir");
        let output = dir.path().join("app.d.ts");
        let emitter = TypesEmitter::with_output_path(&output, true);

        emitter.server_registered("board", vec![operation_tool("kanban", kanban_operations())]);
        emitter.flush();

        let text = read_types(&output);
        assert!(
            !text.contains("on(event:"),
            "a server with no declared notifications must not emit on() overloads:\n{text}"
        );
    }

    /// Waits up to `DEBOUNCE_WAIT` for `predicate` to hold, polling briefly.
    ///
    /// Every wait in these tests is bounded through this helper so a broken
    /// debounce surfaces as a test failure, never as a hung suite.
    async fn wait_until(mut predicate: impl FnMut() -> bool) -> bool {
        let deadline = tokio::time::Instant::now() + DEBOUNCE_WAIT;
        while tokio::time::Instant::now() < deadline {
            if predicate() {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        predicate()
    }

    #[tokio::test]
    async fn emits_flat_method_and_nested_operation_signatures() {
        let dir = tempdir().expect("temp dir");
        let output = dir.path().join("app.d.ts");
        let emitter = TypesEmitter::with_output_path(&output, true);

        // A flat tool: one server "weather", one tool "current" with a typed
        // input schema and no operations `_meta`.
        emitter.server_registered(
            "weather",
            vec![flat_tool(
                "current",
                json!({
                    "type": "object",
                    "properties": { "city": { "type": "string" } },
                    "required": ["city"]
                }),
            )],
        );

        // An operation tool: server "board", tool "kanban", carrying a real
        // `io.swissarmyhammer/operations` tree.
        emitter.server_registered("board", vec![operation_tool("kanban", kanban_operations())]);

        emitter.flush();

        let text = read_types(&output);

        // The flat tool emits one typed method on its server's namespace.
        assert!(
            text.contains("current(input: { city: string }): Promise<unknown>;"),
            "flat tool method signature missing from:\n{text}"
        );

        // The operation tool emits the nested tool → noun → verb structure,
        // mirroring the `_meta` tree exactly.
        assert!(text.contains("kanban: {"), "operation tool block missing");
        assert!(text.contains("task: {"), "noun namespace 'task' missing");
        assert!(text.contains("board: {"), "noun namespace 'board' missing");

        // Each verb is a method whose input object type is built from that
        // verb's `parameters` map — required vs optional honored.
        assert!(
            text.contains("add(input: { title: string; description?: string }): Promise<unknown>;"),
            "operation 'task.add' signature wrong in:\n{text}"
        );
        assert!(
            text.contains("move(input: { id: string; column: string }): Promise<unknown>;"),
            "operation 'task.move' signature wrong in:\n{text}"
        );
        assert!(
            text.contains("init(input: { name: string }): Promise<unknown>;"),
            "operation 'board.init' signature wrong in:\n{text}"
        );

        // The `App` interface and the `declare global` proxy block are present.
        assert!(text.contains("interface App {"), "App interface missing");
        assert!(
            text.contains("declare global {"),
            "declare global block missing"
        );
    }

    #[tokio::test]
    async fn debounces_a_burst_into_one_write() {
        let dir = tempdir().expect("temp dir");
        let output = dir.path().join("app.d.ts");
        let emitter = TypesEmitter::with_output_path(&output, true);

        // Register several servers in a tight burst — as a plugin's `load()`
        // would — all inside the debounce window.
        for index in 0..8 {
            emitter.server_registered(
                format!("server-{index}"),
                vec![flat_tool("noop", json!({ "type": "object" }))],
            );
        }

        // Wait out the debounce, then confirm the file landed exactly once.
        let landed = wait_until(|| emitter.write_count() >= 1).await;
        assert!(landed, "the debounced write never landed");

        // Give any stray extra debounce task room to (wrongly) fire, then
        // assert the burst still collapsed to a single write.
        tokio::time::sleep(DEBOUNCE_WAIT).await;
        assert_eq!(
            emitter.write_count(),
            1,
            "a burst inside the debounce window must produce exactly one write"
        );

        // The single write reflects every server in the burst.
        let text = read_types(&output);
        for index in 0..8 {
            assert!(
                text.contains(&format!("\"server-{index}\": {{")),
                "server-{index} missing from the debounced output"
            );
        }
    }

    #[tokio::test]
    async fn write_is_atomic_and_leaves_no_temp_file() {
        let dir = tempdir().expect("temp dir");
        let output = dir.path().join("app.d.ts");
        let emitter = TypesEmitter::with_output_path(&output, true);

        emitter.server_registered(
            "weather",
            vec![flat_tool(
                "current",
                json!({ "type": "object", "properties": { "city": { "type": "string" } } }),
            )],
        );
        emitter.flush();

        // The destination holds a complete declaration — the `App` interface
        // opens and the `declare global` block closes the file.
        let text = read_types(&output);
        assert!(text.starts_with("// Generated by the SwissArmyHammer plugin host"));
        assert!(text.trim_end().ends_with('}'), "file looks truncated");
        assert!(text.contains("interface App {"));

        // The write-then-rename path leaves no temp file beside the output.
        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .expect("temp dir readable")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".tmp"))
            .collect();
        assert!(
            leftovers.is_empty(),
            "atomic write left temp files behind: {leftovers:?}"
        );

        // Only the destination file exists in the directory.
        let entries: Vec<_> = std::fs::read_dir(dir.path())
            .expect("temp dir readable")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            entries,
            vec!["app.d.ts".to_string()],
            "only the declaration file should remain"
        );
    }

    #[tokio::test]
    async fn production_mode_writes_nothing() {
        let dir = tempdir().expect("temp dir");
        let output = dir.path().join("app.d.ts");
        // Dev-mode flag OFF — the production posture.
        let emitter = TypesEmitter::with_output_path(&output, false);

        emitter.server_registered(
            "weather",
            vec![flat_tool("current", json!({ "type": "object" }))],
        );
        emitter.flush();
        emitter.server_registered("board", vec![operation_tool("kanban", kanban_operations())]);

        // Give a debounce window plus a margin: a production emitter must still
        // write nothing.
        tokio::time::sleep(DEBOUNCE_WAIT).await;

        assert!(
            !output.exists(),
            "production mode (dev flag off) must not write the declaration file"
        );
        assert_eq!(
            emitter.write_count(),
            0,
            "production mode must not count any writes"
        );
    }

    #[tokio::test]
    async fn unregister_drops_a_server_from_the_output() {
        let dir = tempdir().expect("temp dir");
        let output = dir.path().join("app.d.ts");
        let emitter = TypesEmitter::with_output_path(&output, true);

        emitter.server_registered(
            "weather",
            vec![flat_tool("current", json!({ "type": "object" }))],
        );
        emitter.server_registered("board", vec![operation_tool("kanban", kanban_operations())]);
        emitter.flush();
        assert!(read_types(&output).contains("weather: {"));

        // Unregistering "weather" regenerates the file without it.
        emitter.server_unregistered("weather");
        emitter.flush();

        let text = read_types(&output);
        assert!(
            !text.contains("weather: {"),
            "an unregistered server must disappear from the output"
        );
        assert!(
            text.contains("board: {"),
            "the remaining server must still be present"
        );
    }

    #[tokio::test]
    async fn tools_changed_regenerates_for_that_server() {
        let dir = tempdir().expect("temp dir");
        let output = dir.path().join("app.d.ts");
        let emitter = TypesEmitter::with_output_path(&output, true);

        emitter.server_registered(
            "weather",
            vec![flat_tool("current", json!({ "type": "object" }))],
        );
        emitter.flush();
        assert!(read_types(&output).contains("current(input:"));

        // A `notifications/tools/list_changed`-equivalent: the server now
        // advertises a different tool.
        emitter.tools_changed(
            "weather",
            vec![flat_tool(
                "forecast",
                json!({
                    "type": "object",
                    "properties": { "days": { "type": "integer" } },
                    "required": ["days"]
                }),
            )],
        );
        emitter.flush();

        let text = read_types(&output);
        assert!(
            text.contains("forecast(input: { days: number }): Promise<unknown>;"),
            "the changed tool set was not reflected:\n{text}"
        );
        assert!(
            !text.contains("current(input:"),
            "the replaced tool must no longer appear"
        );
    }
}
