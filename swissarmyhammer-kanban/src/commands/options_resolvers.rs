//! Built-in [`OptionsResolver`]s for kanban-specific picker options.
//!
//! This module hosts the three resolvers that ship with
//! `swissarmyhammer-kanban`:
//!
//! - [`PerspectiveFieldsResolver`] — `"perspective.fields"`
//! - [`ViewKindsResolver`] — `"view.kinds"`
//! - [`SortDirectionsResolver`] — `"sort.directions"`
//!
//! [`default_options_registry`] returns a fresh [`OptionsRegistry`]
//! with all three pre-registered. Consumers (the kanban-app GUI, the
//! kanban-cli, headless tests) call this exactly once at startup and
//! thread the registry into every [`crate::scope_commands::commands_for_scope`]
//! invocation.
//!
//! Adding a new built-in resolver: implement [`OptionsResolver`] in
//! this module and register it inside [`default_options_registry`].

use swissarmyhammer_commands::{OptionsContext, OptionsRegistry, OptionsResolver, ParamOption};

use crate::scope_commands::DynamicSources;

/// Construct an [`OptionsRegistry`] with all kanban-specific built-in
/// resolvers pre-registered.
///
/// Built-ins registered, in registration order:
///
/// 1. [`PerspectiveFieldsResolver`] (key `"perspective.fields"`)
/// 2. [`ViewKindsResolver`] (key `"view.kinds"`)
/// 3. [`SortDirectionsResolver`] (key `"sort.directions"`)
///
/// Downstream consumers can further [`OptionsRegistry::register`]
/// additional resolvers on top of the returned registry.
pub fn default_options_registry() -> OptionsRegistry {
    let mut registry = OptionsRegistry::new();
    registry.register(Box::new(PerspectiveFieldsResolver));
    registry.register(Box::new(ViewKindsResolver));
    registry.register(Box::new(SortDirectionsResolver));
    registry
}

/// Downcast `OptionsContext::data` to `&DynamicSources`.
///
/// Every kanban resolver expects the consumer to thread a
/// [`DynamicSources`] through the context. A wrong-type downcast is
/// programmer error (a non-kanban consumer reusing kanban resolvers);
/// we return `None` and let the resolver fall back to an empty list
/// rather than panic in production.
fn dynamic_from<'a>(ctx: &OptionsContext<'a>) -> Option<&'a DynamicSources> {
    ctx.data.downcast_ref::<DynamicSources>()
}

/// Resolve `"perspective.fields"` against the innermost
/// `perspective:{id}` moniker in the scope chain.
///
/// Walks the scope chain innermost-first (the documented order from
/// [`crate::scope_commands::commands_for_scope`], e.g.
/// `["perspective:01P", "view:01V", "board:my-board"]`) and returns
/// the first `perspective:{id}` it encounters — that is the
/// perspective the user has open. We look that perspective up in
/// [`DynamicSources::perspectives`] and project its denormalised
/// [`swissarmyhammer_perspectives::PerspectiveFieldInfo`] list onto one
/// [`ParamOption`] per field.
///
/// Returns an empty `Vec` when:
///
/// - the scope chain has no `perspective:{id}` moniker,
/// - the moniker's id is unknown to `DynamicSources.perspectives`, or
/// - the matching perspective has an empty field list.
///
/// Never panics — the resolver is read-only and tolerates every
/// missing-input branch.
pub struct PerspectiveFieldsResolver;

impl OptionsResolver for PerspectiveFieldsResolver {
    fn key(&self) -> &'static str {
        "perspective.fields"
    }

    fn resolve(&self, ctx: &OptionsContext<'_>) -> Vec<ParamOption> {
        let Some(dyn_src) = dynamic_from(ctx) else {
            return Vec::new();
        };
        let Some(perspective_id) = innermost_perspective_id(ctx.scope_chain) else {
            return Vec::new();
        };
        let Some(perspective) = dyn_src.perspectives.iter().find(|p| p.id == perspective_id) else {
            return Vec::new();
        };
        perspective
            .fields
            .iter()
            .map(|f| ParamOption {
                value: f.id.clone(),
                label: f.display_name.clone(),
            })
            .collect()
    }
}

/// Find the innermost `perspective:{id}` moniker in the scope chain.
///
/// `scope_chain` is documented innermost-first, so the first
/// `perspective:{id}` we encounter IS the innermost. Returns the
/// trailing id portion (`{id}`) or `None` if no perspective moniker
/// is present.
fn innermost_perspective_id(scope_chain: &[String]) -> Option<&str> {
    scope_chain
        .iter()
        .find_map(|m| m.strip_prefix("perspective:"))
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
    use swissarmyhammer_perspectives::{PerspectiveFieldInfo, PerspectiveInfo};

    /// Build a [`DynamicSources`] carrying one perspective with three
    /// fields. The three field ids are stable test ULIDs so the
    /// assertion can match on exact value strings.
    fn fixture_dyn_sources() -> DynamicSources {
        DynamicSources {
            perspectives: vec![PerspectiveInfo {
                id: "01P".into(),
                name: "Active Sprint".into(),
                view: "grid".into(),
                fields: vec![
                    PerspectiveFieldInfo {
                        id: "01F1".into(),
                        display_name: "Title".into(),
                    },
                    PerspectiveFieldInfo {
                        id: "01F2".into(),
                        display_name: "Status".into(),
                    },
                    PerspectiveFieldInfo {
                        id: "01F3".into(),
                        display_name: "Priority".into(),
                    },
                ],
            }],
            ..Default::default()
        }
    }

    /// With `perspective:01P` in scope and a fixture perspective that
    /// carries three fields, the resolver returns three
    /// `ParamOption`s in field order with `value = field_id` and
    /// `label = field_display_name`. This pins the wire format the
    /// frontend `<CommandPopover>` will consume.
    #[test]
    fn perspective_fields_resolver_returns_fields_for_in_scope_perspective() {
        let dyn_src = fixture_dyn_sources();
        let scope = vec!["perspective:01P".to_string()];
        let ctx = OptionsContext {
            scope_chain: &scope,
            data: &dyn_src as &dyn std::any::Any,
        };
        let opts = PerspectiveFieldsResolver.resolve(&ctx);
        assert_eq!(opts.len(), 3);
        assert_eq!(opts[0].value, "01F1");
        assert_eq!(opts[0].label, "Title");
        assert_eq!(opts[1].value, "01F2");
        assert_eq!(opts[1].label, "Status");
        assert_eq!(opts[2].value, "01F3");
        assert_eq!(opts[2].label, "Priority");
    }

    /// With an empty scope chain, the resolver returns an empty list
    /// rather than panicking. This is what lets a perspective field
    /// picker be safely rendered in a context where no perspective is
    /// open yet (e.g. a board-level command palette).
    #[test]
    fn perspective_fields_resolver_returns_empty_when_no_perspective_in_scope() {
        let dyn_src = fixture_dyn_sources();
        let scope: Vec<String> = Vec::new();
        let ctx = OptionsContext {
            scope_chain: &scope,
            data: &dyn_src as &dyn std::any::Any,
        };
        let opts = PerspectiveFieldsResolver.resolve(&ctx);
        assert!(opts.is_empty());
    }

    /// An in-scope `perspective:{id}` that does NOT match any
    /// perspective in [`DynamicSources`] also resolves to an empty
    /// list (not a panic, not a fallback to another perspective).
    /// This pins the "unknown perspective" behavior the picker
    /// relies on when the scope chain is stale.
    #[test]
    fn perspective_fields_resolver_empty_when_perspective_id_unknown() {
        let dyn_src = fixture_dyn_sources();
        let scope = vec!["perspective:nope".to_string()];
        let ctx = OptionsContext {
            scope_chain: &scope,
            data: &dyn_src as &dyn std::any::Any,
        };
        let opts = PerspectiveFieldsResolver.resolve(&ctx);
        assert!(opts.is_empty());
    }

    /// The resolver enumerates every [`swissarmyhammer_views::ViewKind`]
    /// variant via its canonical [`as_kebab_str`] helper, so the wire
    /// format stays in lockstep with the `CommandDef.view_kinds`
    /// filter and the views-crate exhaustiveness test.
    ///
    /// [`as_kebab_str`]: swissarmyhammer_views::ViewKind::as_kebab_str
    #[test]
    fn view_kinds_resolver_lists_every_variant_via_canonical_helper() {
        use swissarmyhammer_views::ViewKind;
        let dyn_src = DynamicSources::default();
        let scope: Vec<String> = Vec::new();
        let ctx = OptionsContext {
            scope_chain: &scope,
            data: &dyn_src as &dyn std::any::Any,
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
        let dyn_src = DynamicSources::default();
        let scope: Vec<String> = Vec::new();
        let ctx = OptionsContext {
            scope_chain: &scope,
            data: &dyn_src as &dyn std::any::Any,
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

    /// If a non-kanban consumer reuses these resolvers with a
    /// non-`DynamicSources` context, the kanban-specific resolvers
    /// return an empty list rather than panicking. This pins the
    /// "graceful misuse" contract the cross-crate downcast relies on.
    #[test]
    fn perspective_fields_resolver_returns_empty_for_wrong_context_type() {
        let data: () = ();
        let scope = vec!["perspective:01P".to_string()];
        let ctx = OptionsContext {
            scope_chain: &scope,
            data: &data as &dyn std::any::Any,
        };
        let opts = PerspectiveFieldsResolver.resolve(&ctx);
        assert!(opts.is_empty());
    }
}
