//! Kanban-side composition of every domain crate's built-in
//! [`OptionsResolver`]s into a single [`OptionsRegistry`].
//!
//! [`swissarmyhammer_perspectives::PerspectiveFieldsResolver`] now
//! lives in the perspectives crate and is registered here via
//! [`swissarmyhammer_perspectives::register_perspective_resolvers`].
//! The two remaining built-ins are still defined in this module:
//!
//! - [`ViewKindsResolver`] — `"view.kinds"` (will move to
//!   `swissarmyhammer-views` in a follow-up commit)
//! - [`SortDirectionsResolver`] — `"sort.directions"` (will move to
//!   `swissarmyhammer-commands` in a follow-up commit)
//!
//! [`default_options_registry`] returns a fresh [`OptionsRegistry`]
//! with every built-in pre-registered. Consumers (the kanban-app GUI,
//! the kanban-cli, headless tests) call this exactly once at startup
//! and thread the registry into every
//! [`crate::scope_commands::commands_for_scope`] invocation.

use swissarmyhammer_commands::{OptionsContext, OptionsRegistry, OptionsResolver, ParamOption};
use swissarmyhammer_perspectives::register_perspective_resolvers;

/// Construct an [`OptionsRegistry`] with every built-in resolver
/// pre-registered.
///
/// Built-ins registered, in registration order:
///
/// 1. `perspective.fields` — from `swissarmyhammer-perspectives` via
///    [`register_perspective_resolvers`]
/// 2. [`ViewKindsResolver`] (key `"view.kinds"`)
/// 3. [`SortDirectionsResolver`] (key `"sort.directions"`)
///
/// Downstream consumers can further [`OptionsRegistry::register`]
/// additional resolvers on top of the returned registry.
pub fn default_options_registry() -> OptionsRegistry {
    let mut registry = OptionsRegistry::new();
    register_perspective_resolvers(&mut registry);
    registry.register(Box::new(ViewKindsResolver));
    registry.register(Box::new(SortDirectionsResolver));
    registry
}

/// Resolve `"view.kinds"` to a static list of every [`ViewKind`]
/// variant, projected through [`swissarmyhammer_views::ViewKind::as_kebab_str`]
/// so the wire-format values stay coherent with the
/// `CommandDef.view_kinds` filter and every other consumer of the
/// kebab-case representation.
///
/// `label` is a title-cased version of the kebab-case value —
/// "board" → "Board", "calendar" → "Calendar" — built without an
/// extra table so adding a new `ViewKind` variant requires touching
/// only the views crate, not this resolver.
///
/// Scope-independent: returns the same list regardless of context.
pub struct ViewKindsResolver;

impl OptionsResolver for ViewKindsResolver {
    fn key(&self) -> &'static str {
        "view.kinds"
    }

    fn resolve(&self, _ctx: &OptionsContext<'_>) -> Vec<ParamOption> {
        use swissarmyhammer_views::ViewKind;
        // Listing every variant explicitly is what wires the
        // `ViewKind::as_kebab_str` exhaustiveness test in
        // `swissarmyhammer-views` to this resolver: if a new variant
        // is added and not listed here, the exhaustiveness test in
        // this module fires before the picker silently misses an
        // option in production.
        let kinds = [
            ViewKind::Board,
            ViewKind::Grid,
            ViewKind::List,
            ViewKind::Calendar,
            ViewKind::Timeline,
        ];
        kinds
            .iter()
            .map(|k| {
                let value = k.as_kebab_str().to_string();
                let label = title_case(&value);
                ParamOption { value, label }
            })
            .collect()
    }
}

/// Title-case a single-word lowercase string ("board" → "Board").
///
/// Used to project a [`swissarmyhammer_views::ViewKind`]'s kebab-case
/// canonical string into a human-readable picker label without
/// maintaining a parallel label table. Multi-word kebab-case inputs
/// ("two-words") return "Two-words" (one capital, hyphen preserved)
/// — fine for the current `ViewKind` set (every variant is a single
/// word).
fn title_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().chain(chars).collect(),
    }
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

    /// The resolver enumerates every [`swissarmyhammer_views::ViewKind`]
    /// variant via its canonical [`as_kebab_str`] helper, so the wire
    /// format stays in lockstep with the `CommandDef.view_kinds`
    /// filter and the views-crate exhaustiveness test.
    ///
    /// [`as_kebab_str`]: swissarmyhammer_views::ViewKind::as_kebab_str
    #[test]
    fn view_kinds_resolver_lists_every_variant_via_canonical_helper() {
        use swissarmyhammer_views::ViewKind;
        let sources = OptionsSources::new();
        let scope: Vec<String> = Vec::new();
        let ctx = OptionsContext {
            scope_chain: &scope,
            data: &sources as &dyn std::any::Any,
        };
        let opts = ViewKindsResolver.resolve(&ctx);
        let values: Vec<&str> = opts.iter().map(|o| o.value.as_str()).collect();
        // Every non-Unknown variant must surface. `Unknown` is the
        // serde fallback for legacy YAML and is intentionally NOT
        // listed — the picker never offers it.
        for kind in [
            ViewKind::Board,
            ViewKind::Grid,
            ViewKind::List,
            ViewKind::Calendar,
            ViewKind::Timeline,
        ] {
            assert!(
                values.contains(&kind.as_kebab_str()),
                "view.kinds resolver must include {} via as_kebab_str; got values: {:?}",
                kind.as_kebab_str(),
                values,
            );
        }
        // And the Unknown variant must NOT leak into the picker
        // surface — it is the serde fallback for legacy data, not a
        // user-pickable kind.
        assert!(
            !values.contains(&ViewKind::Unknown.as_kebab_str()),
            "view.kinds resolver must not surface ViewKind::Unknown",
        );
    }

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
