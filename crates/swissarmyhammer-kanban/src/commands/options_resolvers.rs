//! Kanban-side composition of every domain crate's built-in
//! [`OptionsResolver`]s into a single [`OptionsRegistry`], plus the one
//! resolver kanban owns: [`AiModelsResolver`].
//!
//! Built-in resolvers contributed by the domain crates are registered
//! here via the per-domain helpers:
//!
//! - `perspective.fields` — via
//!   [`swissarmyhammer_perspectives::register_perspective_resolvers`]
//! - `view.kinds` — via
//!   [`swissarmyhammer_views::register_view_resolvers`]
//! - `sort.directions` — via
//!   [`swissarmyhammer_commands::register_command_resolvers`]
//!
//! In addition, kanban owns the `ai.models` resolver — the AI panel
//! command scope (`ai.*` commands, declared in `builtin/commands/ai.yaml`)
//! is a kanban-domain concept, so the resolver that backs the
//! `ai.model` command's model picker lives here. It is registered via
//! [`register_kanban_resolvers`].
//!
//! [`default_options_registry`] returns a fresh [`OptionsRegistry`]
//! with every built-in pre-registered. Consumers (the kanban-app GUI,
//! the kanban-cli, headless tests) call this exactly once at startup
//! and thread the registry into every
//! [`crate::scope_commands::commands_for_scope`] invocation.
//!
//! [`OptionsRegistry`]: swissarmyhammer_commands::OptionsRegistry
//! [`OptionsResolver`]: swissarmyhammer_commands::OptionsResolver

use serde::{Deserialize, Serialize};
use swissarmyhammer_commands::{
    register_command_resolvers, OptionsContext, OptionsRegistry, OptionsResolver, OptionsSources,
    ParamOption,
};
use swissarmyhammer_perspectives::register_perspective_resolvers;
use swissarmyhammer_views::register_view_resolvers;

/// A selectable AI model, projected onto the `ai.model` command's
/// `model` picker.
///
/// Deliberately minimal: only the wire `value` (the model id) and the
/// human-readable `label` the picker needs. Richer model metadata
/// (`kind`, `available`, `hint`) lives on the GUI-side `Model` type in
/// `apps/kanban-app/src/ai/models.rs` — the pure-domain kanban crate
/// does not need it to fill a picker option list.
///
/// The model set itself is discovered by `swissarmyhammer-config`'s
/// `ModelManager` (filesystem agent discovery + Claude CLI detection),
/// which the kanban crate intentionally does not depend on. The GUI
/// runtime enumerates the models (via `ai_list_models`) and threads
/// the projected [`AiModelInfo`] list in through
/// [`crate::scope_commands::DynamicSources::ai_models`] — exactly the
/// consumer-supplied-data pattern `PerspectivesOptionsData` uses for
/// the `perspective.fields` resolver.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiModelInfo {
    /// Stable model id — the `swissarmyhammer-config` agent name. This
    /// is the value the dispatched `ai.model` arg carries and the
    /// frontend's per-board model-selection handler applies.
    pub id: String,
    /// Human-readable label shown in the picker.
    pub label: String,
}

/// AI-domain data carried via the shared [`OptionsSources`].
///
/// The consumer (the kanban-app aggregator builder) populates this once
/// per `commands_for_scope` call from
/// [`crate::scope_commands::DynamicSources::ai_models`] and inserts it
/// into the [`OptionsSources`]. [`AiModelsResolver::resolve`] retrieves
/// it via `OptionsSources::get::<AiOptionsData>()`.
///
/// Mirrors `swissarmyhammer_perspectives::PerspectivesOptionsData` —
/// an owned `Vec` rather than a borrowed slice because
/// [`std::any::Any`] requires `'static`. The list is small (one entry
/// per configured model) so the per-call clone cost is negligible.
#[derive(Debug, Clone, Default)]
pub struct AiOptionsData {
    /// The selectable models, in enumeration order. Construction order
    /// is preserved through to the resolver's output.
    pub models: Vec<AiModelInfo>,
}

/// Resolve `"ai.models"` to the configured AI model list.
///
/// Scope-independent: the model set does not depend on the focus
/// context, so the resolver ignores [`OptionsContext::scope_chain`] and
/// reads the consumer-supplied [`AiOptionsData`] straight out of the
/// [`OptionsSources`] container.
///
/// Returns an empty `Vec` when:
///
/// - the context's [`OptionsSources`] does not contain an
///   [`AiOptionsData`] (consumer wired it up wrong, or a headless test
///   that does not feed AI data), or
/// - the data slice carries no models.
///
/// Never panics — the resolver is read-only and tolerates every
/// missing-input branch, exactly like `PerspectiveFieldsResolver`.
pub struct AiModelsResolver;

impl OptionsResolver for AiModelsResolver {
    fn key(&self) -> &'static str {
        "ai.models"
    }

    fn resolve(&self, ctx: &OptionsContext<'_>) -> Vec<ParamOption> {
        let Some(sources) = ctx.data.downcast_ref::<OptionsSources>() else {
            return Vec::new();
        };
        let Some(data) = sources.get::<AiOptionsData>() else {
            return Vec::new();
        };
        data.models
            .iter()
            .map(|m| ParamOption {
                // `value` carries the model id — the wire value the
                // dispatched `ai.model` arg lands on and the frontend's
                // per-board model-selection handler applies.
                value: m.id.clone(),
                label: m.label.clone(),
            })
            .collect()
    }
}

/// Register every kanban-domain resolver onto the given registry.
///
/// Kanban owns exactly one resolver — [`AiModelsResolver`] — because the
/// `ai.*` command scope is a kanban-domain concept. Mirror this from the
/// consumer that builds the registry; [`default_options_registry`] calls
/// it alongside the perspective, view, and sort-direction registrations.
pub fn register_kanban_resolvers(registry: &mut OptionsRegistry) {
    registry.register(Box::new(AiModelsResolver));
}

/// Construct an [`OptionsRegistry`] with every built-in resolver
/// pre-registered.
///
/// Built-ins registered, in registration order:
///
/// 1. `perspective.fields` — from `swissarmyhammer-perspectives`
///    via [`register_perspective_resolvers`]
/// 2. `view.kinds` — from `swissarmyhammer-views` via
///    [`register_view_resolvers`]
/// 3. `sort.directions` — from `swissarmyhammer-commands` via
///    [`register_command_resolvers`]
/// 4. `ai.models` — kanban's own, via [`register_kanban_resolvers`]
///
/// Downstream consumers can further [`OptionsRegistry::register`]
/// additional resolvers on top of the returned registry.
///
/// [`OptionsRegistry`]: swissarmyhammer_commands::OptionsRegistry
pub fn default_options_registry() -> OptionsRegistry {
    let mut registry = OptionsRegistry::new();
    register_perspective_resolvers(&mut registry);
    register_view_resolvers(&mut registry);
    register_command_resolvers(&mut registry);
    register_kanban_resolvers(&mut registry);
    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    /// [`default_options_registry`] must register every built-in
    /// resolver. This guard fires if a new built-in is added to any
    /// domain crate without being wired into this composition.
    #[test]
    fn default_options_registry_registers_every_builtin_key() {
        let registry = default_options_registry();
        for key in [
            "perspective.fields",
            "view.kinds",
            "sort.directions",
            "ai.models",
        ] {
            assert!(
                registry.has(key),
                "default_options_registry must register `{key}`",
            );
        }
    }

    /// `register_kanban_resolvers` adds [`AiModelsResolver`] under the
    /// canonical `ai.models` key. Pins the registration helper's
    /// contract so a future addition (or removal) of a kanban resolver
    /// surfaces here.
    #[test]
    fn register_kanban_resolvers_adds_ai_models_resolver() {
        let mut registry = OptionsRegistry::new();
        register_kanban_resolvers(&mut registry);
        assert!(registry.has("ai.models"));
    }

    /// Build an [`OptionsSources`] carrying two AI models so the
    /// `ai.models` resolver has data to project.
    fn fixture_sources() -> OptionsSources {
        let mut sources = OptionsSources::new();
        sources.insert(AiOptionsData {
            models: vec![
                AiModelInfo {
                    id: "claude-code".into(),
                    label: "Claude Code".into(),
                },
                AiModelInfo {
                    id: "qwen".into(),
                    label: "Qwen Coder".into(),
                },
            ],
        });
        sources
    }

    /// The resolver projects every [`AiModelInfo`] onto a [`ParamOption`]
    /// in enumeration order, with `value = model id` and
    /// `label = model label`. Pins the wire format the frontend
    /// `<CommandPopover>` consumes for the `ai.model` picker.
    #[test]
    fn ai_models_resolver_projects_every_model_in_order() {
        let sources = fixture_sources();
        let scope: Vec<String> = Vec::new();
        let ctx = OptionsContext {
            scope_chain: &scope,
            data: &sources as &dyn std::any::Any,
        };
        let opts = AiModelsResolver.resolve(&ctx);
        assert_eq!(opts.len(), 2, "two models → two ParamOption entries");
        assert_eq!(opts[0].value, "claude-code");
        assert_eq!(opts[0].label, "Claude Code");
        assert_eq!(opts[1].value, "qwen");
        assert_eq!(opts[1].label, "Qwen Coder");
    }

    /// The resolver is scope-independent — the same model list comes
    /// back regardless of what monikers are in scope.
    #[test]
    fn ai_models_resolver_is_scope_independent() {
        let sources = fixture_sources();
        let scope = vec!["board:my-board".to_string(), "task:01X".to_string()];
        let ctx = OptionsContext {
            scope_chain: &scope,
            data: &sources as &dyn std::any::Any,
        };
        let opts = AiModelsResolver.resolve(&ctx);
        assert_eq!(opts.len(), 2);
        assert_eq!(opts[0].value, "claude-code");
    }

    /// When the consumer threads an [`OptionsSources`] that does not
    /// contain an [`AiOptionsData`], the resolver returns an empty list
    /// rather than panicking. Pins the graceful degradation that lets
    /// the resolver be registered on a registry whose consumer does not
    /// always populate AI data (e.g. a headless test).
    #[test]
    fn ai_models_resolver_empty_when_sources_missing_ai_data() {
        let sources = OptionsSources::new();
        let scope: Vec<String> = Vec::new();
        let ctx = OptionsContext {
            scope_chain: &scope,
            data: &sources as &dyn std::any::Any,
        };
        let opts = AiModelsResolver.resolve(&ctx);
        assert!(opts.is_empty());
    }

    /// An [`AiOptionsData`] with no models resolves to an empty list —
    /// a valid "this machine has no selectable models" answer, distinct
    /// from "no resolver registered".
    #[test]
    fn ai_models_resolver_empty_when_no_models_configured() {
        let mut sources = OptionsSources::new();
        sources.insert(AiOptionsData::default());
        let scope: Vec<String> = Vec::new();
        let ctx = OptionsContext {
            scope_chain: &scope,
            data: &sources as &dyn std::any::Any,
        };
        let opts = AiModelsResolver.resolve(&ctx);
        assert!(opts.is_empty());
    }
}
