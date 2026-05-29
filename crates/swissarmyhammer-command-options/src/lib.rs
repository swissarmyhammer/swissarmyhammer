//! Backend option-resolver machinery for command param pickers.

use serde::{Deserialize, Serialize};

/// A single option value for an enum-shaped param.
///
/// Used as an inline alternative to a backend resolver: when the option
/// list is static and known at YAML write time, write it directly on the
/// `ParamDef` rather than wiring up an `options_from` resolver.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParamOption {
    /// Machine-readable value that flows into the command's args bag.
    pub value: String,
    /// Human-readable label shown in the picker UI.
    pub label: String,
}

use std::any::{Any, TypeId};
use std::collections::HashMap;

/// A typed multi-domain container for [`OptionsContext::data`].
///
/// The [`OptionsResolver`] trait carries `data: &dyn Any`, which can
/// only be downcast to one concrete type at a time. That's a problem
/// for the multi-domain `commands_for_scope` path: a single call wants
/// to feed perspective-domain resolvers, view-domain resolvers, and
/// any-other-domain resolvers from the same context.
///
/// [`OptionsSources`] is the agreed-upon `data` shape. Each domain
/// crate defines its own owned data struct (e.g.
/// `PerspectivesOptionsData`, `ViewsOptionsData`) and inserts an
/// instance via [`Self::insert`]. Each resolver in that domain
/// retrieves it via [`Self::get`]. The kanban-app consumer (and any
/// other consumer of the command registry) constructs an
/// `OptionsSources`, populates it from its aggregator, and threads
/// `&sources as &dyn Any` into [`OptionsContext::data`].
///
/// This keeps `OptionsResolver` unchanged (the trait surface is
/// stable as the prior task delivered it) while letting per-domain
/// resolvers live in their domain crates without any back-reference
/// to a consumer-side aggregator type.
#[derive(Default)]
pub struct OptionsSources {
    map: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl OptionsSources {
    /// Construct an empty container. Use [`Self::insert`] to add
    /// per-domain data.
    pub fn new() -> Self {
        Self::default()
    }

    /// Store `value` keyed by its concrete [`TypeId`]. Replaces any
    /// previously-inserted value of the same type (last-write-wins
    /// per type, mirroring [`OptionsRegistry::register`]'s
    /// last-registration-wins semantics for keys).
    pub fn insert<T>(&mut self, value: T)
    where
        T: Any + Send + Sync,
    {
        self.map.insert(TypeId::of::<T>(), Box::new(value));
    }

    /// Retrieve a previously-inserted value of type `T`, or `None`
    /// when none was inserted. Resolvers call this to fetch their
    /// domain-specific data slice.
    pub fn get<T>(&self) -> Option<&T>
    where
        T: Any,
    {
        self.map.get(&TypeId::of::<T>())?.downcast_ref::<T>()
    }
}

/// Per-resolution context handed to every [`OptionsResolver::resolve`] call.
///
/// Carries the scope chain (innermost moniker first) and an opaque
/// reference to consumer-specific source data. Resolvers downcast
/// [`Self::data`] to whatever concrete type their consumer crate
/// provides (e.g. `&DynamicSources` for the kanban backend).
///
/// Held by reference for the duration of one resolver call — the
/// registry constructs a single context per `commands_for_scope`
/// invocation and threads it through every resolution in that call.
pub struct OptionsContext<'a> {
    /// Scope chain from the active `commands_for_scope` invocation
    /// (innermost moniker first, e.g.
    /// `["perspective:01P", "view:01V", "board:my-board"]`).
    pub scope_chain: &'a [String],
    /// Opaque pointer to consumer-specific source data.
    ///
    /// Concrete type is decided by the consumer crate that owns the
    /// resolvers — kanban resolvers downcast this to
    /// `&DynamicSources`. The pointer is borrowed for the duration of
    /// the resolve call so callers retain ownership.
    pub data: &'a dyn Any,
}

/// Resolves an `options_from` key into a concrete list of options for
/// a specific scope.
///
/// Implementations read from the [`OptionsContext`] only — they do not
/// own any state of their own. This keeps resolvers trivially
/// clonable, sendable, and shareable across test fixtures.
pub trait OptionsResolver: Send + Sync {
    /// The `options_from` key this resolver answers to (e.g.
    /// `"perspective.fields"`, `"view.kinds"`).
    fn key(&self) -> &'static str;

    /// Compute the concrete option list for the given context.
    ///
    /// Returns an empty `Vec` (not `None`) when the resolver
    /// understands the request but the scope does not yield any
    /// options — e.g. `PerspectiveFieldsResolver` returning `vec![]`
    /// when no `perspective:{id}` is in scope. Empty-list is a valid
    /// answer; `None` is reserved for the "key has no resolver" case
    /// handled at the [`OptionsRegistry`] level.
    fn resolve(&self, ctx: &OptionsContext<'_>) -> Vec<ParamOption>;
}

/// Registry of [`OptionsResolver`] implementations indexed by
/// [`OptionsResolver::key`].
///
/// `OptionsRegistry::new()` returns an empty registry — consumer
/// crates (e.g. `swissarmyhammer-kanban`) provide their own
/// constructors that pre-register their built-in resolvers. This
/// keeps the consumer-agnostic `swissarmyhammer-commands` crate free
/// of kanban-specific knowledge while still providing the shared
/// trait + registry plumbing.
#[derive(Default)]
pub struct OptionsRegistry {
    resolvers: HashMap<&'static str, Box<dyn OptionsResolver>>,
}

impl OptionsRegistry {
    /// Construct an empty registry. Use [`Self::register`] to add
    /// resolvers, or have a consumer crate provide a constructor
    /// that pre-registers its built-ins.
    pub fn new() -> Self {
        Self {
            resolvers: HashMap::new(),
        }
    }

    /// Register a resolver. If a resolver with the same
    /// [`OptionsResolver::key`] is already registered, the new one
    /// replaces it — last-registration-wins. This mirrors the YAML
    /// merge semantics of [`crate::registry::CommandsRegistry`].
    pub fn register(&mut self, resolver: Box<dyn OptionsResolver>) {
        let key = resolver.key();
        self.resolvers.insert(key, resolver);
    }

    /// Resolve an `options_from` key against the given context.
    ///
    /// Returns `None` when no resolver is registered for `key` — the
    /// caller is responsible for the warn-and-fall-through behavior
    /// (leave `options: None` on the emitted param). Returns
    /// `Some(vec)` when a resolver answered, even if the vec is
    /// empty (empty is a valid "this scope has no options" answer).
    pub fn resolve(&self, key: &str, ctx: &OptionsContext<'_>) -> Option<Vec<ParamOption>> {
        self.resolvers.get(key).map(|r| r.resolve(ctx))
    }

    /// True iff a resolver is registered for `key`. Used by the
    /// enrichment pass to decide between resolution and the
    /// warn-once-then-leave-`None` fallback.
    pub fn has(&self, key: &str) -> bool {
        self.resolvers.contains_key(key)
    }
}

/// Resolve `"sort.directions"` to the canonical `[asc, desc]` pair.
///
/// Static — does not consult the context. Mirrors the
/// `swissarmyhammer-perspectives` `SortDirection`'s lowercase serde
/// representation so the picker `value` is what the perspective
/// loader expects to deserialize.
///
/// Lives in the consumer-agnostic commands crate (rather than in
/// any domain crate) because the value list is a wire-format
/// constant — no domain-specific logic involved.
pub struct SortDirectionsResolver;

impl OptionsResolver for SortDirectionsResolver {
    fn key(&self) -> &'static str {
        "sort.directions"
    }

    fn resolve(&self, _ctx: &OptionsContext<'_>) -> Vec<ParamOption> {
        vec![
            ParamOption {
                value: "asc".into(),
                label: "Ascending".into(),
            },
            ParamOption {
                value: "desc".into(),
                label: "Descending".into(),
            },
        ]
    }
}

/// Register every commands-defined resolver onto the given registry.
///
/// Mirror this from the consumer that builds the registry; the
/// kanban-app's `default_options_registry()` calls it alongside the
/// perspective and view registrations.
pub fn register_command_resolvers(registry: &mut OptionsRegistry) {
    registry.register(Box::new(SortDirectionsResolver));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `Vec<String>` doubles as the "scope-only" source data shape so
    /// the trait + registry can be exercised without a downstream
    /// consumer crate.
    struct EchoLengthResolver;
    impl OptionsResolver for EchoLengthResolver {
        fn key(&self) -> &'static str {
            "test.echo_length"
        }
        fn resolve(&self, ctx: &OptionsContext<'_>) -> Vec<ParamOption> {
            vec![ParamOption {
                value: ctx.scope_chain.len().to_string(),
                label: format!("chain has {} monikers", ctx.scope_chain.len()),
            }]
        }
    }

    /// A `new()` registry answers `None` for every key — no built-ins
    /// are auto-registered at this layer.
    #[test]
    fn new_registry_is_empty() {
        let registry = OptionsRegistry::new();
        let scope: Vec<String> = vec![];
        let data: () = ();
        let ctx = OptionsContext {
            scope_chain: &scope,
            data: &data,
        };
        assert!(registry.resolve("test.echo_length", &ctx).is_none());
        assert!(!registry.has("test.echo_length"));
    }

    /// `register` adds a resolver; `resolve` then routes to it and the
    /// resolver sees the per-call scope chain via [`OptionsContext`].
    #[test]
    fn register_then_resolve_routes_to_resolver() {
        let mut registry = OptionsRegistry::new();
        registry.register(Box::new(EchoLengthResolver));
        let scope = vec!["a:1".to_string(), "b:2".to_string()];
        let data: () = ();
        let ctx = OptionsContext {
            scope_chain: &scope,
            data: &data,
        };
        let opts = registry.resolve("test.echo_length", &ctx).unwrap();
        assert_eq!(opts.len(), 1);
        assert_eq!(opts[0].value, "2");
        assert!(registry.has("test.echo_length"));
    }

    /// Re-registering the same `key` replaces the previous resolver
    /// (last-registration-wins). Mirrors the CommandsRegistry merge
    /// semantics for YAML overrides.
    #[test]
    fn register_replaces_previous_resolver_with_same_key() {
        struct A;
        impl OptionsResolver for A {
            fn key(&self) -> &'static str {
                "shared"
            }
            fn resolve(&self, _: &OptionsContext<'_>) -> Vec<ParamOption> {
                vec![ParamOption {
                    value: "a".into(),
                    label: "from A".into(),
                }]
            }
        }
        struct B;
        impl OptionsResolver for B {
            fn key(&self) -> &'static str {
                "shared"
            }
            fn resolve(&self, _: &OptionsContext<'_>) -> Vec<ParamOption> {
                vec![ParamOption {
                    value: "b".into(),
                    label: "from B".into(),
                }]
            }
        }
        let mut registry = OptionsRegistry::new();
        registry.register(Box::new(A));
        registry.register(Box::new(B));
        let scope: Vec<String> = vec![];
        let data: () = ();
        let ctx = OptionsContext {
            scope_chain: &scope,
            data: &data,
        };
        let opts = registry.resolve("shared", &ctx).unwrap();
        assert_eq!(opts[0].value, "b");
    }

    /// The static `sort.directions` resolver returns exactly the
    /// `asc` and `desc` rows, in that order. Exact-match test —
    /// the perspective loader expects these specific lowercase
    /// values via `swissarmyhammer-perspectives`'s `SortDirection`
    /// `#[serde(rename_all = "lowercase")]`, so drift here would
    /// break round-trip.
    #[test]
    fn sort_directions_resolver_returns_asc_and_desc_only() {
        let scope: Vec<String> = Vec::new();
        let data: () = ();
        let ctx = OptionsContext {
            scope_chain: &scope,
            data: &data,
        };
        let opts = SortDirectionsResolver.resolve(&ctx);
        assert_eq!(opts.len(), 2);
        assert_eq!(opts[0].value, "asc");
        assert_eq!(opts[0].label, "Ascending");
        assert_eq!(opts[1].value, "desc");
        assert_eq!(opts[1].label, "Descending");
    }

    /// `register_command_resolvers` adds [`SortDirectionsResolver`]
    /// under the canonical key.
    #[test]
    fn register_command_resolvers_adds_sort_directions_resolver() {
        let mut registry = OptionsRegistry::new();
        register_command_resolvers(&mut registry);
        assert!(registry.has("sort.directions"));
    }

    /// `OptionsSources::insert` stores by concrete TypeId and
    /// `OptionsSources::get` retrieves the same instance back. Pins
    /// the typed-multimap contract every per-domain resolver relies
    /// on.
    #[test]
    fn options_sources_round_trip_by_type() {
        #[derive(Debug, PartialEq)]
        struct Marker(u32);
        let mut sources = OptionsSources::new();
        sources.insert(Marker(42));
        let got = sources.get::<Marker>().unwrap();
        assert_eq!(got.0, 42);
    }

    /// Inserting two values of the same type replaces the earlier
    /// one (last-write-wins per type).
    #[test]
    fn options_sources_insert_replaces_same_type() {
        #[derive(Debug, PartialEq)]
        struct Marker(u32);
        let mut sources = OptionsSources::new();
        sources.insert(Marker(1));
        sources.insert(Marker(2));
        assert_eq!(sources.get::<Marker>().unwrap().0, 2);
    }

    /// `OptionsSources::get` returns `None` for a type that was
    /// never inserted.
    #[test]
    fn options_sources_get_missing_returns_none() {
        struct Marker;
        let sources = OptionsSources::new();
        assert!(sources.get::<Marker>().is_none());
    }
}
