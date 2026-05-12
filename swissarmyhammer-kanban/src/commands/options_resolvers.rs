//! Kanban-side composition of every domain crate's built-in
//! [`OptionsResolver`]s into a single [`OptionsRegistry`].
//!
//! Both per-domain resolvers now live in their domain crates and are
//! registered here via the per-domain helpers:
//!
//! - `perspective.fields` — via
//!   [`swissarmyhammer_perspectives::register_perspective_resolvers`]
//! - `view.kinds` — via
//!   [`swissarmyhammer_views::register_view_resolvers`]
//!
//! [`SortDirectionsResolver`] is the only resolver still defined in
//! this module (it will move to `swissarmyhammer-commands` in a
//! follow-up commit since it has no domain dep).
//!
//! [`default_options_registry`] returns a fresh [`OptionsRegistry`]
//! with every built-in pre-registered. Consumers (the kanban-app GUI,
//! the kanban-cli, headless tests) call this exactly once at startup
//! and thread the registry into every
//! [`crate::scope_commands::commands_for_scope`] invocation.

use swissarmyhammer_commands::{OptionsContext, OptionsRegistry, OptionsResolver, ParamOption};
use swissarmyhammer_perspectives::register_perspective_resolvers;
use swissarmyhammer_views::register_view_resolvers;

/// Construct an [`OptionsRegistry`] with every built-in resolver
/// pre-registered.
///
/// Built-ins registered, in registration order:
///
/// 1. `perspective.fields` — from `swissarmyhammer-perspectives` via
///    [`register_perspective_resolvers`]
/// 2. `view.kinds` — from `swissarmyhammer-views` via
///    [`register_view_resolvers`]
/// 3. [`SortDirectionsResolver`] (key `"sort.directions"`)
///
/// Downstream consumers can further [`OptionsRegistry::register`]
/// additional resolvers on top of the returned registry.
pub fn default_options_registry() -> OptionsRegistry {
    let mut registry = OptionsRegistry::new();
    register_perspective_resolvers(&mut registry);
    register_view_resolvers(&mut registry);
    registry.register(Box::new(SortDirectionsResolver));
    registry
}

/// Resolve `"sort.directions"` to the canonical `[asc, desc]` pair.
///
/// Static — does not consult the context. Mirrors
/// [`swissarmyhammer_perspectives::SortDirection`]'s `lowercase`
/// serde representation so the picker `value` is what the perspective
/// loader expects to deserialize.
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

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_commands::OptionsSources;

    /// The static `sort.directions` resolver returns exactly the
    /// `asc` and `desc` rows, in that order. This is an exact-match
    /// test — the perspective loader expects these specific lowercase
    /// values via [`swissarmyhammer_perspectives::SortDirection`]'s
    /// `#[serde(rename_all = "lowercase")]`, so drift here would
    /// break round-trip.
    #[test]
    fn sort_directions_resolver_returns_asc_and_desc_only() {
        let sources = OptionsSources::new();
        let scope: Vec<String> = Vec::new();
        let ctx = OptionsContext {
            scope_chain: &scope,
            data: &sources as &dyn std::any::Any,
        };
        let opts = SortDirectionsResolver.resolve(&ctx);
        assert_eq!(opts.len(), 2);
        assert_eq!(opts[0].value, "asc");
        assert_eq!(opts[0].label, "Ascending");
        assert_eq!(opts[1].value, "desc");
        assert_eq!(opts[1].label, "Descending");
    }

    /// [`default_options_registry`] must register every built-in
    /// resolver. This guard fires if a new built-in is added to the
    /// module without being wired into the constructor.
    #[test]
    fn default_options_registry_registers_every_builtin_key() {
        let registry = default_options_registry();
        for key in ["perspective.fields", "view.kinds", "sort.directions"] {
            assert!(
                registry.has(key),
                "default_options_registry must register `{key}`",
            );
        }
    }
}
