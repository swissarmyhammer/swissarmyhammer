//! Built-in [`OptionsResolver`]s for perspective-domain picker options.
//!
//! The single resolver in this module ([`PerspectiveFieldsResolver`])
//! reads from a [`PerspectivesOptionsData`] retrieved out of the shared
//! [`OptionsSources`] container the consumer threads into
//! [`OptionsContext::data`]. The wrapper struct
//! [`PerspectivesOptionsData`] is defined here so the resolver can
//! live in this crate without any back-reference to the consumer-side
//! command aggregator (which lives in `kanban-app`).
//!
//! Register the resolver onto an [`OptionsRegistry`] via
//! [`register_perspective_resolvers`]; mirror that helper from the
//! consumer crate that owns the registry.
//!
//! [`OptionsRegistry`]: swissarmyhammer_command_options::OptionsRegistry
//! [`OptionsContext::data`]: swissarmyhammer_command_options::OptionsContext::data
//! [`OptionsResolver`]: swissarmyhammer_command_options::OptionsResolver
//! [`OptionsSources`]: swissarmyhammer_command_options::OptionsSources

use swissarmyhammer_command_options::{
    OptionsContext, OptionsRegistry, OptionsResolver, OptionsSources, ParamOption,
};

use crate::perspective_info::PerspectiveInfo;

/// Perspective-domain data carried via the shared [`OptionsSources`].
///
/// The consumer (the kanban-app aggregator builder) populates this
/// once per `commands_for_scope` call and inserts it into the
/// [`OptionsSources`]. [`PerspectiveFieldsResolver::resolve`]
/// retrieves it via `OptionsSources::get::<PerspectivesOptionsData>()`.
///
/// Owned `Vec` rather than a borrowed slice because [`std::any::Any`]
/// requires `'static`. Callers clone the descriptor list at the
/// aggregator-build boundary; the count is small (one entry per
/// perspective) so the cost is negligible.
#[derive(Debug, Clone, Default)]
pub struct PerspectivesOptionsData {
    /// The lightweight perspective descriptors used to answer the
    /// `perspective.fields` resolver. Construction order is
    /// preserved through to the resolver's output.
    pub perspectives: Vec<PerspectiveInfo>,
}

/// Resolve `"perspective.fields"` against the innermost
/// `perspective:{id}` moniker in the scope chain.
///
/// Walks the scope chain innermost-first (the documented order from
/// `commands_for_scope`, e.g. `["perspective:01P", "view:01V",
/// "board:my-board"]`) and returns the first `perspective:{id}` it
/// encounters — that is the perspective the user has open. We look
/// that perspective up in [`PerspectivesOptionsData::perspectives`]
/// and project its denormalised [`crate::PerspectiveFieldInfo`] list
/// onto one [`ParamOption`] per field.
///
/// Returns an empty `Vec` when:
///
/// - the scope chain has no `perspective:{id}` moniker,
/// - the moniker's id is unknown to the data slice, or
/// - the matching perspective has an empty field list, or
/// - the context's [`OptionsSources`] does not contain a
///   [`PerspectivesOptionsData`] (consumer wired it up wrong).
///
/// Never panics — the resolver is read-only and tolerates every
/// missing-input branch.
pub struct PerspectiveFieldsResolver;

impl OptionsResolver for PerspectiveFieldsResolver {
    fn key(&self) -> &'static str {
        "perspective.fields"
    }

    fn resolve(&self, ctx: &OptionsContext<'_>) -> Vec<ParamOption> {
        let Some(sources) = ctx.data.downcast_ref::<OptionsSources>() else {
            return Vec::new();
        };
        let Some(data) = sources.get::<PerspectivesOptionsData>() else {
            return Vec::new();
        };
        let Some(perspective_id) = innermost_perspective_id(ctx.scope_chain) else {
            return Vec::new();
        };
        let Some(perspective) = data.perspectives.iter().find(|p| p.id == perspective_id) else {
            return Vec::new();
        };
        perspective
            .fields
            .iter()
            .map(|f| ParamOption {
                // `value` carries the field NAME (schema slug), not the
                // field ID. The dispatched `perspective.group` arg lands
                // on `PerspectiveDef.group`, which the frontend
                // `<GroupedBoardView>` reads via `task.fields[<group>]`
                // — and tasks key their values by field name. Persisted
                // perspective YAMLs at `.kanban/perspectives/*.yaml`
                // also store `group:` by name (`group: project`,
                // `group: tags`), so the end-to-end contract is "name
                // in, name out". Emitting field IDs here would land
                // every task in the `(ungrouped)` bucket. See task
                // `01KRH2EX1N1CA2HA3B4NMWZH67` for the regression.
                value: f.name.clone(),
                label: f.display_name.clone(),
            })
            .collect()
    }
}

/// Register every perspective-domain resolver onto the given registry.
///
/// Mirror this from the consumer that builds the registry; the
/// kanban-app's `default_options_registry()` calls it alongside the
/// view and sort-direction registrations.
pub fn register_perspective_resolvers(registry: &mut OptionsRegistry) {
    registry.register(Box::new(PerspectiveFieldsResolver));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::perspective_info::PerspectiveFieldInfo;

    /// Build an [`OptionsSources`] carrying one perspective with three
    /// fields. The three field names are the schema-slug shape (e.g.
    /// `"title"`, `"assignees"`) — that is what the dispatched
    /// `perspective.group` arg must carry end-to-end, because the
    /// persisted YAML's `group:` key and the frontend's
    /// `task.fields[<name>]` lookup both key by field name.
    fn fixture_sources() -> OptionsSources {
        let mut sources = OptionsSources::new();
        sources.insert(PerspectivesOptionsData {
            perspectives: vec![PerspectiveInfo {
                id: "01P".into(),
                name: "Active Sprint".into(),
                view: "grid".into(),
                fields: vec![
                    PerspectiveFieldInfo {
                        id: "01F1".into(),
                        name: "title".into(),
                        display_name: "Title".into(),
                    },
                    PerspectiveFieldInfo {
                        id: "01F2".into(),
                        name: "status".into(),
                        display_name: "Status".into(),
                    },
                    PerspectiveFieldInfo {
                        id: "01F3".into(),
                        name: "priority".into(),
                        display_name: "Priority".into(),
                    },
                ],
            }],
        });
        sources
    }

    /// With `perspective:01P` in scope and a fixture perspective that
    /// carries three fields, the resolver returns three
    /// [`ParamOption`]s in field order with `value = field_name` and
    /// `label = field_display_name`. Pins the wire format the
    /// frontend `<CommandPopover>` will consume.
    ///
    /// **`value` must be the field NAME, not the field ID.** The
    /// dispatched `perspective.group` arg flows through to
    /// `<GroupedBoardView>` / `computeGroups`, which reads
    /// `task.fields[<arg>]`. Tasks key their values by field name
    /// (`task.fields["assignees"]`, `task.fields["tags"]`), so the
    /// picker option `value` must match. Persisted perspective YAMLs
    /// at `.kanban/perspectives/*.yaml` also store `group:` by name
    /// (`group: project`, `group: tags`), so the wire contract is
    /// "name in, name out" — emitting field IDs here lands every task
    /// in the `(ungrouped)` bucket. Regression for task
    /// `01KRH2EX1N1CA2HA3B4NMWZH67`.
    #[test]
    fn perspective_fields_resolver_returns_fields_for_in_scope_perspective() {
        let sources = fixture_sources();
        let scope = vec!["perspective:01P".to_string()];
        let ctx = OptionsContext {
            scope_chain: &scope,
            data: &sources as &dyn std::any::Any,
        };
        let opts = PerspectiveFieldsResolver.resolve(&ctx);
        assert_eq!(opts.len(), 3);
        assert_eq!(opts[0].value, "title");
        assert_eq!(opts[0].label, "Title");
        assert_eq!(opts[1].value, "status");
        assert_eq!(opts[1].label, "Status");
        assert_eq!(opts[2].value, "priority");
        assert_eq!(opts[2].label, "Priority");
    }

    /// With an empty scope chain, the resolver returns an empty list
    /// rather than panicking. This is what lets a perspective field
    /// picker be safely rendered in a context where no perspective is
    /// open yet (e.g. a board-level command palette).
    #[test]
    fn perspective_fields_resolver_returns_empty_when_no_perspective_in_scope() {
        let sources = fixture_sources();
        let scope: Vec<String> = Vec::new();
        let ctx = OptionsContext {
            scope_chain: &scope,
            data: &sources as &dyn std::any::Any,
        };
        let opts = PerspectiveFieldsResolver.resolve(&ctx);
        assert!(opts.is_empty());
    }

    /// An in-scope `perspective:{id}` that does NOT match any
    /// perspective in the data slice also resolves to an empty list
    /// (not a panic, not a fallback to another perspective). Pins
    /// the "unknown perspective" behavior the picker relies on when
    /// the scope chain is stale.
    #[test]
    fn perspective_fields_resolver_empty_when_perspective_id_unknown() {
        let sources = fixture_sources();
        let scope = vec!["perspective:nope".to_string()];
        let ctx = OptionsContext {
            scope_chain: &scope,
            data: &sources as &dyn std::any::Any,
        };
        let opts = PerspectiveFieldsResolver.resolve(&ctx);
        assert!(opts.is_empty());
    }

    /// When the consumer threads an [`OptionsSources`] that doesn't
    /// contain a [`PerspectivesOptionsData`], the resolver returns
    /// an empty list. Pins the graceful degradation that lets the
    /// resolver be registered on a registry whose consumer doesn't
    /// always populate perspective data (e.g. a board-only headless
    /// test).
    #[test]
    fn perspective_fields_resolver_empty_when_sources_missing_perspectives_data() {
        let sources = OptionsSources::new();
        let scope = vec!["perspective:01P".to_string()];
        let ctx = OptionsContext {
            scope_chain: &scope,
            data: &sources as &dyn std::any::Any,
        };
        let opts = PerspectiveFieldsResolver.resolve(&ctx);
        assert!(opts.is_empty());
    }

    /// `register_perspective_resolvers` adds [`PerspectiveFieldsResolver`]
    /// under the canonical key. Pins the registration helper's
    /// contract so a future addition (or removal) of a resolver
    /// surfaces here.
    #[test]
    fn register_perspective_resolvers_adds_perspective_fields_resolver() {
        let mut registry = OptionsRegistry::new();
        register_perspective_resolvers(&mut registry);
        assert!(registry.has("perspective.fields"));
    }
}
