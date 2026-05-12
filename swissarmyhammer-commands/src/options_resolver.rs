//! Backend resolver registry for `ParamDef.options_from`.
//!
//! The `options_from` field on a [`crate::types::ParamDef`] is a
//! stringly-typed key that names a backend resolver. At
//! `commands_for_scope` emission time, the kanban backend walks every
//! enum-shaped param whose `options_from` is set and asks an
//! [`OptionsRegistry`] to resolve the key into a concrete list of
//! [`crate::types::ParamOption`] entries. The result is written onto the
//! emitted command so the frontend never has to invent picker options —
//! it consumes whatever the backend embedded.
//!
//! # Crate boundary
//!
//! This crate is consumer-agnostic — it does not know about kanban,
//! perspectives, views, or any specific entity types. The trait and
//! registry defined here therefore use an opaque [`std::any::Any`]
//! reference for consumer-specific source data: kanban-side resolvers
//! downcast it to `&swissarmyhammer_kanban::scope_commands::DynamicSources`
//! at resolve time, while other consumers can plug in their own concrete
//! source-data types without this crate growing a dependency on theirs.

use std::any::Any;
use std::collections::HashMap;

use crate::types::ParamOption;

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
}
