//! Backend-driven command resolution for a given scope chain.
//!
//! `commands_for_scope` is the single source of truth for what commands
//! are available in a given focus context. It walks the scope chain,
//! merges per-moniker cross-cutting and scoped-registry commands with
//! global registry commands, checks availability, and resolves all
//! template names (e.g. `{{entity.type}}` → "Task").
//!
//! ## Emission ordering
//!
//! For every entity moniker in the scope chain (innermost first), the
//! dispatcher emits commands in this order:
//!
//!   1. **cross-cutting** — registry commands whose primary param declares
//!      `from: target` (e.g. `ui.inspect`, `entity.delete`, `entity.archive`,
//!      `entity.unarchive`). Surfaces uniformly on every entity moniker
//!      without needing per-type opt-in YAML. See `emit_cross_cutting_commands`.
//!      Like `emit_entity_add`, this pass logs a `debug` trace at every
//!      decision point (entry counts, per-command include/filter outcome,
//!      dedup skips) so a missing cross-cutting command on a given entity
//!      can be diagnosed from logs alone — see the task
//!      `Commands: tracing for emit_cross_cutting_commands` for the
//!      capture protocol via `log show --predicate 'subsystem == "com.swissarmyhammer.kanban"'`.
//!   2. **scoped-registry** — registry commands with `scope:` pinned to this
//!      entity type (e.g. `task.untag` with `scope: "entity:tag,entity:task"`).
//!
//! After all monikers are processed:
//!
//!   3. **global-registry** — registry commands with no `scope:` pin
//!      (e.g. `app.quit`, `app.undo`).
//!   4. **dynamic** — runtime-generated rows such as per-view "Switch to X"
//!      entries (emitted as the canonical `view.set` command with a
//!      pre-filled `args.view_id`), per-perspective "Go to Perspective: X"
//!      entries (emitted as `perspective.switch` with `args.perspective_id`),
//!      plus the prefix-id rows `board.switch:{path}` and
//!      `entity.add:{type}`.
//!
//! Within each phase, the shared `(id, target)` seen-set guarantees a command
//! cannot double-emit for the same target.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use crate::commands_core::{
    Command, CommandContext, CommandDef, CommandsRegistry, KeysDef, OptionsContext,
    OptionsRegistry, ParamDef, ParamSource, TabButtonDef,
};
use swissarmyhammer_common::WindowInfo;
use swissarmyhammer_ui_state::UIState;
use swissarmyhammer_fields::FieldsContext;
use swissarmyhammer_perspectives::PerspectiveInfo;
use swissarmyhammer_views::ViewInfo;

/// Lightweight open-board descriptor for dynamic command generation.
///
/// Only carries the fields needed to produce a `board.switch:{path}` command.
#[derive(Debug, Clone)]
pub struct BoardInfo {
    /// Canonical filesystem path of the board.
    pub path: String,
    /// Human-readable board name (directory basename or custom name).
    pub name: String,
    /// Entity display name (the board entity's `name` field value, or empty).
    pub entity_name: String,
    /// Context display name (from `KanbanContext::name()`, the path stem).
    pub context_name: String,
}

/// Runtime data that feeds dynamic command generation beyond the static
/// registry and entity schemas.
#[derive(Debug, Clone, Default)]
pub struct DynamicSources {
    /// Loaded view definitions — each generates a `view.set` palette row
    /// with `args.view_id` pre-filled.
    pub views: Vec<ViewInfo>,
    /// Open boards — each generates a `board.switch:{path}` command.
    pub boards: Vec<BoardInfo>,
    /// Open windows — each generates a `window.focus:{label}` command.
    pub windows: Vec<WindowInfo>,
    /// Perspectives — each generates a `perspective.switch` palette row with
    /// `args.perspective_id` pre-filled.
    pub perspectives: Vec<PerspectiveInfo>,
    /// Selectable AI models — feeds the `ai.models` options resolver so the
    /// `ai.model` command palette popover offers the configured models
    /// instead of a free-text box.
    ///
    /// Unlike views/boards/perspectives this list generates no dynamic
    /// command rows; it is consumer-supplied runtime data. The model set is
    /// discovered by `swissarmyhammer-config`'s `ModelManager`, which the
    /// pure-domain kanban crate does not depend on — the GUI runtime
    /// enumerates it (via `ai_list_models`) and threads it in here.
    pub ai_models: Vec<crate::commands::options_resolvers::AiModelInfo>,
}

/// A fully resolved command ready for display in a menu, palette, or context menu.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ResolvedCommand {
    /// Command ID (e.g. "entity.copy").
    ///
    /// For dynamic palette entries that fan out a single canonical command
    /// across a runtime-discovered set of targets (e.g. one
    /// "Switch to <ViewName>" row per view), multiple resolved commands can
    /// share the same `id` — the distinguishing information lives in
    /// [`Self::args`]. Consumers that need per-row identity (React keys,
    /// test ids, dedup keys) must combine `id` with `target` and `args`.
    pub id: String,
    /// Fully resolved display name (e.g. "Copy Tag", never "Copy {{entity.type}}").
    pub name: String,
    /// Resolved menu display name. Falls back to `name` when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub menu_name: Option<String>,
    /// Target moniker (e.g. "tag:01X") or None for global commands.
    pub target: Option<String>,
    /// Group for separator insertion (entity type like "tag", "task", or "global").
    pub group: String,
    /// Whether this command should appear in context menus.
    pub context_menu: bool,
    /// Keybindings per keymap mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keys: Option<KeysDef>,
    /// Whether the command is currently available (enabled).
    pub available: bool,
    /// Pre-filled arguments to pass to the dispatcher alongside `id`.
    ///
    /// Used by dynamic palette entries that invoke a canonical command
    /// with per-row state (e.g. `view.set` with `{"view_id": "..."}`
    /// emitted one row per known view). When absent, the dispatcher
    /// receives no additional arguments beyond whatever the caller
    /// supplies at dispatch time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<serde_json::Value>,
    /// Parameter definitions forwarded from the source [`CommandDef`].
    ///
    /// Empty for dynamic / synthetic rows that have no backing
    /// `CommandDef`. For registry-driven rows the list mirrors
    /// `CommandDef.params` with one important enrichment: when a
    /// param declares `options_from`, the options enrichment pass at
    /// `commands_for_scope` emission time fills `options` from the
    /// backend [`crate::commands_core::OptionsRegistry`] so the
    /// frontend never has to invent picker options.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<ParamDef>,
    /// Optional tab-button affordance forwarded from the source
    /// [`CommandDef`]. `None` for dynamic / synthetic rows and for
    /// any registry command whose YAML omits `tab_button`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab_button: Option<TabButtonDef>,
}

/// Parameters for template resolution in command names.
#[derive(Debug, Clone, Default)]
pub struct TemplateParams<'a> {
    /// Entity type (e.g. "task") — resolved as capitalized for `{{entity.type}}`.
    pub entity_type: &'a str,
    /// Entity display name (e.g. the entity's `name` field value).
    /// Resolves `{{entity.display_name}}`; empty string if not set.
    pub entity_name: &'a str,
    /// Context display name (e.g. `KanbanContext::name()`, the directory stem).
    /// Resolves `{{entity.context.display_name}}`; empty string if not set.
    pub context_name: &'a str,
}

/// Resolve template variables in a command name or menu_name.
///
/// Supported variables:
/// - `{{entity.type}}` → capitalized entity type (e.g. "Task")
/// - `{{entity.display_name}}` → entity name field value
/// - `{{entity.context.display_name}}` → context path stem
///
/// Missing values resolve to empty string. Each variable is independent.
pub fn resolve_name_template(name: &str, params: &TemplateParams<'_>) -> String {
    if !name.contains("{{") {
        return name.to_string();
    }
    let mut result = name.to_string();
    if result.contains("{{entity.type}}") {
        let entity_type = params.entity_type;
        let capitalized = if entity_type.is_empty() {
            String::new()
        } else {
            format!("{}{}", &entity_type[..1].to_uppercase(), &entity_type[1..])
        };
        result = result.replace("{{entity.type}}", &capitalized);
    }
    // Note: context.display_name must be checked before display_name to avoid
    // partial matching on the shorter template.
    if result.contains("{{entity.context.display_name}}") {
        result = result.replace("{{entity.context.display_name}}", params.context_name);
    }
    if result.contains("{{entity.display_name}}") {
        result = result.replace("{{entity.display_name}}", params.entity_name);
    }
    result
}

/// Check command availability by building a CommandContext and calling `available()`.
///
/// If no Rust impl exists (client-side command like entity.inspect), returns true —
/// the command is assumed available if it's declared in the entity schema.
fn check_available(
    cmd_id: &str,
    scope_chain: &[String],
    target: Option<&str>,
    command_impls: &HashMap<String, Arc<dyn Command>>,
    ui_state: &Arc<UIState>,
) -> bool {
    let Some(cmd_impl) = command_impls.get(cmd_id) else {
        return true;
    };
    let ctx = CommandContext::new(
        cmd_id,
        scope_chain.to_vec(),
        target.map(|s| s.to_string()),
        HashMap::new(),
    )
    .with_ui_state(Arc::clone(ui_state));
    cmd_impl.available(&ctx)
}

/// Identity key used across all `emit_*` helpers to collapse duplicates.
///
/// The tuple captures three axes that together distinguish one emitted row
/// from another:
///
///   * the command `id`,
///   * the per-row `target` moniker (empty for global rows), and
///   * a canonical serialization of `args` (empty when absent).
///
/// The `args` axis is load-bearing for fan-out palette entries such as the
/// "Switch to <ViewName>" rows emitted by `emit_view_switch`: every row
/// shares `id == "view.set"` and `target == None`, so without `args` in the
/// key they would all collapse to a single entry. The serialization uses
/// `serde_json::to_string` so two equal `Value` payloads hash identically.
type SeenKey = (String, Option<String>, Option<String>);

/// Build a [`SeenKey`] from a row's `id`, `target`, and `args`.
///
/// The `args` JSON value is serialized to a canonical string — when two rows
/// carry equivalent argument maps (same keys, same values) the resulting
/// strings match and dedup collapses them. Returns `None` for the args slot
/// when the row has no args, so the common case (no args) matches the same
/// key shape as before the `args` axis was added.
fn seen_key_of(cmd: &ResolvedCommand) -> SeenKey {
    let args_key = cmd
        .args
        .as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_default());
    (cmd.id.clone(), cmd.target.clone(), args_key)
}

/// Push a command once, honoring the `(id, target, args)` seen-set for dedup.
///
/// Shared across all `emit_*` helpers below so that overlapping emitters (and
/// repeated scope monikers) can never produce duplicate commands in the same
/// `commands_for_scope` result.
fn push_dedup(
    seen: &mut HashSet<SeenKey>,
    result: &mut Vec<ResolvedCommand>,
    cmd: ResolvedCommand,
) {
    let key = seen_key_of(&cmd);
    if seen.contains(&key) {
        return;
    }
    seen.insert(key);
    result.push(cmd);
}

/// Emit one "Switch to <ViewName>" palette row per known view, each one
/// dispatching the canonical `view.set` command with its `view_id`
/// pre-filled in `args`.
///
/// Always marked `context_menu: false` — view switching is a palette-only
/// navigation action, alongside `board.switch`, `window.focus`, and the
/// sibling `perspective.switch` fan-out. Right-clicking a view button never
/// surfaces a "Switch to <ViewName>" entry; the palette
/// (`context_menu_only == false`) still lists one row per view.
///
/// Shares `seen` with the other emit_* helpers so cross-emitter dedup works.
/// Every emitted row has `id == "view.set"` and `target == None`; the
/// distinguishing information lives in `args["view_id"]`, which is why
/// `push_dedup`'s [`SeenKey`] includes the args serialization.
///
/// The wire format change retires the legacy `view.switch:{id}` id in favour
/// of the canonical `view.set` command with pre-filled args, removing the
/// dispatcher-side rewrite that previously translated the former into the
/// latter (PR #40, task 01KPZMXXEXKVE3RNPA4XJP0105).
fn emit_view_switch(
    views: &[ViewInfo],
    seen: &mut HashSet<SeenKey>,
    result: &mut Vec<ResolvedCommand>,
) {
    for view in views {
        push_dedup(
            seen,
            result,
            ResolvedCommand {
                id: "view.set".into(),
                name: format!("Switch to {}", view.name),
                menu_name: None,
                target: None,
                group: "view".into(),
                context_menu: false,
                keys: None,
                available: true,
                args: Some(serde_json::json!({ "view_id": view.id })),
                params: Vec::new(),
                tab_button: None,
            },
        );
    }
}

/// Emit one `board.switch:{path}` command per known board.
///
/// The display name is template-resolved against the board's display and
/// context names. Marked `context_menu: false` (palette-only). Shares `seen`
/// with the other emit_* helpers so cross-emitter dedup works.
fn emit_board_switch(
    boards: &[BoardInfo],
    seen: &mut HashSet<SeenKey>,
    result: &mut Vec<ResolvedCommand>,
) {
    for board in boards {
        let tpl = TemplateParams {
            entity_type: "board",
            entity_name: &board.entity_name,
            context_name: &board.context_name,
        };
        let name = resolve_name_template(
            "Switch to Board: {{entity.display_name}} ({{entity.context.display_name}})",
            &tpl,
        );
        let menu_name = resolve_name_template("{{entity.context.display_name}}", &tpl);
        push_dedup(
            seen,
            result,
            ResolvedCommand {
                id: format!("board.switch:{}", board.path),
                name,
                menu_name: Some(menu_name),
                target: None,
                group: "board".into(),
                context_menu: false,
                keys: None,
                available: true,
                args: None,
                params: Vec::new(),
                tab_button: None,
            },
        );
    }
}

/// Emit one `window.focus:{label}` command per known window.
///
/// The displayed name is the window's title (e.g. the board path it shows).
/// Marked `context_menu: false` (palette-only). Shares `seen` with the other
/// emit_* helpers so cross-emitter dedup works.
fn emit_window_focus(
    windows: &[WindowInfo],
    seen: &mut HashSet<SeenKey>,
    result: &mut Vec<ResolvedCommand>,
) {
    for window in windows {
        push_dedup(
            seen,
            result,
            ResolvedCommand {
                id: format!("window.focus:{}", window.label),
                name: window.title.clone(),
                menu_name: Some(window.title.clone()),
                target: None,
                group: "window".into(),
                context_menu: false,
                keys: None,
                available: true,
                args: None,
                params: Vec::new(),
                tab_button: None,
            },
        );
    }
}

/// Emit one "Go to Perspective: <Name>" palette row per known perspective,
/// each dispatching the canonical `perspective.switch` command with its
/// `perspective_id` pre-filled in `args`.
///
/// Marked `context_menu: false` (palette-only). Shares `seen` with the other
/// emit_* helpers so cross-emitter dedup works. Every emitted row has
/// `id == "perspective.switch"` and `target == None`; the distinguishing
/// information lives in `args["perspective_id"]`, which is why
/// `push_dedup`'s [`SeenKey`] includes the args serialization.
///
/// The wire format change retires the legacy `perspective.goto:{id}` id in
/// favour of the canonical `perspective.switch` command with pre-filled
/// args, removing the dispatcher-side rewrite that previously translated
/// the former into the latter (PR #40, task 01KPZMXXEXKVE3RNPA4XJP0105).
/// 01KP3ERHEDP86C2JYYR7NM1593 then replaced `perspective.set` with
/// `perspective.switch`, which collapses the prior two-step (set id +
/// frontend filter fetch) into one atomic backend command.
fn emit_perspective_goto(
    perspectives: &[PerspectiveInfo],
    seen: &mut HashSet<SeenKey>,
    result: &mut Vec<ResolvedCommand>,
) {
    for perspective in perspectives {
        push_dedup(
            seen,
            result,
            ResolvedCommand {
                id: "perspective.switch".into(),
                name: format!("Go to Perspective: {}", perspective.name),
                menu_name: None,
                target: None,
                group: "perspective".into(),
                context_menu: false,
                keys: None,
                available: true,
                args: Some(serde_json::json!({ "perspective_id": perspective.id })),
                params: Vec::new(),
                tab_button: None,
            },
        );
    }
}

/// Resolve a `view:*` moniker to the non-empty `entity_type` that should
/// power its dynamic `entity.add:{type}` command, or `None` if no such
/// command should be emitted. Logs a `debug` trace at each decision point.
fn resolve_entity_type_for_moniker<'a>(
    moniker: &str,
    views_by_id: &'a HashMap<&str, &ViewInfo>,
) -> Option<&'a str> {
    let view_id = moniker.strip_prefix("view:")?;
    let Some(view) = views_by_id.get(view_id) else {
        tracing::debug!(
            scope_moniker = %moniker,
            view_id = %view_id,
            known_view_count = views_by_id.len(),
            "emit_entity_add: view moniker has no matching ViewInfo — \
             gather_views did not populate this view"
        );
        return None;
    };
    let Some(entity_type) = view.entity_type.as_deref() else {
        tracing::debug!(
            scope_moniker = %moniker,
            view_id = %view_id,
            view_name = %view.name,
            "emit_entity_add: view has no entity_type — skipping (dashboard-style view)"
        );
        return None;
    };
    if entity_type.is_empty() {
        tracing::debug!(
            scope_moniker = %moniker,
            view_id = %view_id,
            view_name = %view.name,
            "emit_entity_add: view entity_type is empty string — skipping"
        );
        return None;
    }
    Some(entity_type)
}

/// Emit `entity.add:{type}` commands for each view type in the scope chain.
///
/// Surfaces only when a `view:{id}` moniker is present and the matching view
/// declares an `entity_type`. One command per distinct type; dedup handles
/// overlapping views. Marked `context_menu: true` because creation is a
/// first-class action, unlike navigation dynamics.
///
/// Emits `debug` traces at each decision point so a missing entity.add in
/// the final result can be diagnosed from logs alone — see the task
/// `Fix "New" so it works uniformly on every grid` for the intended
/// capture protocol.
fn emit_entity_add(
    views_by_id: &HashMap<&str, &ViewInfo>,
    scope_chain: &[String],
    seen: &mut HashSet<SeenKey>,
    result: &mut Vec<ResolvedCommand>,
) {
    for moniker in scope_chain {
        let Some(entity_type) = resolve_entity_type_for_moniker(moniker, views_by_id) else {
            continue;
        };
        let tpl = TemplateParams {
            entity_type,
            ..Default::default()
        };
        let cmd_id = format!("entity.add:{entity_type}");
        tracing::debug!(
            scope_moniker = %moniker,
            entity_type = %entity_type,
            cmd_id = %cmd_id,
            "emit_entity_add: pushing dynamic command"
        );
        push_dedup(
            seen,
            result,
            ResolvedCommand {
                id: cmd_id,
                name: resolve_name_template("New {{entity.type}}", &tpl),
                menu_name: None,
                target: None,
                group: "entity".into(),
                context_menu: true,
                keys: None,
                available: true,
                args: None,
                params: Vec::new(),
                tab_button: None,
            },
        );
    }
}

/// Emit dynamic commands from runtime data into the result list.
///
/// Generates per-view and per-perspective palette rows (dispatching
/// `view.set` / `perspective.switch` directly with pre-filled args),
/// `board.switch:{path}`, `window.focus:{label}`, and `entity.add:{type}`
/// commands from the dynamic sources. Skips commands already in the
/// `seen` set.
///
/// `entity.add:{type}` is the only dynamic command that depends on the current
/// scope chain: it surfaces only when a `view:{id}` moniker is active and
/// the matching view declares an `entity_type`. Unlike the navigation
/// dynamics (view switching, board switching, perspective switching,
/// window focus) which all set `context_menu: false`, `entity.add:*` is a
/// first-class creation action and is emitted with `context_menu: true` so
/// it appears on right-click inside the view.
///
/// The dispatch-side handler that actually creates the entity lives in
/// `crate::entity::add::AddEntity`; `entity.add:{type}` monikers produced
/// here are rewritten to the canonical `entity.add` command in
/// `kanban-app/src/commands.rs::dispatch_command_internal` and routed into
/// that operation.
fn emit_dynamic_commands(
    dyn_src: &DynamicSources,
    scope_chain: &[String],
    seen: &mut HashSet<SeenKey>,
    result: &mut Vec<ResolvedCommand>,
) {
    // Index views by id once so the `entity.add` emission below is O(scope)
    // rather than O(scope × views).
    let views_by_id: HashMap<&str, &ViewInfo> =
        dyn_src.views.iter().map(|v| (v.id.as_str(), v)).collect();
    emit_view_switch(&dyn_src.views, seen, result);
    emit_board_switch(&dyn_src.boards, seen, result);
    emit_window_focus(&dyn_src.windows, seen, result);
    emit_perspective_goto(&dyn_src.perspectives, seen, result);
    emit_entity_add(&views_by_id, scope_chain, seen, result);
}

/// Compute all available commands for a given scope chain.
///
/// This is the single source of truth for what commands are available.
/// The frontend calls this and renders the result — no command logic in the UI.
///
/// # Arguments
/// - `scope_chain` — Monikers from innermost to outermost (e.g. `["tag:bug", "task:01X", "column:todo", "board:board"]`)
/// - `registry` — The command definitions registry (YAML-loaded)
/// - `command_impls` — Rust Command trait implementations
/// - `fields` — Entity type schemas (for entity-declared commands)
/// - `ui_state` — UI state (clipboard, etc.)
/// - `context_menu_only` — If true, only return commands with `context_menu: true`
/// - `dynamic` — Runtime data for view-switch and board-switch commands
/// - `options_registry` — Backend resolver registry consulted by the
///   options-enrichment pass. When `Some`, every emitted param whose
///   YAML declared `options_from` has its `options` filled in by the
///   matching [`OptionsResolver`]. When `None`, every param keeps
///   whatever YAML supplied (degenerate path for surfaces that don't
///   render pickers, e.g. the native menu bar).
#[allow(clippy::too_many_arguments)]
pub fn commands_for_scope(
    scope_chain: &[String],
    registry: &CommandsRegistry,
    command_impls: &HashMap<String, Arc<dyn Command>>,
    fields: Option<&FieldsContext>,
    ui_state: &Arc<UIState>,
    context_menu_only: bool,
    dynamic: Option<&DynamicSources>,
    options_registry: Option<&OptionsRegistry>,
) -> Vec<ResolvedCommand> {
    let mut result: Vec<ResolvedCommand> = Vec::new();
    let mut seen: HashSet<SeenKey> = HashSet::new();

    let clipboard_type = ui_state.clipboard_entity_type();
    let all_registry_cmds = registry.available_commands(scope_chain);

    emit_scoped_commands(
        scope_chain,
        &all_registry_cmds,
        command_impls,
        fields,
        ui_state,
        clipboard_type.as_deref(),
        &mut seen,
        &mut result,
    );
    emit_global_registry_commands(
        &all_registry_cmds,
        scope_chain,
        command_impls,
        ui_state,
        clipboard_type.as_deref(),
        &mut seen,
        &mut result,
    );
    if let Some(dyn_src) = dynamic {
        emit_dynamic_commands(dyn_src, scope_chain, &mut seen, &mut result);
    }
    dedupe_by_id(&mut result);
    if context_menu_only {
        result.retain(|c| c.context_menu);
    }
    result.retain(|c| c.available);
    filter_by_view_kind(&mut result, scope_chain, &all_registry_cmds, dynamic);
    enrich_options(&mut result, scope_chain, options_registry, dynamic);

    result
}

/// Compute all available commands for a given scope chain, threading the
/// active [`KanbanContext`]'s [`FieldsContext`] and [`OptionsRegistry`]
/// through to [`commands_for_scope`] in one step.
///
/// This is the call shape every GUI / TUI / CLI consumer wants: pickers
/// must be enriched (so the popover can render real options) and the
/// entity schema must be available (so `entity.add` / cross-cutting
/// commands resolve their per-entity names). Both of those live on the
/// active [`KanbanContext`], so the helper takes the context as the
/// single source for both — eliminating the foot-gun where a caller
/// remembers to thread `fields()` but forgets `options_registry()` (or
/// vice versa), which silently empties every picker downstream.
///
/// This regression — caller threading the fields but passing `None` for
/// the options registry — is exactly what produced the empty Group By
/// popover the user reported in task `01KRGW1DYD0T05PSTEDPT5D076`
/// (iteration 4). The wrapper makes that mistake unrepresentable at the
/// type level: the registry is pulled directly from the context, so a
/// caller that has a context cannot forget to pass it.
///
/// # Arguments
/// - `scope_chain` — see [`commands_for_scope`].
/// - `registry` — see [`commands_for_scope`].
/// - `command_impls` — see [`commands_for_scope`].
/// - `active_context` — the context whose entity schemas and resolver
///   registry should be consulted. `None` when no board is focused
///   (splash / welcome path); in that case no perspectives or entities
///   exist so the enrichment pass has nothing to do.
/// - `ui_state` — see [`commands_for_scope`].
/// - `context_menu_only` — see [`commands_for_scope`].
/// - `dynamic` — see [`commands_for_scope`].
pub fn commands_for_scope_with_context(
    scope_chain: &[String],
    registry: &CommandsRegistry,
    command_impls: &HashMap<String, Arc<dyn Command>>,
    active_context: Option<&crate::context::KanbanContext>,
    ui_state: &Arc<UIState>,
    context_menu_only: bool,
    dynamic: Option<&DynamicSources>,
) -> Vec<ResolvedCommand> {
    let fields = active_context.and_then(|c| c.fields());
    let options_registry = active_context.map(|c| c.options_registry());
    commands_for_scope(
        scope_chain,
        registry,
        command_impls,
        fields,
        ui_state,
        context_menu_only,
        dynamic,
        options_registry,
    )
}

/// Walk every emitted [`ResolvedCommand`] and fill in each param's
/// `options` by consulting the [`OptionsRegistry`] for any param whose
/// YAML declared `options_from`.
///
/// Runs **after** [`filter_by_view_kind`] so resolvers do not waste work
/// on commands that won't appear in the final list. Param shape and
/// `options_from` come from the param itself (the registry-emit pass
/// already forwarded them from the source [`CommandDef`]); the resolver
/// produces a `Vec<ParamOption>` which is written into
/// [`ParamDef::options`] in place.
///
/// Params with `options_from = None` are untouched — their inline YAML
/// `options` (if any) flow through as-is.
///
/// When no registry is supplied (the `options_registry == None` path),
/// every param keeps whatever the YAML declared — this is the surface
/// the native menu bar uses, where no picker UI ever renders.
///
/// A missing resolver (key present in YAML but absent from the
/// registry) leaves `options: None` on the emitted param. The frontend
/// treats `None` as "this command can't be picked right now". A
/// `tracing::warn` is emitted once per unknown key to surface the
/// authoring mistake without flooding logs.
fn enrich_options(
    result: &mut [ResolvedCommand],
    scope_chain: &[String],
    options_registry: Option<&OptionsRegistry>,
    dynamic: Option<&DynamicSources>,
) {
    let Some(registry) = options_registry else {
        return;
    };
    // Compose a per-domain `OptionsSources` once per call from the
    // `DynamicSources` aggregator the consumer supplied. Each domain
    // crate's resolver downcasts the context's `data: &dyn Any` to
    // `&OptionsSources` and then pulls its own per-domain data via
    // [`OptionsSources::get`]. Resolvers that don't need any data
    // (sort.directions, view.kinds) ignore the lookup entirely.
    let sources = build_options_sources(dynamic);
    let ctx = OptionsContext {
        scope_chain,
        data: &sources as &dyn std::any::Any,
    };
    for cmd in result.iter_mut() {
        for param in cmd.params.iter_mut() {
            let Some(key) = param.options_from.as_deref() else {
                continue;
            };
            match registry.resolve(key, &ctx) {
                Some(options) => {
                    param.options = Some(options);
                }
                None => {
                    log_missing_resolver_once(key);
                    param.options = None;
                }
            }
        }
    }
}

/// Compose a fresh [`OptionsSources`] from the optional
/// [`DynamicSources`] aggregator.
///
/// Each per-domain `*OptionsData` is constructed by cloning the
/// matching slice out of the aggregator. The clone happens once per
/// `commands_for_scope` call, not per param — fan-out is O(commands
/// × params), so the per-call cost of one allocation per domain is
/// negligible compared to the registry walk.
///
/// When `dynamic == None`, every per-domain slot is left empty; this
/// matches the headless / shell path where the consumer doesn't have
/// any dynamic data to feed the resolvers.
fn build_options_sources(
    dynamic: Option<&DynamicSources>,
) -> crate::commands_core::OptionsSources {
    use crate::commands::options_resolvers::AiOptionsData;
    use crate::commands_core::OptionsSources;
    use swissarmyhammer_perspectives::PerspectivesOptionsData;
    let mut sources = OptionsSources::new();
    sources.insert(PerspectivesOptionsData {
        perspectives: dynamic.map(|d| d.perspectives.clone()).unwrap_or_default(),
    });
    sources.insert(AiOptionsData {
        models: dynamic.map(|d| d.ai_models.clone()).unwrap_or_default(),
    });
    sources
}

/// Process-wide "warn once per missing resolver key" gate.
///
/// Mirrors the `LOGGED_LEGACY_PERSPECTIVES` pattern: surface every
/// unknown `options_from` key the first time we see it so the YAML
/// authoring mistake shows up in logs, but never re-emit the same
/// warning for the same key (which would flood every `commands_for_scope`
/// invocation while the registry stays misconfigured).
fn log_missing_resolver_once(key: &str) {
    use std::sync::Mutex;
    use std::sync::OnceLock;
    static LOGGED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    let logged = LOGGED.get_or_init(|| Mutex::new(HashSet::new()));
    let Ok(mut guard) = logged.lock() else {
        return;
    };
    if guard.insert(key.to_string()) {
        tracing::warn!(
            options_from = %key,
            "commands_for_scope: param declares options_from key with no matching resolver in OptionsRegistry — options will be None"
        );
    }
}

/// Drop every emitted command whose YAML `view_kinds` filter does not
/// admit the currently-active view kind.
///
/// Resolution rules:
///
/// 1. Find the innermost `view:{id}` moniker in `scope_chain`.
/// 2. Look it up in `DynamicSources.views` to get the view's `kind`.
/// 3. For each resolved command in `result`, look up its `CommandDef` via
///    `id`. Commands with `view_kinds: None` are unconstrained (kept). A
///    command with `view_kinds: Some(list)` is kept iff the resolved kind
///    is present in the list.
/// 4. When no `view:{id}` is in scope or it does not match any known
///    [`ViewInfo`], the resolved kind is `None` — every command with a
///    non-empty `view_kinds` list is filtered out. This is the safe
///    default for headless / shell contexts: a grid-only command cannot
///    surface in a palette that has no view to anchor against.
///
/// Dynamic / synthetic command rows whose `id` does not appear in the
/// registry (e.g. the prefix-id `board.switch:{path}`, `entity.add:{type}`
/// rows) are never filtered — they are not user-authored commands and
/// have no `view_kinds` metadata to honor.
fn filter_by_view_kind(
    result: &mut Vec<ResolvedCommand>,
    scope_chain: &[String],
    all_registry_cmds: &[&CommandDef],
    dynamic: Option<&DynamicSources>,
) {
    // Build the id → CommandDef lookup once; the filter pass below otherwise
    // re-scans every CommandDef for every resolved row.
    let def_by_id: HashMap<&str, &CommandDef> = all_registry_cmds
        .iter()
        .map(|c| (c.id.as_str(), *c))
        .collect();
    let active_view_kind = resolve_active_view_kind(scope_chain, dynamic);
    result.retain(|cmd| {
        let Some(def) = def_by_id.get(cmd.id.as_str()) else {
            // Dynamic / prefix-id rows have no CommandDef — they are not
            // user-authored and carry no view_kinds metadata to honor.
            return true;
        };
        let Some(allowed) = def.view_kinds.as_deref() else {
            // No view_kinds filter on this command — keep unconditionally.
            return true;
        };
        // view_kinds is set: keep iff the active kind is in the allow-list.
        // Empty allow-list is treated the same as a list that does not
        // contain the resolved kind (i.e. always filter out) — an empty
        // allow-list is a YAML authoring mistake, not a wildcard.
        match active_view_kind.as_deref() {
            Some(kind) => allowed.iter().any(|k| k == kind),
            None => false,
        }
    });
}

/// Resolve the innermost `view:{id}` moniker in `scope_chain` to its view
/// kind by consulting `DynamicSources.views`.
///
/// Returns `None` when there is no `view:{id}` moniker in the chain, no
/// `DynamicSources` was provided, or the moniker's id is not registered
/// in `DynamicSources.views`. The `view_kinds` filter treats `None` as a
/// hard no-match for any command that declares the filter, so headless
/// / shell contexts (which never carry a `view:{id}`) cannot surface
/// view-scoped commands.
fn resolve_active_view_kind(
    scope_chain: &[String],
    dynamic: Option<&DynamicSources>,
) -> Option<String> {
    let dyn_src = dynamic?;
    let view_id = scope_chain.iter().find_map(|m| m.strip_prefix("view:"))?;
    dyn_src
        .views
        .iter()
        .find(|v| v.id == view_id)
        .map(|v| v.kind.clone())
}

/// Emit cross-cutting and scoped-registry commands for each moniker in the
/// scope chain.
///
/// Walks each moniker in `scope_chain` in order (innermost first), skipping
/// `field:*` monikers. For each entity moniker, runs two passes in order:
///
///   1. `emit_cross_cutting_commands` — registry commands whose primary param
///      is `from: target` (e.g. `ui.inspect`, `entity.delete`, `entity.archive`).
///   2. `emit_scoped_registry_commands` — registry commands with a `scope:`
///      pin that matches this entity type.
///
/// This ensures commands appear in scope order (attachment before task before
/// global) — the documented ordering at the top of this module.
#[allow(clippy::too_many_arguments)]
fn emit_scoped_commands(
    scope_chain: &[String],
    all_registry_cmds: &[&CommandDef],
    command_impls: &HashMap<String, Arc<dyn Command>>,
    fields: Option<&FieldsContext>,
    ui_state: &Arc<UIState>,
    clipboard_type: Option<&str>,
    seen: &mut HashSet<SeenKey>,
    result: &mut Vec<ResolvedCommand>,
) {
    for moniker in scope_chain {
        let Some((entity_type, entity_id)) = moniker.split_once(':') else {
            continue;
        };
        // Field monikers ("field:task:abc.title") are not entities — skip them.
        // The frontend prefixes field-level FocusScopes with "field:" so they
        // don't masquerade as entity monikers in the scope chain.
        if entity_type == "field" {
            continue;
        }
        let entity_moniker = format!("{entity_type}:{entity_id}");

        // Cross-cutting commands are gated on the entity type being a real
        // declared entity (`fields.get_entity(entity_type).is_some()`). This
        // prevents synthetic monikers like `"foo:bar"` from sprouting
        // `entity.delete`/`entity.archive`/`ui.inspect` against an unknown
        // entity type.
        let is_known_entity = fields
            .map(|f| f.get_entity(entity_type).is_some())
            .unwrap_or(false);
        if is_known_entity {
            emit_cross_cutting_commands(
                all_registry_cmds,
                entity_type,
                &entity_moniker,
                moniker,
                scope_chain,
                command_impls,
                ui_state,
                clipboard_type,
                seen,
                result,
            );
        }
        emit_scoped_registry_commands(
            all_registry_cmds,
            entity_type,
            scope_chain,
            command_impls,
            ui_state,
            clipboard_type,
            seen,
            result,
        );
    }
}

/// Emit registry commands whose primary param is `from: target` for the
/// current entity moniker.
///
/// A "cross-cutting" command is one whose first param's `from` field is
/// `ParamSource::Target` — by construction it operates on whatever entity the
/// context menu fired over (`ui.inspect`, `entity.delete`, `entity.archive`,
/// `entity.unarchive`, …). This pass surfaces each such command exactly once
/// per entity moniker without requiring per-type opt-in YAML.
///
/// Filtering rules, in order:
///
/// 1. The command must be declared with at least one param whose first entry
///    is `ParamSource::Target` (the "primary param is target" signal).
/// 2. If the command declares a `scope:` pin, it must mention either the bare
///    entity type or `entity:{type}` for the current moniker.
/// 3. If the target param declares `entity_type: <type>`, only emit when the
///    moniker's type matches.
/// 4. The command must be `visible: true`.
/// 5. The Rust `Command::available()` impl is the final opt-out (e.g. an
///    archive impl can reject attachments by returning `false`). Commands
///    that fail availability are still emitted with `available: false` —
///    `commands_for_scope` filters them out at the end. This matches the
///    behaviour of the scoped-registry pass.
///
/// Dedup via the shared `(id, target)` seen-set in `push_dedup` ensures a
/// command never double-emits for the same target moniker, even when the
/// scope chain repeats a type or other emit_* helpers cover the same id.
#[allow(clippy::too_many_arguments)]
fn emit_cross_cutting_commands(
    all_registry_cmds: &[&CommandDef],
    entity_type: &str,
    entity_moniker: &str,
    moniker: &str,
    scope_chain: &[String],
    command_impls: &HashMap<String, Arc<dyn Command>>,
    ui_state: &Arc<UIState>,
    clipboard_type: Option<&str>,
    seen: &mut HashSet<SeenKey>,
    result: &mut Vec<ResolvedCommand>,
) {
    let scope_prefixed = format!("entity:{entity_type}");
    // Collect matches into a local vec first so we can sort by
    // (context_menu_group, context_menu_order, id) before emitting. Pushing
    // straight into `result` would inherit the HashMap-iteration order of
    // `all_registry_cmds`, which reseeds per process — the context menu
    // would reshuffle every run.
    let mut pending: Vec<Pending> = Vec::new();
    for cmd_def in all_registry_cmds {
        if let Some(p) = try_match_cross_cutting_command(
            cmd_def,
            entity_type,
            entity_moniker,
            moniker,
            scope_chain,
            &scope_prefixed,
            command_impls,
            ui_state,
            clipboard_type,
            seen,
        ) {
            pending.push(p);
        }
    }
    // Stable sort: primary by context_menu_group (None → u32::MAX sinks
    // uncategorised to the bottom), then by context_menu_order (default 0),
    // then by command id for a deterministic tiebreaker. Without the id
    // tiebreaker, same-group-same-order commands would inherit
    // HashMap-iteration order and reshuffle per process.
    pending.sort_by(|a, b| {
        (a.ctx_group, a.ctx_order, a.cmd.id.as_str()).cmp(&(
            b.ctx_group,
            b.ctx_order,
            b.cmd.id.as_str(),
        ))
    });
    for p in pending {
        push_dedup(seen, result, p.cmd);
    }
}

/// One entry in the sort buffer for `emit_cross_cutting_commands`.
///
/// Carries the sort-key fields alongside the `ResolvedCommand` so the
/// final sort operates on primitive tuples rather than reaching into the
/// resolved struct.
struct Pending {
    cmd: ResolvedCommand,
    ctx_group: u32,
    ctx_order: u32,
}

/// Decide whether one registry command matches the cross-cutting emit pass
/// for a given moniker, returning the sort-buffered `Pending` entry when it
/// does. Encapsulates the three filter rules (param kind, scope pin, target
/// entity-type constraint) plus the seen-set dedup probe.
#[allow(clippy::too_many_arguments)]
fn try_match_cross_cutting_command(
    cmd_def: &CommandDef,
    entity_type: &str,
    entity_moniker: &str,
    moniker: &str,
    scope_chain: &[String],
    scope_prefixed: &str,
    command_impls: &HashMap<String, Arc<dyn Command>>,
    ui_state: &Arc<UIState>,
    clipboard_type: Option<&str>,
    seen: &HashSet<SeenKey>,
) -> Option<Pending> {
    if !cmd_def.visible {
        return None;
    }
    // Rule 1: primary (first) param must be `from: target`.
    let first_param = cmd_def.params.first()?;
    if first_param.from != ParamSource::Target {
        return None;
    }
    // Rule 2: if a scope pin is declared, it must include this type.
    // No pin → cross-cutting on every type (the common case).
    if cmd_def.scope.is_some()
        && !scope_matches(cmd_def.scope.as_deref(), entity_type, scope_prefixed)
    {
        tracing::debug!(
            cmd_id = %cmd_def.id,
            entity_type = %entity_type,
            scope_pin = ?cmd_def.scope,
            "emit_cross_cutting_commands: filtered — scope pin does not match"
        );
        return None;
    }
    // Rule 3: target param can constrain to a specific entity_type.
    if let Some(constrained_type) = first_param.entity_type.as_deref() {
        if constrained_type != entity_type {
            tracing::debug!(
                cmd_id = %cmd_def.id,
                entity_type = %entity_type,
                constrained_type = %constrained_type,
                "emit_cross_cutting_commands: filtered — target param entity_type mismatch"
            );
            return None;
        }
    }

    let tpl = paste_aware_tpl(&cmd_def.id, entity_type, clipboard_type);
    let available = check_available(
        &cmd_def.id,
        scope_chain,
        Some(moniker),
        command_impls,
        ui_state,
    );
    let target = Some(entity_moniker.to_string());
    // Probe the seen-set before push_dedup so dedup skips are observable
    // in the trace — push_dedup itself silently drops duplicates. The third
    // slot (`args`) is `None` here because cross-cutting rows are always
    // emitted without pre-filled args; the fan-out `emit_*` helpers that do
    // use args build their own keys via `push_dedup`.
    let dedup_key: SeenKey = (cmd_def.id.clone(), target.clone(), None);
    if seen.contains(&dedup_key) {
        tracing::debug!(
            cmd_id = %cmd_def.id,
            target = ?target,
            available,
            "emit_cross_cutting_commands: dedup skip — (id, target) already in seen set"
        );
        return None;
    }
    let ctx_group = cmd_def.context_menu_group.unwrap_or(u32::MAX);
    let ctx_order = cmd_def.context_menu_order.unwrap_or(0);
    tracing::debug!(
        cmd_id = %cmd_def.id,
        target = ?target,
        available,
        ctx_group,
        ctx_order,
        outcome = if available { "included" } else { "filtered_unavailable" },
        "emit_cross_cutting_commands: matched command"
    );
    // `group` is the per-entity-type-suffixed context-menu bucket. The
    // frontend renderer inserts a separator whenever it sees a new
    // group string, so distinct `ctx_group` values must produce
    // distinct strings. Suffixing with `entity_type` keeps adjacent
    // entity monikers (e.g. tag then task) from bleeding into one
    // group when context menus are rendered per-moniker.
    let group = format!("{entity_type}:ctx{ctx_group}");
    Some(Pending {
        ctx_group,
        ctx_order,
        cmd: ResolvedCommand {
            id: cmd_def.id.clone(),
            name: resolve_name_template(&cmd_def.name, &tpl),
            menu_name: cmd_def
                .menu_name
                .as_ref()
                .map(|mn| resolve_name_template(mn, &tpl)),
            target,
            group,
            context_menu: cmd_def.context_menu,
            keys: cmd_def.keys.clone(),
            available,
            args: None,
            params: cmd_def.params.clone(),
            tab_button: cmd_def.tab_button.clone(),
        },
    })
}

/// Emit registry commands whose `scope` names the current entity type.
#[allow(clippy::too_many_arguments)]
fn emit_scoped_registry_commands(
    all_registry_cmds: &[&CommandDef],
    entity_type: &str,
    scope_chain: &[String],
    command_impls: &HashMap<String, Arc<dyn Command>>,
    ui_state: &Arc<UIState>,
    clipboard_type: Option<&str>,
    seen: &mut HashSet<SeenKey>,
    result: &mut Vec<ResolvedCommand>,
) {
    let scope_prefixed = format!("entity:{entity_type}");
    for cmd_def in all_registry_cmds {
        if !cmd_def.visible {
            continue;
        }
        if !scope_matches(cmd_def.scope.as_deref(), entity_type, &scope_prefixed) {
            continue;
        }
        let key: SeenKey = (cmd_def.id.clone(), None, None);
        if seen.contains(&key) {
            continue;
        }
        seen.insert(key);

        let tpl = paste_aware_tpl(&cmd_def.id, entity_type, clipboard_type);
        let name = resolve_name_template(&cmd_def.name, &tpl);
        let menu_name = cmd_def
            .menu_name
            .as_ref()
            .map(|mn| resolve_name_template(mn, &tpl));
        let available = check_available(&cmd_def.id, scope_chain, None, command_impls, ui_state);

        result.push(ResolvedCommand {
            id: cmd_def.id.clone(),
            name,
            menu_name,
            target: None,
            group: entity_type.to_string(),
            context_menu: cmd_def.context_menu,
            keys: cmd_def.keys.clone(),
            available,
            args: None,
            params: cmd_def.params.clone(),
            tab_button: cmd_def.tab_button.clone(),
        });
    }
}

/// Emit global (unscoped) registry commands after all scoped commands.
fn emit_global_registry_commands(
    all_registry_cmds: &[&CommandDef],
    scope_chain: &[String],
    command_impls: &HashMap<String, Arc<dyn Command>>,
    ui_state: &Arc<UIState>,
    clipboard_type: Option<&str>,
    seen: &mut HashSet<SeenKey>,
    result: &mut Vec<ResolvedCommand>,
) {
    let innermost_type = scope_chain
        .first()
        .and_then(|m| m.split_once(':').map(|(t, _)| t))
        .unwrap_or("entity");

    for cmd_def in all_registry_cmds {
        if !cmd_def.visible {
            continue;
        }
        let key: SeenKey = (cmd_def.id.clone(), None, None);
        if seen.contains(&key) {
            continue;
        }
        seen.insert(key);

        let tpl = paste_aware_tpl(&cmd_def.id, innermost_type, clipboard_type);
        let name = resolve_name_template(&cmd_def.name, &tpl);
        let menu_name = cmd_def
            .menu_name
            .as_ref()
            .map(|mn| resolve_name_template(mn, &tpl));
        let available = check_available(&cmd_def.id, scope_chain, None, command_impls, ui_state);

        result.push(ResolvedCommand {
            id: cmd_def.id.clone(),
            name,
            menu_name,
            target: None,
            group: "global".to_string(),
            context_menu: cmd_def.context_menu,
            keys: cmd_def.keys.clone(),
            available,
            args: None,
            params: cmd_def.params.clone(),
            tab_button: cmd_def.tab_button.clone(),
        });
    }
}

/// True when `scope` mentions either the bare entity type or `entity:{type}`.
fn scope_matches(scope: Option<&str>, bare: &str, prefixed: &str) -> bool {
    scope.is_some_and(|s| {
        s.split(',').any(|r| {
            let r = r.trim();
            r == bare || r == prefixed
        })
    })
}

/// Build a TemplateParams that substitutes the clipboard type for `entity.paste`.
fn paste_aware_tpl<'a>(
    cmd_id: &str,
    default_type: &'a str,
    clipboard_type: Option<&'a str>,
) -> TemplateParams<'a> {
    TemplateParams {
        entity_type: if cmd_id == "entity.paste" {
            clipboard_type.unwrap_or("entity")
        } else {
            default_type
        },
        ..Default::default()
    }
}

/// Keep only the innermost occurrence of each distinct command row.
///
/// Identity keys on the same `(id, args)` pair the cross-emitter dedup
/// uses: `id` alone is not enough because fan-out dynamic rows (e.g. the
/// per-view `view.set` entries) share an id and only differ by `args`.
/// Collapsing by `id` alone would drop every row but the first, erasing
/// the palette's "Switch to <ViewName>" list.
///
/// When a command like `entity.cut` appears in both tag and task scopes
/// (same id, no args on either), only the innermost (tag) copy is kept.
/// To act on the task, right-click it directly. This prevents confusing
/// menus that show both "Cut Tag" and "Cut Task" when right-clicking a
/// tag pill.
fn dedupe_by_id(result: &mut Vec<ResolvedCommand>) {
    let mut seen: HashSet<(String, Option<String>)> = HashSet::new();
    result.retain(|c| {
        let args_key = c
            .args
            .as_ref()
            .map(|v| serde_json::to_string(v).unwrap_or_default());
        let key = (c.id.clone(), args_key);
        if seen.contains(&key) {
            return false;
        }
        seen.insert(key);
        true
    });
}

/// Collapse a `ResolvedCommand` list for menu-bar consumption.
///
/// The menu bar (e.g. macOS Edit menu) is a global surface — Cut / Copy /
/// Paste must each appear exactly once regardless of how many entity
/// monikers are in scope. The cross-cutting emission pass in
/// `emit_cross_cutting_commands` fires `entity.cut` / `entity.copy` /
/// `entity.paste` once per entity moniker (correct for context menus where
/// each target is a distinct action), so a menu-bar caller must collapse
/// those per-target entries to a single per-id entry.
///
/// Dedup key is the command `id` alone — the menu-bar contract is "one row
/// per command id, ignore target". Per-id menu placement (`CommandDef.menu`)
/// is constant by construction since every emission of a given id resolves
/// to the same `CommandDef`, so keying on `id` is equivalent to keying on
/// `(id, menu.path)` for any well-formed registry.
///
/// The kept entry is the **first** occurrence — `commands_for_scope` emits
/// innermost-first, so this preserves the most-specific target for dispatch.
/// When the user picks Edit → Cut from the menu bar, the dispatcher acts on
/// the innermost entity in the current scope, matching the user's selection.
///
/// This is a no-op when called on a list that already has at most one entry
/// per id (e.g. the output of `commands_for_scope` post-`dedupe_by_id`); it
/// is intended for callers that bypass the inner dedupe and feed the raw
/// per-target stream into a menu-bar renderer.
pub fn dedupe_for_menu_bar(commands: &mut Vec<ResolvedCommand>) {
    let mut seen_ids: HashSet<String> = HashSet::new();
    commands.retain(|c| {
        if seen_ids.contains(&c.id) {
            return false;
        }
        seen_ids.insert(c.id.clone());
        true
    });
}


#[cfg(test)]
mod tests {
    // The `scope_commands` test suite that lived here drove `commands_for_scope`
    // against the YAML-driven `CommandsRegistry` composed from the (now
    // deleted) 12 builtin command YAMLs. Stage 4 of the kanban cut-over
    // retired both — `CommandService` (fed by the 7 builtin command plugins
    // at app startup) is now the sole source of command metadata, so the
    // scope-resolution surface those tests exercised has moved out of this
    // module. The end-to-end coverage now lives in the per-plugin e2e tests
    // under `swissarmyhammer-command-service/tests/integration/builtin_*_commands_e2e.rs`
    // and the new `full_baseline_e2e` integration test.
}
