//! Registry of MCP servers registered with the plugin platform.
//!
//! Tracks the set of registered [`McpServer`]s by unique name. The platform
//! has a single global server namespace, with **idempotent registration for
//! structurally-equal sources**: the first `register` of a name claims it, a
//! second `register` of the same name with the SAME [`ServerSource`] shares
//! the live registration (refcount bumps; the duplicate is dropped), and a
//! second `register` of the same name with a DIFFERENT [`ServerSource`] is
//! rejected with [`Error::ServerNameTaken`]. The name is freed for re-use
//! only when the LAST caller unregisters it.
//!
//! Unregistering does not erase the name: the registry keeps a *tombstone* —
//! a record that the name was once live and has since been disposed. A call
//! that resolves a tombstoned name learns the server was disposed out from
//! under it ([`ServerStatus::Disposed`]) rather than that the name never
//! existed ([`ServerStatus::Unknown`]). Re-registering the name clears its
//! tombstone.

use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;

use crate::error::{Error, Result};
use crate::server::McpServer;

/// The unique name an [`McpServer`] is registered under.
///
/// An alias for `String`: server names live in a single flat namespace, so a
/// plain string is the natural key. The alias names the role the string plays
/// throughout the registry API.
pub type ServerName = String;

/// A structural description of where a registered MCP server lives.
///
/// This is the registry's view of the JSON `source` shape an SDK `register`
/// payload carries. Holding it as a typed value — rather than the raw
/// [`Value`] — lets the registry compare two sources by **structural
/// equality**, which is what makes idempotent registration work: two plugins
/// that both wrote `{ cli: ["weather-server"] }` produce equal `ServerSource`
/// values, so the second `register` recognizes the first's live registration
/// and shares it.
///
/// `env` and `headers` are stored as [`BTreeMap`]s so equality is order-
/// independent: two callers that wrote `{ "FOO": "1", "BAR": "2" }` and
/// `{ "BAR": "2", "FOO": "1" }` produce equal sources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerSource {
    /// A host-exposed Rust module activated by id from the available-modules
    /// table. The id is the key the host registered the module under.
    Rust { id: String },

    /// A child process the host spawns and speaks MCP over stdio with.
    Cli {
        /// The command argv. First element is the executable; rest are args.
        command: Vec<String>,
        /// Extra environment variables for the child. Empty means "inherit
        /// the host's environment without additions".
        env: BTreeMap<String, String>,
        /// The child's working directory; `None` means the host's CWD.
        cwd: Option<PathBuf>,
    },

    /// An HTTP MCP endpoint the host connects to.
    Url {
        /// The endpoint URL the host connects to.
        url: String,
        /// Extra HTTP headers sent on every request. Empty means no extras.
        headers: BTreeMap<String, String>,
    },
}

impl ServerSource {
    /// Parses a JSON `source` payload into a structural [`ServerSource`].
    ///
    /// Accepts the three shapes the SDK's `ServerSource` type produces:
    /// `{ rust: "<id>" }`, `{ cli: [...], env?, cwd? }`, or
    /// `{ url: "...", headers? }`. Returns `None` for any other shape —
    /// callers map that to [`Error::ServerUnavailable`], matching the
    /// host's `connect_source` failure mode for an unknown source shape.
    ///
    /// # Parameters
    ///
    /// - `value` — the JSON `source` field from a `register` envelope.
    pub fn from_json(value: &Value) -> Option<Self> {
        if let Some(id) = value.get("rust").and_then(Value::as_str) {
            return Some(Self::Rust { id: id.to_string() });
        }
        if let Some(cli) = value.get("cli").and_then(Value::as_array) {
            let command: Option<Vec<String>> = cli
                .iter()
                .map(|element| element.as_str().map(str::to_string))
                .collect();
            let command = command?;
            let env = value
                .get("env")
                .and_then(Value::as_object)
                .map(|obj| {
                    obj.iter()
                        .filter_map(|(key, value)| {
                            value.as_str().map(|value| (key.clone(), value.to_string()))
                        })
                        .collect::<BTreeMap<_, _>>()
                })
                .unwrap_or_default();
            let cwd = value.get("cwd").and_then(Value::as_str).map(PathBuf::from);
            return Some(Self::Cli { command, env, cwd });
        }
        if let Some(url) = value.get("url").and_then(Value::as_str) {
            let headers = value
                .get("headers")
                .and_then(Value::as_object)
                .map(|obj| {
                    obj.iter()
                        .filter_map(|(key, value)| {
                            value.as_str().map(|value| (key.clone(), value.to_string()))
                        })
                        .collect::<BTreeMap<_, _>>()
                })
                .unwrap_or_default();
            return Some(Self::Url {
                url: url.to_string(),
                headers,
            });
        }
        None
    }
}

/// The outcome of a successful [`ServerRegistry::register`] call.
///
/// Distinguishes a fresh registration — the first `register` of a name —
/// from an idempotent share — a second `register` whose `(name, source)`
/// matches the live registration. Callers use this to skip side effects
/// that should only run once per fresh registration, like emitting a
/// types-emitter `server_registered` event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterOutcome {
    /// A fresh registration with refcount=1; the supplied `server` is now
    /// live under the name.
    Registered,

    /// The name was already registered with a structurally-equal source;
    /// the existing live server is kept (the supplied `server` argument is
    /// dropped) and the refcount is incremented. A later [`unregister`]
    /// must run once per `AlreadyRegistered` outcome to balance the share.
    ///
    /// [`unregister`]: ServerRegistry::unregister
    AlreadyRegistered,
}

/// The outcome of a [`ServerRegistry::unregister`] call.
///
/// Distinguishes the three meaningful cases a caller has to handle: no
/// registration at all, a refcount decrement that leaves the server live,
/// and the final unregister that actually tears the server down. Callers
/// use the cases to drive their own side effects — types-emitter events,
/// debug logging for a misbehaving plugin — exactly once each.
///
/// `Debug` is written by hand because the `Removed` variant carries an
/// `Arc<dyn McpServer>` and the [`McpServer`] trait has no `Debug` bound —
/// the impl reports only which variant fired and, for `Removed`, that an
/// arc was carried, without trying to print the opaque server.
pub enum UnregisterOutcome {
    /// The name has no live registration — never registered, or already
    /// fully torn down.
    NotRegistered,

    /// The refcount was decremented but the server stays live: another
    /// caller still holds the registration.
    Decremented,

    /// The last caller has unregistered. The refcount hit zero, the server
    /// was removed from the live registry, and the name carries a
    /// tombstone for future [`resolve`](ServerRegistry::resolve) calls.
    Removed(Arc<dyn McpServer>),
}

impl fmt::Debug for UnregisterOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotRegistered => f.write_str("NotRegistered"),
            Self::Decremented => f.write_str("Decremented"),
            Self::Removed(_) => f.write_str("Removed(<server>)"),
        }
    }
}

/// The outcome of resolving a name against the [`ServerRegistry`].
///
/// Resolution distinguishes four cases so a caller can return the accurate
/// error: a [`Live`](Self::Live) server; a name whose plugin is currently
/// being hot-reloaded ([`Reloading`](Self::Reloading) — the spec edge case
/// that maps to [`Error::PluginReloaded`](crate::Error::PluginReloaded)); a
/// name whose server was registered and later [`Disposed`](Self::Disposed) —
/// the regular tombstone case; and a name the registry has never seen
/// ([`Unknown`](Self::Unknown)).
///
/// `Reloading` takes priority over `Disposed`: during the hot-reload window,
/// the name *is* tombstoned (the v1 unregister already ran) but a caller's
/// `retry` is the correct response, so the more specific status is reported.
pub enum ServerStatus {
    /// The name resolves to a live, callable server.
    Live(Arc<dyn McpServer>),

    /// The name's backing plugin is mid-hot-reload — the v1 server has been
    /// disposed and the v2 server has not finished registering. Callers see
    /// this as [`Error::PluginReloaded`](crate::Error::PluginReloaded) and
    /// the right response is to retry once v2 settles.
    Reloading,

    /// The name was registered and has since been unregistered. A consumer
    /// holding this name learns its server was disposed out from under it.
    Disposed,

    /// The registry has no record of the name — it was never registered.
    Unknown,
}

/// One live registration kept by the registry.
///
/// Holds the live server, the source it was registered against (for
/// structural comparison on a second `register` of the same name), and the
/// refcount that tracks how many callers currently hold this registration.
/// The server is torn down — by removing this struct and yielding its
/// `Arc<dyn McpServer>` — only when the refcount reaches zero.
struct RegisteredServer {
    /// The live MCP server backing the registration.
    server: Arc<dyn McpServer>,

    /// The source the live server was connected from; compared structurally
    /// to a second registration's source to detect duplicates.
    source: ServerSource,

    /// How many callers (plugins, or host-internal registrants) hold this
    /// registration. Starts at 1 on first `register`, increments on every
    /// idempotent share, decrements on every `unregister`. The registration
    /// is removed when it would hit zero.
    refcount: usize,
}

/// Tracks the MCP servers registered with the platform, keyed by name.
///
/// The registry owns the shared handles to every registered server. Callers
/// register a server under a name + source, look it up by name to dispatch
/// work, and unregister it when the backing plugin goes away. Idempotent
/// registration: a second registration of the same `(name, source)` shares
/// the live server (refcount bump) rather than failing.
///
/// Unregistering leaves a tombstone in [`disposed`](Self::disposed) so a later
/// resolution of the freed name reports [`ServerStatus::Disposed`] rather than
/// [`ServerStatus::Unknown`]. Re-registering the name clears its tombstone.
#[derive(Default)]
pub struct ServerRegistry {
    /// The registered servers, keyed by their unique [`ServerName`].
    servers: HashMap<ServerName, RegisteredServer>,

    /// Tombstones: names that were registered and have since been fully
    /// unregistered (refcount hit zero). A name is in exactly one of
    /// `servers` or `disposed` — registering moves it into `servers` and
    /// clears any tombstone; the last unregister moves it the other way.
    disposed: HashSet<ServerName>,

    /// Names currently in the **hot-reload window**: their v1 server has been
    /// unregistered and their v2 server has not finished re-registering. A
    /// resolve against a reloading name returns
    /// [`ServerStatus::Reloading`], which the host translates to
    /// [`Error::PluginReloaded`](crate::Error::PluginReloaded) — the spec's
    /// "in-flight calls reject with `PluginReloaded`" edge case.
    ///
    /// The reload path stages this set with
    /// [`mark_reloading`](Self::mark_reloading) before disposing v1's
    /// registrations, and clears it with
    /// [`clear_reloading`](Self::clear_reloading) once v2's load completes;
    /// a successful v2 [`register`](Self::register) also clears the
    /// reloading flag for the re-registered name (the name is live again).
    reloading: HashSet<ServerName>,
}

/// `Debug` is written by hand because the registered server values are
/// `Arc<dyn McpServer>`, and the [`McpServer`] trait deliberately carries no
/// `Debug` supertrait bound — adding one would burden every transport impl.
/// The trait objects are therefore not printable, so this impl reports the
/// registered server names instead, which is the registry's meaningful state.
impl fmt::Debug for ServerRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServerRegistry")
            .field("servers", &self.servers.keys())
            .field("disposed", &self.disposed)
            .field("reloading", &self.reloading)
            .finish()
    }
}

impl ServerRegistry {
    /// Creates an empty registry with no servers registered.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `server` under `name` against `source`.
    ///
    /// The platform's name namespace is global with idempotent-for-equal-
    /// source semantics:
    ///
    /// - When `name` is free, the registration is created with refcount=1;
    ///   any tombstone or in-flight reload marker on the name is cleared.
    /// - When `name` is already registered and the existing source is
    ///   structurally equal to `source`, the call SUCCEEDS: the refcount is
    ///   incremented, the existing live server is kept, and the supplied
    ///   `server` is dropped. The outcome is
    ///   [`RegisterOutcome::AlreadyRegistered`], so the caller can skip side
    ///   effects (types emitter, etc.) that should only run on a fresh
    ///   registration.
    /// - When `name` is already registered but the existing source differs
    ///   from `source`, the call FAILS with [`Error::ServerNameTaken`] —
    ///   the platform never silently shadows a live server with a different
    ///   implementation.
    ///
    /// # Parameters
    ///
    /// - `name` — the name to register the server under.
    /// - `source` — the structural source describing where the server lives.
    /// - `server` — a shared handle to the server connected from `source`.
    ///
    /// # Returns
    ///
    /// [`RegisterOutcome::Registered`] on a fresh registration,
    /// [`RegisterOutcome::AlreadyRegistered`] on an idempotent share.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ServerNameTaken`] — carrying `name` — when a server
    /// is already registered under that name from a STRUCTURALLY DIFFERENT
    /// source. The existing registration is left untouched.
    pub fn register(
        &mut self,
        name: ServerName,
        source: ServerSource,
        server: Arc<dyn McpServer>,
    ) -> Result<RegisterOutcome> {
        match self.servers.entry(name) {
            Entry::Vacant(slot) => {
                // A re-registration revives the name: clear any tombstone and
                // any in-flight reload marker so the name resolves as live
                // rather than disposed or reloading.
                self.disposed.remove(slot.key());
                self.reloading.remove(slot.key());
                slot.insert(RegisteredServer {
                    server,
                    source,
                    refcount: 1,
                });
                Ok(RegisterOutcome::Registered)
            }
            Entry::Occupied(mut slot) => {
                if slot.get().source == source {
                    // Idempotent share: the live server stays, the duplicate
                    // `server` arg is dropped, and the refcount bumps. The
                    // reloading marker is cleared too — the name is live and
                    // a re-registration during a reload window counts.
                    self.reloading.remove(slot.key());
                    slot.get_mut().refcount = slot
                        .get()
                        .refcount
                        .checked_add(1)
                        .expect("server registration refcount must not overflow usize");
                    Ok(RegisterOutcome::AlreadyRegistered)
                } else {
                    Err(Error::ServerNameTaken(slot.key().clone()))
                }
            }
        }
    }

    /// Removes one caller's hold on the server registered under `name`.
    ///
    /// Refcounted: the registration is torn down only when the LAST caller
    /// unregisters. A decrement that leaves the refcount above zero keeps
    /// the live server reachable — other callers still hold it.
    ///
    /// On the last unregister (refcount → 0) the registration is removed
    /// from the live registry, a tombstone is left so a later
    /// [`resolve`](Self::resolve) reports [`ServerStatus::Disposed`] rather
    /// than [`ServerStatus::Unknown`], and the freed `Arc<dyn McpServer>`
    /// is returned in [`UnregisterOutcome::Removed`].
    ///
    /// # Parameters
    ///
    /// - `name` — the name of the server one holder is releasing.
    ///
    /// # Returns
    ///
    /// [`UnregisterOutcome::NotRegistered`] when no registration is live under
    /// `name`, [`UnregisterOutcome::Decremented`] when the refcount drops but
    /// the server stays live, or [`UnregisterOutcome::Removed`] carrying the
    /// freed server handle when the last caller has unregistered.
    pub fn unregister(&mut self, name: &str) -> UnregisterOutcome {
        let Some(entry) = self.servers.get_mut(name) else {
            return UnregisterOutcome::NotRegistered;
        };

        // Decrement; if the refcount has not hit zero, leave the registration
        // in place — another caller still holds it.
        entry.refcount = entry
            .refcount
            .checked_sub(1)
            .expect("server registration refcount must not underflow below zero");
        if entry.refcount > 0 {
            return UnregisterOutcome::Decremented;
        }

        // Last caller has unregistered: tear the registration down and leave
        // a tombstone for a later `resolve`.
        let removed = self
            .servers
            .remove(name)
            .expect("the entry was present a moment ago — get_mut above proved it");
        self.disposed.insert(name.to_string());
        UnregisterOutcome::Removed(removed.server)
    }

    /// Looks up the server registered under `name`.
    ///
    /// # Parameters
    ///
    /// - `name` — the name of the server to look up.
    ///
    /// # Returns
    ///
    /// `Some` clone of the shared handle when a server is registered under
    /// `name`; `None` otherwise. This does not distinguish a disposed name
    /// from one that never existed — use [`resolve`](Self::resolve) when that
    /// distinction matters.
    pub fn get(&self, name: &str) -> Option<Arc<dyn McpServer>> {
        self.servers.get(name).map(|entry| entry.server.clone())
    }

    /// Resolves `name` to its registration status.
    ///
    /// Unlike [`get`](Self::get), this distinguishes a live server from a name
    /// whose server was disposed (tombstoned), a name whose backing plugin is
    /// mid-hot-reload, and a name the registry has never seen — so a router
    /// can return [`Error::PluginReloaded`] for the reloading case,
    /// [`Error::ServerUnavailable`] for the disposed case, and
    /// [`Error::UnknownServer`] only for the never-registered case.
    ///
    /// # Parameters
    ///
    /// - `name` — the name of the server to resolve.
    ///
    /// # Returns
    ///
    /// [`ServerStatus::Reloading`] when `name` is staged for hot reload (this
    /// status wins over `Live`/`Disposed` for the duration of the reload
    /// window), [`ServerStatus::Live`] carrying the server handle when `name`
    /// is live, [`ServerStatus::Disposed`] when `name` carries a tombstone,
    /// and [`ServerStatus::Unknown`] when the registry has no record of
    /// `name`.
    pub fn resolve(&self, name: &str) -> ServerStatus {
        // Reloading wins over Live and Disposed: the in-flight reload window
        // expressly tells callers to retry, so a server that the v1 unregister
        // has already removed from `servers` (and that the v2 load has not
        // re-registered yet) reports `Reloading` rather than `Disposed`.
        if self.reloading.contains(name) {
            return ServerStatus::Reloading;
        }
        if let Some(entry) = self.servers.get(name) {
            return ServerStatus::Live(entry.server.clone());
        }
        if self.disposed.contains(name) {
            return ServerStatus::Disposed;
        }
        ServerStatus::Unknown
    }

    /// Marks `name` as in the hot-reload window.
    ///
    /// The reload path calls this for every server name v1 holds before
    /// disposing v1's registrations. Until the matching
    /// [`clear_reloading`](Self::clear_reloading) — or until v2's
    /// [`register`](Self::register) of the same name — a
    /// [`resolve`](Self::resolve) of `name` reports
    /// [`ServerStatus::Reloading`], which the host translates to
    /// [`Error::PluginReloaded`](crate::Error::PluginReloaded).
    ///
    /// Idempotent: marking a name that is already reloading is a no-op.
    pub fn mark_reloading(&mut self, name: &str) {
        self.reloading.insert(name.to_string());
    }

    /// Clears the hot-reload marker on `name`.
    ///
    /// Called by the reload path after v2's load completes (success or
    /// failure) for every name v1 held that v2 did not re-register. A
    /// successful v2 [`register`](Self::register) already clears the marker
    /// for the re-registered name, so this call is a no-op there.
    ///
    /// Returns `true` if the marker was set and is now cleared, `false` if
    /// the name was not marked reloading.
    pub fn clear_reloading(&mut self, name: &str) -> bool {
        self.reloading.remove(name)
    }

    /// Returns whether `name` currently has any caller holding it.
    ///
    /// A helper for callers (chiefly the host) that need to know whether a
    /// `register` would create a fresh registration without actually trying
    /// — useful in the `connect_and_register` fast-path that skips the
    /// expensive `connect_source` work when an idempotent share is possible.
    pub fn source_for(&self, name: &str) -> Option<&ServerSource> {
        self.servers.get(name).map(|entry| &entry.source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::{CallerId, ToolMetadata};
    use async_trait::async_trait;
    use rmcp::model::Tool;
    use serde_json::{Map, Value};

    /// A trivial [`McpServer`] used to exercise the registry.
    ///
    /// It advertises a single fixed tool and its `invoke` echoes the input
    /// straight back, so registry tests can register and dispatch against a
    /// real trait object without a transport.
    struct FakeServer {
        tool_name: &'static str,
    }

    #[async_trait]
    impl McpServer for FakeServer {
        fn tools(&self) -> Vec<ToolMetadata> {
            let schema = Map::new();
            vec![ToolMetadata::new(Tool::new(
                self.tool_name,
                "a fixed fake tool",
                schema,
            ))]
        }

        async fn invoke(&self, _caller: CallerId, _tool: &str, input: Value) -> Result<Value> {
            Ok(input)
        }
    }

    fn rust_source(id: &str) -> ServerSource {
        ServerSource::Rust { id: id.to_string() }
    }

    #[test]
    fn register_two_distinct_names_succeeds() {
        let mut registry = ServerRegistry::new();

        let first: Arc<dyn McpServer> = Arc::new(FakeServer { tool_name: "alpha" });
        let second: Arc<dyn McpServer> = Arc::new(FakeServer { tool_name: "beta" });

        let outcome_alpha = registry
            .register("alpha".to_string(), rust_source("alpha-mod"), first)
            .expect("registering a fresh name should succeed");
        assert!(matches!(outcome_alpha, RegisterOutcome::Registered));
        let outcome_beta = registry
            .register("beta".to_string(), rust_source("beta-mod"), second)
            .expect("registering a fresh name should succeed");
        assert!(matches!(outcome_beta, RegisterOutcome::Registered));

        assert!(registry.get("alpha").is_some());
        assert!(registry.get("beta").is_some());
    }

    #[test]
    fn register_taken_name_with_different_source_errors_with_server_name_taken() {
        let mut registry = ServerRegistry::new();

        let first: Arc<dyn McpServer> = Arc::new(FakeServer { tool_name: "alpha" });
        let duplicate: Arc<dyn McpServer> = Arc::new(FakeServer { tool_name: "other" });

        registry
            .register("alpha".to_string(), rust_source("first-mod"), first)
            .expect("first registration of a fresh name should succeed");

        // Registering the same name with a DIFFERENT source still fails —
        // idempotent registration applies only to structurally-equal sources.
        let err = registry
            .register("alpha".to_string(), rust_source("other-mod"), duplicate)
            .expect_err("registering with a mismatched source must fail");

        match err {
            Error::ServerNameTaken(name) => assert_eq!(name, "alpha"),
            other => panic!("expected ServerNameTaken, got {other:?}"),
        }
    }

    #[test]
    fn register_same_name_same_source_is_idempotent_and_keeps_first_server() {
        let mut registry = ServerRegistry::new();
        let source = rust_source("alpha-mod");

        let first: Arc<dyn McpServer> = Arc::new(FakeServer { tool_name: "first" });
        let outcome = registry
            .register("alpha".to_string(), source.clone(), Arc::clone(&first))
            .expect("the first registration should succeed");
        assert!(matches!(outcome, RegisterOutcome::Registered));

        let second: Arc<dyn McpServer> = Arc::new(FakeServer {
            tool_name: "second",
        });
        let outcome = registry
            .register("alpha".to_string(), source, Arc::clone(&second))
            .expect("a same-source second registration must succeed");
        assert!(
            matches!(outcome, RegisterOutcome::AlreadyRegistered),
            "the second same-source registration must report AlreadyRegistered"
        );

        // The live server is still the FIRST one. The new `Arc<dyn McpServer>`
        // passed to the second `register` was dropped in favor of the live
        // registration.
        let live = registry.get("alpha").expect("alpha must be live");
        assert!(
            Arc::ptr_eq(&live, &first),
            "the same-source dedup must keep the originally-registered server"
        );
    }

    #[test]
    fn unregister_only_drops_the_server_when_the_last_caller_leaves() {
        let mut registry = ServerRegistry::new();
        let source = rust_source("alpha-mod");
        let server: Arc<dyn McpServer> = Arc::new(FakeServer { tool_name: "alpha" });

        registry
            .register("alpha".to_string(), source.clone(), Arc::clone(&server))
            .expect("the first registration should succeed");
        registry
            .register("alpha".to_string(), source, Arc::clone(&server))
            .expect("a same-source share should succeed");

        // First unregister: refcount goes 2 → 1, the server stays live.
        assert!(
            matches!(registry.unregister("alpha"), UnregisterOutcome::Decremented),
            "the first unregister of a shared name should only decrement"
        );
        assert!(
            registry.get("alpha").is_some(),
            "the shared server must stay live until all callers unregister"
        );

        // Last unregister: refcount hits zero, the server is removed and
        // tombstoned. The Arc returned must be the one originally registered.
        let outcome = registry.unregister("alpha");
        let removed = match outcome {
            UnregisterOutcome::Removed(removed) => removed,
            other => panic!("the last unregister must report Removed, got {other:?}"),
        };
        assert!(
            Arc::ptr_eq(&removed, &server),
            "the removed arc must point at the registered server"
        );
        assert!(
            matches!(registry.resolve("alpha"), ServerStatus::Disposed),
            "after the last unregister the name should resolve as Disposed"
        );
    }

    #[test]
    fn resolve_distinguishes_live_disposed_and_unknown() {
        let mut registry = ServerRegistry::new();

        // A name the registry has never seen resolves as Unknown.
        assert!(
            matches!(registry.resolve("never"), ServerStatus::Unknown),
            "a never-registered name must resolve as Unknown"
        );

        let server: Arc<dyn McpServer> = Arc::new(FakeServer { tool_name: "alpha" });
        registry
            .register("alpha".to_string(), rust_source("alpha-mod"), server)
            .expect("registration of a fresh name should succeed");

        // A live registration resolves to a callable server.
        assert!(
            matches!(registry.resolve("alpha"), ServerStatus::Live(_)),
            "a registered name must resolve as Live"
        );

        let outcome = registry.unregister("alpha");
        assert!(
            matches!(outcome, UnregisterOutcome::Removed(_)),
            "a single-holder unregister must report Removed, got {outcome:?}"
        );

        // After the last unregister the name carries a tombstone.
        assert!(
            matches!(registry.resolve("alpha"), ServerStatus::Disposed),
            "an unregistered name must resolve as Disposed, not Unknown"
        );
    }

    #[test]
    fn re_registering_a_disposed_name_clears_its_tombstone() {
        let mut registry = ServerRegistry::new();

        let first: Arc<dyn McpServer> = Arc::new(FakeServer { tool_name: "alpha" });
        registry
            .register("alpha".to_string(), rust_source("alpha-mod"), first)
            .expect("registration of a fresh name should succeed");
        let _ = registry.unregister("alpha");
        assert!(
            matches!(registry.resolve("alpha"), ServerStatus::Disposed),
            "the name should be tombstoned after the last unregister"
        );

        let second: Arc<dyn McpServer> = Arc::new(FakeServer { tool_name: "beta" });
        registry
            .register("alpha".to_string(), rust_source("beta-mod"), second)
            .expect("re-registering a tombstoned name should succeed");
        assert!(
            matches!(registry.resolve("alpha"), ServerStatus::Live(_)),
            "re-registration must clear the tombstone and resolve as Live"
        );
    }

    #[test]
    fn unregister_of_a_never_registered_name_leaves_no_tombstone() {
        let mut registry = ServerRegistry::new();

        let outcome = registry.unregister("ghost");
        assert!(
            matches!(outcome, UnregisterOutcome::NotRegistered),
            "unregister of an unknown name reports NotRegistered, got {outcome:?}"
        );
        assert!(
            matches!(registry.resolve("ghost"), ServerStatus::Unknown),
            "a name that was never live must not gain a tombstone"
        );
    }

    #[test]
    fn server_source_from_json_parses_each_shape() {
        let rust = ServerSource::from_json(&serde_json::json!({ "rust": "kanban" }))
            .expect("a `{ rust }` source should parse");
        assert!(matches!(rust, ServerSource::Rust { ref id } if id == "kanban"));

        let cli = ServerSource::from_json(&serde_json::json!({
            "cli": ["weather", "--mode", "live"],
            "env": { "API_KEY": "abc" },
            "cwd": "/tmp",
        }))
        .expect("a `{ cli }` source should parse");
        match cli {
            ServerSource::Cli {
                ref command,
                ref env,
                ref cwd,
            } => {
                assert_eq!(command, &vec!["weather", "--mode", "live"]);
                assert_eq!(env.get("API_KEY").map(String::as_str), Some("abc"));
                assert_eq!(cwd.as_deref(), Some(std::path::Path::new("/tmp")));
            }
            other => panic!("expected Cli, got {other:?}"),
        }

        let url = ServerSource::from_json(&serde_json::json!({
            "url": "https://example.test/weather",
            "headers": { "X-Token": "t" },
        }))
        .expect("a `{ url }` source should parse");
        match url {
            ServerSource::Url {
                ref url,
                ref headers,
            } => {
                assert_eq!(url, "https://example.test/weather");
                assert_eq!(headers.get("X-Token").map(String::as_str), Some("t"));
            }
            other => panic!("expected Url, got {other:?}"),
        }

        assert!(ServerSource::from_json(&serde_json::json!({})).is_none());
    }

    #[test]
    fn server_source_cli_equality_is_order_independent_for_env() {
        let a = ServerSource::from_json(&serde_json::json!({
            "cli": ["x"],
            "env": { "A": "1", "B": "2" },
        }))
        .unwrap();
        let b = ServerSource::from_json(&serde_json::json!({
            "cli": ["x"],
            "env": { "B": "2", "A": "1" },
        }))
        .unwrap();
        assert_eq!(a, b, "env-map equality must be order-independent");
    }
}
