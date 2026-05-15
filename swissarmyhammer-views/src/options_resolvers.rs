//! Built-in [`OptionsResolver`]s for view-domain picker options.
//!
//! Hosts [`ViewKindsResolver`], the scope-independent resolver that
//! enumerates every non-`Unknown` [`crate::types::ViewKind`] variant
//! through its canonical [`crate::types::ViewKind::as_kebab_str`]
//! helper. Register it onto an [`OptionsRegistry`] via
//! [`register_view_resolvers`]; the consumer composing the default
//! registry calls this alongside the other per-domain registration
//! helpers.
//!
//! [`OptionsRegistry`]: swissarmyhammer_commands::OptionsRegistry
//! [`OptionsResolver`]: swissarmyhammer_commands::OptionsResolver

use swissarmyhammer_commands::{OptionsContext, OptionsRegistry, OptionsResolver, ParamOption};

use crate::types::ViewKind;

/// Resolve `"view.kinds"` to a static list of every [`ViewKind`]
/// variant, projected through [`crate::types::ViewKind::as_kebab_str`]
/// so the wire-format values stay coherent with the
/// `CommandDef.view_kinds` filter and every other consumer of the
/// kebab-case representation.
///
/// `label` is a title-cased version of the kebab-case value —
/// `"board"` → `"Board"`, `"calendar"` → `"Calendar"` — built without
/// an extra table so adding a new [`ViewKind`] variant requires
/// touching only this crate.
///
/// Scope-independent: returns the same list regardless of context.
pub struct ViewKindsResolver;

impl OptionsResolver for ViewKindsResolver {
    fn key(&self) -> &'static str {
        "view.kinds"
    }

    fn resolve(&self, _ctx: &OptionsContext<'_>) -> Vec<ParamOption> {
        // Listing every variant explicitly is what wires the
        // `ViewKind::as_kebab_str` exhaustiveness test to this
        // resolver: if a new variant is added and not listed here,
        // the exhaustiveness test in this module fires before the
        // picker silently misses an option in production.
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

/// Register every view-domain resolver onto the given registry.
///
/// Mirror this from the consumer that builds the registry; the
/// kanban-app's `default_options_registry()` calls it alongside the
/// perspective and sort-direction registrations.
pub fn register_view_resolvers(registry: &mut OptionsRegistry) {
    registry.register(Box::new(ViewKindsResolver));
}

/// Title-case a single-word lowercase string (`"board"` → `"Board"`).
///
/// Used to project a [`ViewKind`]'s kebab-case canonical string into
/// a human-readable picker label without maintaining a parallel label
/// table. Multi-word kebab-case inputs (`"two-words"`) return
/// `"Two-words"` (one capital, hyphen preserved) — fine for the
/// current [`ViewKind`] set (every variant is a single word).
fn title_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().chain(chars).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_commands::OptionsSources;

    /// The resolver enumerates every [`ViewKind`] variant via its
    /// canonical [`ViewKind::as_kebab_str`] helper, so the wire
    /// format stays in lockstep with the `CommandDef.view_kinds`
    /// filter and the views-crate exhaustiveness test.
    #[test]
    fn view_kinds_resolver_lists_every_variant_via_canonical_helper() {
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

    /// `title_case` capitalises the first character and leaves the
    /// rest untouched. Pins the helper's contract so a future change
    /// to multi-word view kinds surfaces here.
    #[test]
    fn title_case_capitalises_first_character_only() {
        assert_eq!(title_case("board"), "Board");
        assert_eq!(title_case("calendar"), "Calendar");
        assert_eq!(title_case(""), "");
        assert_eq!(title_case("two-words"), "Two-words");
    }

    /// `register_view_resolvers` adds [`ViewKindsResolver`] under the
    /// canonical key. Pins the registration helper's contract so a
    /// future addition (or removal) of a resolver surfaces here.
    #[test]
    fn register_view_resolvers_adds_view_kinds_resolver() {
        let mut registry = OptionsRegistry::new();
        register_view_resolvers(&mut registry);
        assert!(registry.has("view.kinds"));
    }
}
