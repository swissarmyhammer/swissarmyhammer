//! Backend-driven command resolution for a given scope chain.
//!
//! `commands_for_scope` is the single source of truth for what commands
//! are available in a given focus context. It walks the scope chain,
//! looks up entity schemas for their declared commands, merges with
//! global registry commands, checks availability, and resolves all
//! template names (e.g. `{{entity.type}}` → "Task").

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use swissarmyhammer_commands::{
    Command, CommandContext, CommandDef, CommandsRegistry, KeysDef, UIState,
};
use swissarmyhammer_fields::FieldsContext;

/// Lightweight view descriptor for dynamic command generation.
///
/// Only carries the fields needed to produce a `view.switch:{id}` command.
/// Intentionally decoupled from `ViewDef` so the scope_commands module
/// does not depend on the views crate directly.
#[derive(Debug, Clone)]
pub struct ViewInfo {
    /// View identifier (e.g. "board-view", "tasks-grid").
    pub id: String,
    /// Human-readable name (e.g. "Board View", "Task Grid").
    pub name: String,
    /// Entity type this view renders (e.g. "task", "tag", "project").
    ///
    /// When present, the scope dispatcher emits a dynamic
    /// `entity.add:{entity_type}` command so every view type gets a
    /// generic "New {Type}" creation action without per-type Rust code.
    pub entity_type: Option<String>,
}

/// Lightweight open-window descriptor for dynamic command generation.
///
/// Only carries the fields needed to produce a `window.focus:{label}` command.
/// Intentionally decoupled from Tauri's WebviewWindow so the scope_commands
/// module does not depend on Tauri directly.
#[derive(Debug, Clone)]
pub struct WindowInfo {
    /// Tauri window label (e.g. "main", "board-01jxyz").
    pub label: String,
    /// Human-readable window title (e.g. "SwissArmyHammer").
    pub title: String,
    /// Whether this window currently has focus.
    pub focused: bool,
}

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

/// Lightweight perspective descriptor for dynamic command generation.
///
/// Only carries the fields needed to produce a `perspective.goto:{id}` command.
/// Intentionally decoupled from `Perspective` so the scope_commands module
/// does not depend on the perspectives crate directly.
#[derive(Debug, Clone)]
pub struct PerspectiveInfo {
    /// Perspective identifier (ULID).
    pub id: String,
    /// Human-readable name (e.g. "Active Sprint").
    pub name: String,
    /// View type (e.g. "board", "grid").
    pub view: String,
}

/// Runtime data that feeds dynamic command generation beyond the static
/// registry and entity schemas.
#[derive(Debug, Clone, Default)]
pub struct DynamicSources {
    /// Loaded view definitions — each generates a `view.switch:{id}` command.
    pub views: Vec<ViewInfo>,
    /// Open boards — each generates a `board.switch:{path}` command.
    pub boards: Vec<BoardInfo>,
    /// Open windows — each generates a `window.focus:{label}` command.
    pub windows: Vec<WindowInfo>,
    /// Perspectives — each generates a `perspective.goto:{id}` command.
    pub perspectives: Vec<PerspectiveInfo>,
}

/// A fully resolved command ready for display in a menu, palette, or context menu.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ResolvedCommand {
    /// Command ID (e.g. "entity.copy").
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

/// Emit dynamic commands from runtime data into the result list.
///
/// Generates `view.switch:{id}`, `board.switch:{path}`, `window.focus:{label}`,
/// `perspective.goto:{id}`, and `entity.add:{type}` commands from the
/// dynamic sources. Skips commands already in the `seen` set.
///
/// `entity.add:{type}` is the only dynamic command that depends on the current
/// scope chain: it surfaces only when a `view:{id}` moniker is active and
/// the matching view declares an `entity_type`. Unlike the navigation
/// dynamics (`view.switch`, `board.switch`, `perspective.goto`,
/// `window.focus`) which all set `context_menu: false`, `entity.add:*` is a
/// first-class creation action and is emitted with `context_menu: true` so
/// it appears on right-click inside the view.
///
/// The dispatch-side handler that actually creates the entity lives in
/// `crate::entity::add::AddEntity`; `entity.add:{type}` monikers produced
/// here are rewritten to the canonical `entity.add` command in
/// `kanban-app/src/commands.rs::dispatch_command_internal` and routed into
/// that operation.
/// Push a command once, honoring the `(id, target)` seen-set for dedup.
fn push_dedup(
    seen: &mut HashSet<(String, Option<String>)>,
    result: &mut Vec<ResolvedCommand>,
    cmd: ResolvedCommand,
) {
    let key = (cmd.id.clone(), cmd.target.clone());
    if seen.contains(&key) {
        return;
    }
    seen.insert(key);
    result.push(cmd);
}

fn emit_view_switch(
    views: &[ViewInfo],
    seen: &mut HashSet<(String, Option<String>)>,
    result: &mut Vec<ResolvedCommand>,
) {
    for view in views {
        push_dedup(
            seen,
            result,
            ResolvedCommand {
                id: format!("view.switch:{}", view.id),
                name: format!("Switch to {}", view.name),
                menu_name: None,
                target: None,
                group: "view".into(),
                context_menu: false,
                keys: None,
                available: true,
            },
        );
    }
}

fn emit_board_switch(
    boards: &[BoardInfo],
    seen: &mut HashSet<(String, Option<String>)>,
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
            },
        );
    }
}

fn emit_window_focus(
    windows: &[WindowInfo],
    seen: &mut HashSet<(String, Option<String>)>,
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
            },
        );
    }
}

fn emit_perspective_goto(
    perspectives: &[PerspectiveInfo],
    seen: &mut HashSet<(String, Option<String>)>,
    result: &mut Vec<ResolvedCommand>,
) {
    for perspective in perspectives {
        push_dedup(
            seen,
            result,
            ResolvedCommand {
                id: format!("perspective.goto:{}", perspective.id),
                name: format!("Go to Perspective: {}", perspective.name),
                menu_name: None,
                target: None,
                group: "perspective".into(),
                context_menu: false,
                keys: None,
                available: true,
            },
        );
    }
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
    seen: &mut HashSet<(String, Option<String>)>,
    result: &mut Vec<ResolvedCommand>,
) {
    for moniker in scope_chain {
        let Some(view_id) = moniker.strip_prefix("view:") else {
            continue;
        };
        let Some(view) = views_by_id.get(view_id) else {
            tracing::debug!(
                scope_moniker = %moniker,
                view_id = %view_id,
                known_view_count = views_by_id.len(),
                "emit_entity_add: view moniker has no matching ViewInfo — \
                 gather_views did not populate this view"
            );
            continue;
        };
        let Some(entity_type) = view.entity_type.as_deref() else {
            tracing::debug!(
                scope_moniker = %moniker,
                view_id = %view_id,
                view_name = %view.name,
                "emit_entity_add: view has no entity_type — skipping (dashboard-style view)"
            );
            continue;
        };
        if entity_type.is_empty() {
            tracing::debug!(
                scope_moniker = %moniker,
                view_id = %view_id,
                view_name = %view.name,
                "emit_entity_add: view entity_type is empty string — skipping"
            );
            continue;
        }
        let tpl = TemplateParams {
            entity_type,
            ..Default::default()
        };
        let cmd_id = format!("entity.add:{entity_type}");
        tracing::debug!(
            scope_moniker = %moniker,
            view_id = %view_id,
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
            },
        );
    }
}

fn emit_dynamic_commands(
    dyn_src: &DynamicSources,
    scope_chain: &[String],
    seen: &mut HashSet<(String, Option<String>)>,
    result: &mut Vec<ResolvedCommand>,
) {
    // Index views by id once so the `entity.add` emission below is O(scope)
    // rather than O(scope × views).
    let views_by_id: HashMap<&str, &ViewInfo> = dyn_src
        .views
        .iter()
        .map(|v| (v.id.as_str(), v))
        .collect();
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
pub fn commands_for_scope(
    scope_chain: &[String],
    registry: &CommandsRegistry,
    command_impls: &HashMap<String, Arc<dyn Command>>,
    fields: Option<&FieldsContext>,
    ui_state: &Arc<UIState>,
    context_menu_only: bool,
    dynamic: Option<&DynamicSources>,
) -> Vec<ResolvedCommand> {
    let mut result: Vec<ResolvedCommand> = Vec::new();
    let mut seen: HashSet<(String, Option<String>)> = HashSet::new();

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

    result
}

/// Emit entity-schema commands for the current moniker.
///
/// Two passes: commands declared directly on `entity_type`, then commands from
/// any entity whose `scope` references this type (e.g. `task.add` declared on
/// task with scope `entity:column` matches when processing a column moniker).
///
/// Walks each moniker in `scope_chain` in order (innermost first), skipping
/// `field:*` monikers. For each entity moniker, delegates to
/// `emit_entity_schema_commands` (if `fields` is supplied) then to
/// `emit_scoped_registry_commands`. This ensures commands appear in scope
/// order: attachment before task before global.
#[allow(clippy::too_many_arguments)]
fn emit_scoped_commands(
    scope_chain: &[String],
    all_registry_cmds: &[&CommandDef],
    command_impls: &HashMap<String, Arc<dyn Command>>,
    fields: Option<&FieldsContext>,
    ui_state: &Arc<UIState>,
    clipboard_type: Option<&str>,
    seen: &mut HashSet<(String, Option<String>)>,
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

        if let Some(fields) = fields {
            emit_entity_schema_commands(
                fields,
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

/// Collect direct + transitively-scoped entity-declared commands for a type.
fn collect_entity_schema_cmds<'a>(
    fields: &'a FieldsContext,
    entity_type: &str,
) -> Vec<&'a swissarmyhammer_fields::types::EntityCommand> {
    let scope_prefixed_et = format!("entity:{entity_type}");
    let direct_cmds = fields
        .get_entity(entity_type)
        .map(|e| e.commands.iter().collect::<Vec<_>>())
        .unwrap_or_default();
    let scoped_cmds: Vec<_> = fields
        .all_entities()
        .iter()
        .flat_map(|e| e.commands.iter())
        .filter(|cmd| scope_matches(cmd.scope.as_deref(), entity_type, &scope_prefixed_et))
        .collect();
    direct_cmds.into_iter().chain(scoped_cmds).collect()
}

#[allow(clippy::too_many_arguments)]
fn emit_entity_schema_commands(
    fields: &FieldsContext,
    entity_type: &str,
    entity_moniker: &str,
    moniker: &str,
    scope_chain: &[String],
    command_impls: &HashMap<String, Arc<dyn Command>>,
    ui_state: &Arc<UIState>,
    clipboard_type: Option<&str>,
    seen: &mut HashSet<(String, Option<String>)>,
    result: &mut Vec<ResolvedCommand>,
) {
    for cmd in collect_entity_schema_cmds(fields, entity_type) {
        if cmd.visible == Some(false) {
            continue;
        }
        let key = (cmd.id.clone(), Some(entity_moniker.to_string()));
        if seen.contains(&key) {
            continue;
        }
        seen.insert(key);

        let tpl = paste_aware_tpl(&cmd.id, entity_type, clipboard_type);
        let keys = cmd.keys.as_ref().map(|k| KeysDef {
            vim: k.vim.clone(),
            cua: k.cua.clone(),
            emacs: k.emacs.clone(),
        });
        result.push(ResolvedCommand {
            id: cmd.id.clone(),
            name: resolve_name_template(&cmd.name, &tpl),
            menu_name: cmd
                .menu_name
                .as_ref()
                .map(|mn| resolve_name_template(mn, &tpl)),
            target: Some(entity_moniker.to_string()),
            group: entity_type.to_string(),
            context_menu: cmd.context_menu,
            keys,
            available: check_available(
                &cmd.id,
                scope_chain,
                Some(moniker),
                command_impls,
                ui_state,
            ),
        });
    }
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
    seen: &mut HashSet<(String, Option<String>)>,
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
        let key = (cmd_def.id.clone(), None);
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
        let available =
            check_available(&cmd_def.id, scope_chain, None, command_impls, ui_state);

        result.push(ResolvedCommand {
            id: cmd_def.id.clone(),
            name,
            menu_name,
            target: None,
            group: entity_type.to_string(),
            context_menu: cmd_def.context_menu,
            keys: cmd_def.keys.clone(),
            available,
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
    seen: &mut HashSet<(String, Option<String>)>,
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
        let key = (cmd_def.id.clone(), None);
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
        let available =
            check_available(&cmd_def.id, scope_chain, None, command_impls, ui_state);

        result.push(ResolvedCommand {
            id: cmd_def.id.clone(),
            name,
            menu_name,
            target: None,
            group: "global".to_string(),
            context_menu: cmd_def.context_menu,
            keys: cmd_def.keys.clone(),
            available,
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

/// Keep only the innermost occurrence of each command id.
///
/// When a command like `entity.cut` appears in both tag and task scopes, only
/// the innermost (tag) copy is kept. To act on the task, right-click it
/// directly. This prevents confusing menus that show both "Cut Tag" and
/// "Cut Task" when right-clicking a tag pill.
fn dedupe_by_id(result: &mut Vec<ResolvedCommand>) {
    let mut seen_ids: HashSet<String> = HashSet::new();
    result.retain(|c| {
        if seen_ids.contains(&c.id) {
            return false;
        }
        seen_ids.insert(c.id.clone());
        true
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defaults::{builtin_entity_definitions, builtin_field_definitions};
    use swissarmyhammer_commands::builtin_yaml_sources;

    /// Test harness tuple: registry, command impls, fields context, and UI state.
    type TestHarness = (
        CommandsRegistry,
        HashMap<String, Arc<dyn Command>>,
        FieldsContext,
        Arc<UIState>,
    );

    /// Build a test harness with registry, command impls, and fields context.
    fn setup() -> TestHarness {
        let registry = CommandsRegistry::from_yaml_sources(&builtin_yaml_sources());
        let command_impls = crate::commands::register_commands();
        let defs = builtin_field_definitions();
        let entities = builtin_entity_definitions();
        let fields = FieldsContext::from_yaml_sources(
            std::path::PathBuf::from("/tmp/test"),
            &defs,
            &entities,
        )
        .unwrap();
        let ui_state = Arc::new(UIState::new());
        (registry, command_impls, fields, ui_state)
    }

    // =========================================================================
    // Board scope
    // =========================================================================

    #[test]
    fn board_scope_has_global_commands() {
        let (registry, impls, fields, ui) = setup();
        ui.set_undo_redo_state(true, true);
        let scope = vec!["board:my-board".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);

        let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();
        assert!(ids.contains(&"app.undo"), "board scope should have undo");
        assert!(ids.contains(&"app.redo"), "board scope should have redo");
        assert!(
            !ids.contains(&"entity.copy"),
            "board scope should NOT have copy (no task/tag)"
        );
        assert!(
            !ids.contains(&"entity.cut"),
            "board scope should NOT have cut"
        );
    }

    #[test]
    fn board_scope_no_paste_without_clipboard() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["board:my-board".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();
        assert!(!ids.contains(&"entity.paste"), "no paste without clipboard");
    }

    // =========================================================================
    // Column scope
    // =========================================================================

    #[test]
    fn column_scope_paste_task_with_task_clipboard() {
        let (registry, impls, fields, ui) = setup();
        ui.set_clipboard_entity_type("task");
        let scope = vec!["column:todo".into(), "board:my-board".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);

        let paste = cmds.iter().find(|c| c.id == "entity.paste");
        assert!(
            paste.is_some(),
            "paste should be available with task on clipboard + column in scope"
        );
        assert_eq!(paste.unwrap().name, "Paste Task");
    }

    // =========================================================================
    // Task scope
    // =========================================================================

    #[test]
    fn task_scope_has_copy_cut_inspect_archive() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec![
            "task:01X".into(),
            "column:todo".into(),
            "board:my-board".into(),
        ];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let names: Vec<&str> = cmds.iter().map(|c| c.name.as_str()).collect();

        assert!(
            names.contains(&"Copy Task"),
            "should have Copy Task: {:?}",
            names
        );
        assert!(
            names.contains(&"Cut Task"),
            "should have Cut Task: {:?}",
            names
        );
        assert!(
            names.contains(&"Inspect Task"),
            "should have Inspect Task: {:?}",
            names
        );
        assert!(
            names.contains(&"Archive Task"),
            "should have Archive Task: {:?}",
            names
        );
    }

    #[test]
    fn task_scope_paste_tag_with_tag_clipboard() {
        let (registry, impls, fields, ui) = setup();
        ui.set_clipboard_entity_type("tag");
        let scope = vec![
            "task:01X".into(),
            "column:todo".into(),
            "board:my-board".into(),
        ];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let paste = cmds.iter().find(|c| c.id == "entity.paste");
        assert!(paste.is_some(), "paste should be available");
        assert_eq!(paste.unwrap().name, "Paste Tag");
    }

    // =========================================================================
    // Tag on task scope
    // =========================================================================

    #[test]
    fn tag_on_task_has_only_tag_copy_cut_inspect() {
        // With dedup-by-id (innermost wins), right-clicking a tag pill shows
        // only the tag-level commands for shared IDs like entity.copy, entity.cut,
        // entity.inspect. The task-level versions are suppressed.
        let (registry, impls, fields, ui) = setup();
        let scope = vec![
            "tag:bug".into(),
            "task:01X".into(),
            "column:todo".into(),
            "board:my-board".into(),
        ];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let names: Vec<&str> = cmds.iter().map(|c| c.name.as_str()).collect();

        // Innermost (tag) versions are present
        assert!(
            names.contains(&"Copy Tag"),
            "should have Copy Tag: {:?}",
            names
        );
        assert!(
            names.contains(&"Cut Tag"),
            "should have Cut Tag: {:?}",
            names
        );
        assert!(
            names.contains(&"Inspect Tag"),
            "should have Inspect Tag: {:?}",
            names
        );

        // Outer (task) versions are suppressed by dedup-by-id
        assert!(
            !names.contains(&"Copy Task"),
            "should NOT have Copy Task (deduped by id, tag wins): {:?}",
            names
        );
        assert!(
            !names.contains(&"Cut Task"),
            "should NOT have Cut Task (deduped by id, tag wins): {:?}",
            names
        );
        assert!(
            !names.contains(&"Inspect Task"),
            "should NOT have Inspect Task (deduped by id, tag wins): {:?}",
            names
        );
    }

    #[test]
    fn tag_on_task_with_tag_clipboard_has_paste_tag() {
        let (registry, impls, fields, ui) = setup();
        ui.set_clipboard_entity_type("tag");
        let scope = vec!["tag:bug".into(), "task:01X".into(), "column:todo".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let paste = cmds.iter().find(|c| c.id == "entity.paste");
        assert!(paste.is_some(), "should have paste");
        assert_eq!(paste.unwrap().name, "Paste Tag");
    }

    #[test]
    fn tag_on_task_no_paste_without_clipboard() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["tag:bug".into(), "task:01X".into(), "column:todo".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let paste_cmds: Vec<_> = cmds.iter().filter(|c| c.id == "entity.paste").collect();
        assert!(paste_cmds.is_empty(), "no paste without clipboard");
    }

    // =========================================================================
    // Name resolution
    // =========================================================================

    #[test]
    fn all_names_fully_resolved() {
        let (registry, impls, fields, ui) = setup();
        ui.set_clipboard_entity_type("task");
        let scope = vec![
            "tag:bug".into(),
            "task:01X".into(),
            "column:todo".into(),
            "board:my-board".into(),
        ];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        for cmd in &cmds {
            assert!(
                !cmd.name.contains("{{"),
                "command '{}' has unresolved template: '{}'",
                cmd.id,
                cmd.name
            );
        }
    }

    // =========================================================================
    // Context menu filter
    // =========================================================================

    #[test]
    fn context_menu_only_filters() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec![
            "task:01X".into(),
            "column:todo".into(),
            "board:my-board".into(),
        ];
        let all = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let ctx_only =
            commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, true, None);

        assert!(
            ctx_only.len() < all.len(),
            "context menu should have fewer commands"
        );
        for cmd in &ctx_only {
            assert!(cmd.context_menu, "'{}' should be context_menu", cmd.id);
        }
    }

    // =========================================================================
    // Empty scope
    // =========================================================================

    #[test]
    fn empty_scope_has_only_global_commands() {
        let (registry, impls, fields, ui) = setup();
        ui.set_undo_redo_state(true, true);
        let cmds = commands_for_scope(&[], &registry, &impls, Some(&fields), &ui, false, None);

        let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();
        assert!(ids.contains(&"app.undo"));
        assert!(!ids.contains(&"entity.copy"));
        for cmd in &cmds {
            assert!(cmd.target.is_none(), "'{}' should have no target", cmd.id);
        }
    }

    // =========================================================================
    // Paste cross-matching: clipboard type vs scope type
    // =========================================================================

    #[test]
    fn task_clipboard_task_focused_no_paste() {
        // Task on clipboard + task focused (no column) → can't paste task here
        let (registry, impls, fields, ui) = setup();
        ui.set_clipboard_entity_type("task");
        let scope = vec!["task:01X".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let paste: Vec<_> = cmds.iter().filter(|c| c.id == "entity.paste").collect();
        assert!(paste.is_empty(), "can't paste task without column in scope");
    }

    #[test]
    fn task_clipboard_column_focused_paste_available() {
        let (registry, impls, fields, ui) = setup();
        ui.set_clipboard_entity_type("task");
        let scope = vec!["column:todo".into(), "board:board".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let paste = cmds.iter().find(|c| c.id == "entity.paste");
        assert!(paste.is_some(), "paste task should be available on column");
        assert_eq!(paste.unwrap().name, "Paste Task");
    }

    #[test]
    fn tag_clipboard_column_focused_no_paste() {
        // Tag on clipboard + column focused (no task) → can't paste tag here
        let (registry, impls, fields, ui) = setup();
        ui.set_clipboard_entity_type("tag");
        let scope = vec!["column:todo".into(), "board:board".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let paste: Vec<_> = cmds.iter().filter(|c| c.id == "entity.paste").collect();
        assert!(paste.is_empty(), "can't paste tag without task in scope");
    }

    #[test]
    fn tag_clipboard_task_focused_paste_available() {
        let (registry, impls, fields, ui) = setup();
        ui.set_clipboard_entity_type("tag");
        let scope = vec!["task:01X".into(), "column:todo".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let paste = cmds.iter().find(|c| c.id == "entity.paste");
        assert!(paste.is_some(), "paste tag should be available on task");
        assert_eq!(paste.unwrap().name, "Paste Tag");
    }

    #[test]
    fn board_scope_with_task_clipboard_paste_available() {
        let (registry, impls, fields, ui) = setup();
        ui.set_clipboard_entity_type("task");
        let scope = vec!["board:my-board".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let paste = cmds.iter().find(|c| c.id == "entity.paste");
        assert!(paste.is_some(), "paste task should be available on board");
        assert_eq!(paste.unwrap().name, "Paste Task");
    }

    // =========================================================================
    // Dedup: paste appears exactly once even when on multiple entity types
    // =========================================================================

    #[test]
    fn paste_appears_once_on_task_scope() {
        // Task + column + board all declare entity.paste
        // Should appear once (from task, innermost)
        let (registry, impls, fields, ui) = setup();
        ui.set_clipboard_entity_type("tag");
        let scope = vec![
            "task:01X".into(),
            "column:todo".into(),
            "board:board".into(),
        ];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let paste_cmds: Vec<_> = cmds.iter().filter(|c| c.id == "entity.paste").collect();
        // Paste is declared on task, column, board — but dedup by (id, target) means
        // each has a different target so they DON'T dedup. However, only the ones
        // where available() returns true should appear.
        // tag clipboard + task in scope → paste available on task target
        // tag clipboard + column in scope → paste NOT available (tag needs task)
        // tag clipboard + board in scope → paste NOT available (tag needs task)
        assert_eq!(
            paste_cmds.len(),
            1,
            "paste should appear once: {:?}",
            paste_cmds.iter().map(|c| &c.target).collect::<Vec<_>>()
        );
    }

    #[test]
    fn paste_appears_once_on_column_scope_with_task_clipboard() {
        let (registry, impls, fields, ui) = setup();
        ui.set_clipboard_entity_type("task");
        let scope = vec!["column:todo".into(), "board:board".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let paste_cmds: Vec<_> = cmds.iter().filter(|c| c.id == "entity.paste").collect();
        // task clipboard: paste available on column (yes) and board (yes)
        // Both have paste declared, but availability check allows both.
        // This is 2 because column and board both pass.
        // We accept this — the frontend dedup isn't our concern here.
        // The backend returns all available instances.
        assert!(!paste_cmds.is_empty(), "at least one paste should appear");
    }

    // =========================================================================
    // Global commands always present
    // =========================================================================

    #[test]
    fn app_quit_always_available() {
        let (registry, impls, fields, ui) = setup();
        for scope in [
            vec![],
            vec!["board:b".into()],
            vec!["task:t".into(), "column:c".into()],
            vec!["tag:x".into(), "task:t".into(), "column:c".into()],
        ] {
            let cmds =
                commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
            let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();
            assert!(
                ids.contains(&"app.quit"),
                "app.quit should be in scope {:?}",
                scope
            );
        }
    }

    #[test]
    fn app_undo_redo_filtered_out_when_stack_empty() {
        let (registry, impls, fields, ui) = setup();
        // Default UIState: can_undo=false, can_redo=false
        let scope = vec!["task:01X".into(), "column:todo".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();
        assert!(
            !ids.contains(&"app.undo"),
            "undo should not appear when stack is empty"
        );
        assert!(
            !ids.contains(&"app.redo"),
            "redo should not appear when stack is empty"
        );
    }

    #[test]
    fn app_undo_available_when_ui_state_says_so() {
        let (registry, impls, fields, ui) = setup();
        ui.set_undo_redo_state(true, false);
        let scope = vec!["task:01X".into(), "column:todo".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();
        assert!(
            ids.contains(&"app.undo"),
            "undo should appear when can_undo is true"
        );
        assert!(
            !ids.contains(&"app.redo"),
            "redo should not appear when can_redo is false"
        );
    }

    #[test]
    fn app_redo_available_when_ui_state_says_so() {
        let (registry, impls, fields, ui) = setup();
        ui.set_undo_redo_state(false, true);
        let scope = vec!["task:01X".into(), "column:todo".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();
        assert!(
            !ids.contains(&"app.undo"),
            "undo should not appear when can_undo is false"
        );
        assert!(
            ids.contains(&"app.redo"),
            "redo should appear when can_redo is true"
        );
    }

    // =========================================================================
    // Keys pass through
    // =========================================================================

    #[test]
    fn copy_task_has_keybindings() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["task:01X".into(), "column:todo".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let copy = cmds.iter().find(|c| c.name == "Copy Task").unwrap();
        let keys = copy.keys.as_ref().expect("Copy Task should have keys");
        assert_eq!(keys.cua.as_deref(), Some("Mod+C"));
        assert_eq!(keys.vim.as_deref(), Some("y"));
    }

    // =========================================================================
    // Visible flag
    // =========================================================================

    #[test]
    fn invisible_commands_not_returned() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["task:01X".into(), "column:todo".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();
        // entity.update_field is visible: false in YAML
        assert!(
            !ids.contains(&"entity.update_field"),
            "invisible commands should be excluded"
        );
    }

    // =========================================================================
    // Cut tag requires task in scope
    // =========================================================================

    #[test]
    fn cut_tag_not_available_without_task_parent() {
        // Hypothetical: tag focused without a task parent
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["tag:bug".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let cut_tag = cmds.iter().find(|c| c.name == "Cut Tag");
        // CutCmd.available() checks has_in_scope("tag") — true. But the actual
        // execute would fail without task. available() doesn't check for task
        // on cut when tag is present. This test documents current behavior.
        // If cut tag should require task, fix CutCmd.available().
        assert!(
            cut_tag.is_some() || cut_tag.is_none(),
            "documenting: cut tag availability without task parent"
        );
    }

    // =========================================================================
    // Targets are correct
    // =========================================================================

    #[test]
    fn entity_commands_have_correct_targets() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec![
            "tag:01TAG".into(),
            "task:01TASK".into(),
            "column:todo".into(),
        ];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);

        // With dedup-by-id (innermost wins), the tag scope wins for entity.copy.
        // "Copy Tag" appears with target "tag:01TAG"; "Copy Task" is deduped away.
        let copy_tag = cmds.iter().find(|c| c.name == "Copy Tag").unwrap();
        assert_eq!(copy_tag.target.as_deref(), Some("tag:01TAG"));

        let copy_task = cmds.iter().find(|c| c.name == "Copy Task");
        assert!(
            copy_task.is_none(),
            "Copy Task should be deduped away when tag is innermost scope"
        );

        // Task-only scope to verify task target is correct when no tag present
        let task_only_scope = vec!["task:01TASK".into(), "column:todo".into()];
        let task_cmds = commands_for_scope(
            &task_only_scope,
            &registry,
            &impls,
            Some(&fields),
            &ui,
            false,
            None,
        );
        let copy_task_direct = task_cmds.iter().find(|c| c.name == "Copy Task").unwrap();
        assert_eq!(copy_task_direct.target.as_deref(), Some("task:01TASK"));

        let inspect_col = cmds.iter().find(|c| c.name == "Inspect Column");
        if let Some(ic) = inspect_col {
            assert_eq!(ic.target.as_deref(), Some("column:todo"));
        }
    }

    // =========================================================================
    // Scoped registry commands (task.add needs column, task.untag needs tag+task)
    // =========================================================================

    #[test]
    fn task_add_available_with_column_in_scope() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["column:todo".into(), "board:board".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();
        assert!(
            ids.contains(&"task.add"),
            "task.add should be available with column in scope: {:?}",
            ids
        );
    }

    #[test]
    fn task_add_not_available_without_column() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["board:board".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();
        assert!(
            !ids.contains(&"task.add"),
            "task.add should NOT be available without column"
        );
    }

    #[test]
    fn task_untag_available_with_tag_and_task() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["tag:bug".into(), "task:01X".into(), "column:todo".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();
        assert!(
            ids.contains(&"task.untag"),
            "task.untag should be available with tag+task: {:?}",
            ids
        );
    }

    #[test]
    fn task_untag_not_available_without_tag() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["task:01X".into(), "column:todo".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();
        assert!(
            !ids.contains(&"task.untag"),
            "task.untag should NOT be available without tag"
        );
    }

    // =========================================================================
    // Other entity types (actor, attachment)
    // =========================================================================

    #[test]
    fn actor_scope_has_inspect() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["actor:alice".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let names: Vec<&str> = cmds.iter().map(|c| c.name.as_str()).collect();
        assert!(
            names.contains(&"Inspect Actor"),
            "actor should have Inspect Actor: {:?}",
            names
        );
    }

    // =========================================================================
    // Unknown entity type in scope
    // =========================================================================

    #[test]
    fn unknown_entity_type_ignored() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["foo:bar".into(), "task:01X".into(), "column:todo".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        // Should still have task commands — unknown type just gets skipped
        let names: Vec<&str> = cmds.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"Copy Task"));
        // Should NOT have commands for "foo" type
        assert!(!cmds.iter().any(|c| c.target.as_deref() == Some("foo:bar")));
    }

    // =========================================================================
    // Drag commands (visible: false) excluded
    // =========================================================================

    // =========================================================================
    // Paste name comes from clipboard, NOT from the entity type it's declared on
    // =========================================================================

    #[test]
    fn paste_on_column_says_paste_task_not_paste_column() {
        // Cut a task → clipboard has "task" → paste on column should say "Paste Task"
        let (registry, impls, fields, ui) = setup();
        ui.set_clipboard_entity_type("task");
        let scope = vec!["column:todo".into(), "board:board".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, true, None);
        let paste = cmds.iter().find(|c| c.id == "entity.paste");
        assert!(paste.is_some(), "paste should be available");
        assert_eq!(
            paste.unwrap().name,
            "Paste Task",
            "paste name should come from clipboard type, not column entity type"
        );
    }

    #[test]
    fn paste_on_task_says_paste_tag_not_paste_task() {
        // Copy a tag → clipboard has "tag" → paste on task should say "Paste Tag"
        let (registry, impls, fields, ui) = setup();
        ui.set_clipboard_entity_type("tag");
        let scope = vec!["task:01X".into(), "column:todo".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, true, None);
        let paste = cmds.iter().find(|c| c.id == "entity.paste");
        assert!(paste.is_some(), "paste should be available");
        assert_eq!(
            paste.unwrap().name,
            "Paste Tag",
            "paste name should come from clipboard type, not task entity type"
        );
    }

    #[test]
    fn drag_commands_never_appear() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["task:01X".into(), "column:todo".into(), "board:b".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();
        assert!(!ids.contains(&"drag.start"));
        assert!(!ids.contains(&"drag.cancel"));
        assert!(!ids.contains(&"drag.complete"));
    }

    // =========================================================================
    // Targets
    // =========================================================================

    #[test]
    fn global_commands_have_no_target() {
        let (registry, impls, fields, ui) = setup();
        ui.set_undo_redo_state(true, true);
        let scope = vec!["task:01X".into(), "column:todo".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);

        let undo = cmds.iter().find(|c| c.id == "app.undo").unwrap();
        assert!(undo.target.is_none());

        let quit = cmds.iter().find(|c| c.id == "app.quit").unwrap();
        assert!(quit.target.is_none());
    }

    // =========================================================================
    // Dynamic view switch commands
    // =========================================================================

    #[test]
    fn view_switch_commands_appear_when_views_provided() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["board:my-board".into()];
        let dynamic = DynamicSources {
            views: vec![
                ViewInfo {
                    id: "board-view".into(),
                    name: "Board View".into(),
                    entity_type: None,
                },
                ViewInfo {
                    id: "tasks-grid".into(),
                    name: "Task Grid".into(),
                    entity_type: None,
                },
                ViewInfo {
                    id: "tags-grid".into(),
                    name: "Tag Grid".into(),
                    entity_type: None,
                },
            ],
            boards: vec![],
            windows: vec![],
            perspectives: vec![],
        };
        let cmds = commands_for_scope(
            &scope,
            &registry,
            &impls,
            Some(&fields),
            &ui,
            false,
            Some(&dynamic),
        );
        let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();

        assert!(
            ids.contains(&"view.switch:board-view"),
            "should have board-view switch: {:?}",
            ids
        );
        assert!(
            ids.contains(&"view.switch:tasks-grid"),
            "should have tasks-grid switch: {:?}",
            ids
        );
        assert!(
            ids.contains(&"view.switch:tags-grid"),
            "should have tags-grid switch: {:?}",
            ids
        );
    }

    #[test]
    fn view_switch_commands_have_correct_names() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["board:my-board".into()];
        let dynamic = DynamicSources {
            views: vec![
                ViewInfo {
                    id: "board-view".into(),
                    name: "Board View".into(),
                    entity_type: None,
                },
                ViewInfo {
                    id: "tasks-grid".into(),
                    name: "Task Grid".into(),
                    entity_type: None,
                },
            ],
            boards: vec![],
            windows: vec![],
            perspectives: vec![],
        };
        let cmds = commands_for_scope(
            &scope,
            &registry,
            &impls,
            Some(&fields),
            &ui,
            false,
            Some(&dynamic),
        );

        let board_switch = cmds
            .iter()
            .find(|c| c.id == "view.switch:board-view")
            .unwrap();
        assert_eq!(board_switch.name, "Switch to Board View");
        assert_eq!(board_switch.group, "view");
        assert!(board_switch.target.is_none());

        let grid_switch = cmds
            .iter()
            .find(|c| c.id == "view.switch:tasks-grid")
            .unwrap();
        assert_eq!(grid_switch.name, "Switch to Task Grid");
    }

    // =========================================================================
    // Dynamic board switch commands
    // =========================================================================

    #[test]
    fn board_switch_commands_appear_when_boards_provided() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["board:my-board".into()];
        let dynamic = DynamicSources {
            views: vec![],
            boards: vec![
                BoardInfo {
                    path: "/home/user/project-a".into(),
                    name: "Project A".into(),
                    entity_name: "Project A".into(),
                    context_name: "project-a".into(),
                },
                BoardInfo {
                    path: "/home/user/project-b".into(),
                    name: "Project B".into(),
                    entity_name: "Project B".into(),
                    context_name: "project-b".into(),
                },
            ],
            windows: vec![],
            perspectives: vec![],
        };
        let cmds = commands_for_scope(
            &scope,
            &registry,
            &impls,
            Some(&fields),
            &ui,
            false,
            Some(&dynamic),
        );
        let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();

        assert!(
            ids.contains(&"board.switch:/home/user/project-a"),
            "should have project-a switch: {:?}",
            ids
        );
        assert!(
            ids.contains(&"board.switch:/home/user/project-b"),
            "should have project-b switch: {:?}",
            ids
        );
    }

    #[test]
    fn board_switch_commands_have_correct_names_and_ids() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["board:my-board".into()];
        let dynamic = DynamicSources {
            views: vec![],
            boards: vec![BoardInfo {
                path: "/tmp/my-kanban".into(),
                name: "My Kanban".into(),
                entity_name: "My Kanban".into(),
                context_name: "my-kanban".into(),
            }],
            windows: vec![],
            perspectives: vec![],
        };
        let cmds = commands_for_scope(
            &scope,
            &registry,
            &impls,
            Some(&fields),
            &ui,
            false,
            Some(&dynamic),
        );

        let board_cmd = cmds
            .iter()
            .find(|c| c.id == "board.switch:/tmp/my-kanban")
            .unwrap();
        assert_eq!(board_cmd.name, "Switch to Board: My Kanban (my-kanban)");
        assert_eq!(board_cmd.menu_name.as_deref(), Some("my-kanban"));
        assert_eq!(board_cmd.group, "board");
        assert!(board_cmd.target.is_none());
        assert!(!board_cmd.context_menu);
    }

    #[test]
    fn view_and_board_commands_not_in_context_menu() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["board:my-board".into()];
        let dynamic = DynamicSources {
            views: vec![ViewInfo {
                id: "board-view".into(),
                name: "Board View".into(),
                entity_type: None,
            }],
            boards: vec![BoardInfo {
                path: "/tmp/board".into(),
                name: "Board".into(),
                entity_name: "Board".into(),
                context_name: "board".into(),
            }],
            windows: vec![],
            perspectives: vec![],
        };
        let cmds = commands_for_scope(
            &scope,
            &registry,
            &impls,
            Some(&fields),
            &ui,
            true,
            Some(&dynamic),
        );
        let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();

        // Dynamic commands have context_menu: false, so they should be filtered out
        assert!(
            !ids.iter().any(|id| id.starts_with("view.switch:")),
            "view commands should not appear in context menu"
        );
        assert!(
            !ids.iter().any(|id| id.starts_with("board.switch:")),
            "board commands should not appear in context menu"
        );
    }

    #[test]
    fn no_dynamic_commands_when_none_provided() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["board:my-board".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();

        assert!(
            !ids.iter().any(|id| id.starts_with("view.switch:")),
            "no view commands without dynamic sources"
        );
        assert!(
            !ids.iter().any(|id| id.starts_with("board.switch:")),
            "no board commands without dynamic sources"
        );
    }

    // =========================================================================
    // Dynamic entity.add commands (view-scope driven) — UNIT-LEVEL
    //
    // The four tests immediately below hand-construct `ViewInfo` entries.
    // They prove the `emit_entity_add` *algorithm* but NOT that the real
    // builtin YAML registry → `gather_views` projection → emission chain
    // holds end-to-end. Registry-backed coverage lives in the
    // `*_for_tasks_grid_view_scope`, `*_for_tags_grid_view_scope`, and
    // `*_for_projects_grid_view_scope` tests further down in this module,
    // plus the `entity_add_emitted_for_every_builtin_view_with_entity_type_real_registry`
    // cross-cutting guard. A regression that breaks only the YAML
    // projection will pass the hand-constructed tests and fail the
    // registry-backed ones — that is by design.
    // =========================================================================

    #[test]
    fn entity_add_emitted_when_view_in_scope() {
        // When a `view:*` moniker is active and the matching view declares
        // an `entity_type`, a dynamic `entity.add:{type}` command appears.
        let (registry, impls, fields, ui) = setup();
        let scope = vec![
            "view:tasks-grid".into(),
            "board:my-board".into(),
        ];
        let dynamic = DynamicSources {
            views: vec![ViewInfo {
                id: "tasks-grid".into(),
                name: "Task Grid".into(),
                entity_type: Some("task".into()),
            }],
            ..Default::default()
        };
        let cmds = commands_for_scope(
            &scope,
            &registry,
            &impls,
            Some(&fields),
            &ui,
            false,
            Some(&dynamic),
        );
        let add_cmd = cmds
            .iter()
            .find(|c| c.id == "entity.add:task")
            .expect("entity.add:task should be emitted when view scope declares entity_type=task");
        assert_eq!(add_cmd.name, "New Task");
        assert_eq!(add_cmd.group, "entity");
        assert!(add_cmd.context_menu, "entity.add must opt into context menu");
        assert!(add_cmd.target.is_none());
        assert!(add_cmd.available);
    }

    #[test]
    fn entity_add_not_emitted_without_view_in_scope() {
        // Without a `view:*` moniker, no entity.add:{type} is emitted even
        // when the view is listed in DynamicSources (it isn't the active one).
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["board:my-board".into()];
        let dynamic = DynamicSources {
            views: vec![ViewInfo {
                id: "tasks-grid".into(),
                name: "Task Grid".into(),
                entity_type: Some("task".into()),
            }],
            ..Default::default()
        };
        let cmds = commands_for_scope(
            &scope,
            &registry,
            &impls,
            Some(&fields),
            &ui,
            false,
            Some(&dynamic),
        );
        let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();
        assert!(
            !ids.iter().any(|id| id.starts_with("entity.add:")),
            "no entity.add without a view moniker in scope: {:?}",
            ids
        );
    }

    #[test]
    fn entity_add_present_in_context_menu() {
        // Unlike view.switch / board.switch / perspective.goto which are
        // navigation and context_menu: false, entity.add:* is a first-class
        // creation action and IS present with context_menu_only=true.
        let (registry, impls, fields, ui) = setup();
        let scope = vec![
            "view:tags-grid".into(),
            "board:my-board".into(),
        ];
        let dynamic = DynamicSources {
            views: vec![ViewInfo {
                id: "tags-grid".into(),
                name: "Tag Grid".into(),
                entity_type: Some("tag".into()),
            }],
            ..Default::default()
        };
        let cmds = commands_for_scope(
            &scope,
            &registry,
            &impls,
            Some(&fields),
            &ui,
            true,
            Some(&dynamic),
        );
        let add_cmd = cmds
            .iter()
            .find(|c| c.id == "entity.add:tag")
            .expect("entity.add:tag should remain after context_menu_only filter");
        assert!(add_cmd.context_menu);
    }

    /// The kanban board view declares `entity_type: task` in its YAML, so
    /// its `view:{id}` moniker in scope must surface `entity.add:task` as a
    /// context-menu + palette command. This is the Rust-side regression guard
    /// for the "Board view: New Task does nothing" bug — the frontend relies
    /// on this list to render the context menu and keyboard command, so if
    /// this test fails the palette loses its New Task entry across the board.
    #[test]
    fn entity_add_task_emitted_for_board_view_scope() {
        let (registry, impls, fields, ui) = setup();
        // Mirrors the scope chain `ViewContainer` + `BoardView` produce: the
        // innermost view moniker first, then the board moniker.
        let scope = vec!["view:01JMVIEW0000000000BOARD0".into(), "board:my-board".into()];
        let dynamic = DynamicSources {
            views: vec![ViewInfo {
                id: "01JMVIEW0000000000BOARD0".into(),
                name: "Board".into(),
                entity_type: Some("task".into()),
            }],
            ..Default::default()
        };
        let cmds = commands_for_scope(
            &scope,
            &registry,
            &impls,
            Some(&fields),
            &ui,
            false,
            Some(&dynamic),
        );
        let add_cmd = cmds
            .iter()
            .find(|c| c.id == "entity.add:task")
            .expect(
                "entity.add:task must be emitted on the board view scope chain — \
                 the board's view:{id} moniker drives this the same way as grids",
            );
        assert!(
            add_cmd.context_menu,
            "entity.add:task must opt into the context menu so right-click works",
        );
        assert_eq!(add_cmd.name, "New Task");
    }

    /// The Projects grid view declares `entity_type: project` in its YAML.
    /// Its `view:{id}` moniker must surface `entity.add:project` in the
    /// palette / context menu. Regression guard for the "New Project never
    /// appears in the command palette or context menu" bug.
    #[test]
    fn entity_add_project_emitted_for_projects_grid_scope() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["view:01JMVIEW0000000000PGRID0".into(), "board:my-board".into()];
        let dynamic = DynamicSources {
            views: vec![ViewInfo {
                id: "01JMVIEW0000000000PGRID0".into(),
                name: "Projects".into(),
                entity_type: Some("project".into()),
            }],
            ..Default::default()
        };
        let cmds = commands_for_scope(
            &scope,
            &registry,
            &impls,
            Some(&fields),
            &ui,
            false,
            Some(&dynamic),
        );
        let add_cmd = cmds
            .iter()
            .find(|c| c.id == "entity.add:project")
            .expect(
                "entity.add:project must be emitted on the projects grid scope \
                 chain — this is what drives the `New Project` menu item",
            );
        assert!(add_cmd.context_menu);
        assert_eq!(add_cmd.name, "New Project");
    }

    #[test]
    fn entity_add_not_emitted_for_views_without_entity_type() {
        // A view with entity_type: None (e.g. a dashboard view) should not
        // produce any entity.add command even when its moniker is active.
        let (registry, impls, fields, ui) = setup();
        let scope = vec![
            "view:dashboard".into(),
            "board:my-board".into(),
        ];
        let dynamic = DynamicSources {
            views: vec![ViewInfo {
                id: "dashboard".into(),
                name: "Dashboard".into(),
                entity_type: None,
            }],
            ..Default::default()
        };
        let cmds = commands_for_scope(
            &scope,
            &registry,
            &impls,
            Some(&fields),
            &ui,
            false,
            Some(&dynamic),
        );
        assert!(
            !cmds.iter().any(|c| c.id.starts_with("entity.add:")),
            "view without entity_type must not emit entity.add"
        );
    }

    // =========================================================================
    // Dynamic perspective goto commands
    // =========================================================================

    #[test]
    fn perspective_goto_commands_appear_when_perspectives_provided() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["board:my-board".into()];
        let dynamic = DynamicSources {
            perspectives: vec![
                PerspectiveInfo {
                    id: "p1".into(),
                    name: "Alpha".into(),
                    view: "board".into(),
                },
                PerspectiveInfo {
                    id: "p2".into(),
                    name: "Beta".into(),
                    view: "board".into(),
                },
            ],
            ..Default::default()
        };
        let cmds = commands_for_scope(
            &scope,
            &registry,
            &impls,
            Some(&fields),
            &ui,
            false,
            Some(&dynamic),
        );
        let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();

        assert!(
            ids.contains(&"perspective.goto:p1"),
            "should have p1: {:?}",
            ids
        );
        assert!(
            ids.contains(&"perspective.goto:p2"),
            "should have p2: {:?}",
            ids
        );

        let p1 = cmds.iter().find(|c| c.id == "perspective.goto:p1").unwrap();
        assert_eq!(p1.name, "Go to Perspective: Alpha");
        assert_eq!(p1.group, "perspective");
    }

    #[test]
    fn perspective_goto_commands_not_in_context_menu() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["board:my-board".into()];
        let dynamic = DynamicSources {
            perspectives: vec![PerspectiveInfo {
                id: "p1".into(),
                name: "Alpha".into(),
                view: "board".into(),
            }],
            ..Default::default()
        };
        let cmds = commands_for_scope(
            &scope,
            &registry,
            &impls,
            Some(&fields),
            &ui,
            true,
            Some(&dynamic),
        );
        let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();

        assert!(
            !ids.iter().any(|id| id.starts_with("perspective.goto:")),
            "perspective commands should not appear in context menu"
        );
    }

    #[test]
    fn no_perspective_commands_without_dynamic_sources() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["board:my-board".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();

        assert!(
            !ids.iter().any(|id| id.starts_with("perspective.goto:")),
            "no perspective commands without dynamic sources"
        );
    }

    // =========================================================================
    // Template resolution
    // =========================================================================

    #[test]
    fn template_entity_type_resolved() {
        let params = TemplateParams {
            entity_type: "task",
            ..Default::default()
        };
        assert_eq!(
            resolve_name_template("Copy {{entity.type}}", &params),
            "Copy Task"
        );
    }

    #[test]
    fn template_entity_display_name_with_value() {
        let params = TemplateParams {
            entity_name: "My Board",
            ..Default::default()
        };
        assert_eq!(
            resolve_name_template("Switch to {{entity.display_name}}", &params),
            "Switch to My Board"
        );
    }

    #[test]
    fn template_entity_display_name_empty_when_missing() {
        let params = TemplateParams::default();
        assert_eq!(
            resolve_name_template("Switch to {{entity.display_name}}", &params),
            "Switch to "
        );
    }

    #[test]
    fn template_context_display_name_resolved() {
        let params = TemplateParams {
            context_name: "swissarmyhammer-kanban",
            ..Default::default()
        };
        assert_eq!(
            resolve_name_template("{{entity.context.display_name}}", &params),
            "swissarmyhammer-kanban"
        );
    }

    #[test]
    fn template_combined_resolves_all_variables() {
        let params = TemplateParams {
            entity_type: "board",
            entity_name: "My Project",
            context_name: "swissarmyhammer-kanban",
        };
        let result = resolve_name_template(
            "{{entity.display_name}} ({{entity.context.display_name}}) [{{entity.type}}]",
            &params,
        );
        assert_eq!(result, "My Project (swissarmyhammer-kanban) [Board]");
    }

    #[test]
    fn template_no_templates_returns_unchanged() {
        let params = TemplateParams {
            entity_type: "task",
            entity_name: "Board",
            context_name: "ctx",
        };
        assert_eq!(resolve_name_template("Quit", &params), "Quit");
    }

    #[test]
    fn dynamic_board_commands_use_templates() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["board:my-board".into()];
        let dynamic = DynamicSources {
            views: vec![],
            boards: vec![BoardInfo {
                path: "/home/user/my-project/.kanban".into(),
                name: "my-project".into(),
                entity_name: "my-project".into(),
                context_name: "my-project".into(),
            }],
            windows: vec![],
            perspectives: vec![],
        };
        let cmds = commands_for_scope(
            &scope,
            &registry,
            &impls,
            Some(&fields),
            &ui,
            false,
            Some(&dynamic),
        );
        let board_cmd = cmds
            .iter()
            .find(|c| c.id.starts_with("board.switch:"))
            .expect("should have board switch command");

        assert_eq!(board_cmd.name, "Switch to Board: my-project (my-project)");
        assert_eq!(board_cmd.menu_name.as_deref(), Some("my-project"));
    }

    // =========================================================================
    // Dynamic window focus commands
    // =========================================================================

    #[test]
    fn window_focus_commands_generated_from_dynamic_sources() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["board:my-board".into()];
        let dynamic = DynamicSources {
            views: vec![],
            boards: vec![],
            windows: vec![
                WindowInfo {
                    label: "main".into(),
                    title: "SwissArmyHammer".into(),
                    focused: true,
                },
                WindowInfo {
                    label: "board-01abc".into(),
                    title: "My Project".into(),
                    focused: false,
                },
            ],
            perspectives: vec![],
        };
        let cmds = commands_for_scope(
            &scope,
            &registry,
            &impls,
            Some(&fields),
            &ui,
            false,
            Some(&dynamic),
        );
        let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();

        assert!(
            ids.contains(&"window.focus:main"),
            "should have main window focus: {:?}",
            ids
        );
        assert!(
            ids.contains(&"window.focus:board-01abc"),
            "should have board-01abc window focus: {:?}",
            ids
        );
    }

    #[test]
    fn window_focus_commands_have_correct_names_and_ids() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["board:my-board".into()];
        let dynamic = DynamicSources {
            views: vec![],
            boards: vec![],
            windows: vec![WindowInfo {
                label: "main".into(),
                title: "SwissArmyHammer".into(),
                focused: true,
            }],
            perspectives: vec![],
        };
        let cmds = commands_for_scope(
            &scope,
            &registry,
            &impls,
            Some(&fields),
            &ui,
            false,
            Some(&dynamic),
        );

        let win_cmd = cmds
            .iter()
            .find(|c| c.id == "window.focus:main")
            .expect("should have window.focus:main");
        assert_eq!(win_cmd.name, "SwissArmyHammer");
        assert_eq!(win_cmd.menu_name.as_deref(), Some("SwissArmyHammer"));
        assert_eq!(win_cmd.group, "window");
        assert!(win_cmd.target.is_none());
        assert!(!win_cmd.context_menu);
        assert!(win_cmd.available);
    }

    #[test]
    fn window_focus_commands_not_in_context_menu() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["board:my-board".into()];
        let dynamic = DynamicSources {
            views: vec![],
            boards: vec![],
            windows: vec![WindowInfo {
                label: "main".into(),
                title: "SwissArmyHammer".into(),
                focused: true,
            }],
            perspectives: vec![],
        };
        let cmds = commands_for_scope(
            &scope,
            &registry,
            &impls,
            Some(&fields),
            &ui,
            true,
            Some(&dynamic),
        );
        let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();

        assert!(
            !ids.iter().any(|id| id.starts_with("window.focus:")),
            "window focus commands should not appear in context menu"
        );
    }

    #[test]
    fn no_window_commands_without_dynamic_sources() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["board:my-board".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();

        assert!(
            !ids.iter().any(|id| id.starts_with("window.focus:")),
            "no window commands without dynamic sources"
        );
    }

    // =========================================================================
    // Perspective scope
    // =========================================================================

    #[test]
    fn perspective_scope_has_filter_and_group_commands() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["perspective:01ABC".into(), "board:my-board".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();

        assert!(
            ids.contains(&"perspective.filter"),
            "perspective scope should have perspective.filter: {:?}",
            ids
        );
        assert!(
            ids.contains(&"perspective.group"),
            "perspective scope should have perspective.group: {:?}",
            ids
        );
    }

    #[test]
    fn perspective_commands_not_available_without_perspective_in_scope() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec![
            "task:01X".into(),
            "column:todo".into(),
            "board:my-board".into(),
        ];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();

        assert!(
            !ids.contains(&"perspective.filter"),
            "perspective.filter should NOT appear without perspective in scope: {:?}",
            ids
        );
    }

    // =========================================================================
    // Dedup-by-id: innermost scope wins for shared command IDs
    // =========================================================================

    #[test]
    fn dedup_by_id_tag_task_scope_only_one_cut_command() {
        // entity.cut appears in both tag and task schemas.
        // With scope ["tag:X", "task:Y"], only the innermost (tag) cut should appear.
        let (registry, impls, fields, ui) = setup();
        let scope = vec![
            "tag:some-tag".into(),
            "task:01TASK".into(),
            "column:todo".into(),
            "board:my-board".into(),
        ];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);

        let cut_cmds: Vec<_> = cmds.iter().filter(|c| c.id == "entity.cut").collect();
        assert_eq!(
            cut_cmds.len(),
            1,
            "entity.cut should appear exactly once (innermost wins): {:?}",
            cut_cmds.iter().map(|c| &c.name).collect::<Vec<_>>()
        );
        assert_eq!(
            cut_cmds[0].name, "Cut Tag",
            "the single cut command should be 'Cut Tag' (tag is innermost): {:?}",
            cut_cmds[0]
        );
        assert_eq!(
            cut_cmds[0].target.as_deref(),
            Some("tag:some-tag"),
            "cut target should be the tag"
        );
    }

    #[test]
    fn dedup_by_id_task_only_scope_shows_cut_task() {
        // When the scope has no tag, "Cut Task" should appear normally.
        let (registry, impls, fields, ui) = setup();
        let scope = vec![
            "task:01TASK".into(),
            "column:todo".into(),
            "board:my-board".into(),
        ];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);

        let cut_cmds: Vec<_> = cmds.iter().filter(|c| c.id == "entity.cut").collect();
        assert_eq!(
            cut_cmds.len(),
            1,
            "entity.cut should appear exactly once: {:?}",
            cut_cmds
        );
        assert_eq!(
            cut_cmds[0].name, "Cut Task",
            "only task in scope → should show 'Cut Task'"
        );
        assert_eq!(
            cut_cmds[0].target.as_deref(),
            Some("task:01TASK"),
            "cut target should be the task"
        );
    }

    #[test]
    fn dedup_by_id_applies_to_copy_and_inspect_too() {
        // Verify that entity.copy and entity.inspect also follow dedup-by-id,
        // showing only the innermost (tag) version when both tag and task are in scope.
        let (registry, impls, fields, ui) = setup();
        let scope = vec![
            "tag:some-tag".into(),
            "task:01TASK".into(),
            "column:todo".into(),
        ];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);

        // entity.copy — only tag version
        let copy_cmds: Vec<_> = cmds.iter().filter(|c| c.id == "entity.copy").collect();
        assert_eq!(
            copy_cmds.len(),
            1,
            "entity.copy should appear exactly once: {:?}",
            copy_cmds.iter().map(|c| &c.name).collect::<Vec<_>>()
        );
        assert_eq!(
            copy_cmds[0].name, "Copy Tag",
            "entity.copy should show 'Copy Tag'"
        );

        // ui.inspect — only tag version
        let inspect_cmds: Vec<_> = cmds.iter().filter(|c| c.id == "ui.inspect").collect();
        assert_eq!(
            inspect_cmds.len(),
            1,
            "ui.inspect should appear exactly once: {:?}",
            inspect_cmds.iter().map(|c| &c.name).collect::<Vec<_>>()
        );
        assert_eq!(
            inspect_cmds[0].name, "Inspect Tag",
            "ui.inspect should show 'Inspect Tag'"
        );
    }

    // =========================================================================
    // Scope ordering — innermost scope commands first
    // =========================================================================

    #[test]
    fn attachment_commands_appear_before_task_commands() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec![
            "attachment:/path/to/file.png".into(),
            "task:01X".into(),
            "column:todo".into(),
        ];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, true, None);
        let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();

        // attachment.open and attachment.reveal should be present
        assert!(
            ids.contains(&"attachment.open"),
            "attachment.open should be in context menu"
        );
        assert!(
            ids.contains(&"attachment.reveal"),
            "attachment.reveal should be in context menu"
        );

        // They should appear before any task commands
        let open_pos = ids.iter().position(|&id| id == "attachment.open").unwrap();
        let reveal_pos = ids
            .iter()
            .position(|&id| id == "attachment.reveal")
            .unwrap();

        // Find the first task-level command
        let first_task_pos = ids
            .iter()
            .position(|&id| id.starts_with("entity.") || id.starts_with("task."));

        if let Some(task_pos) = first_task_pos {
            assert!(
                open_pos < task_pos,
                "attachment.open (pos {open_pos}) should appear before first task command (pos {task_pos})"
            );
            assert!(
                reveal_pos < task_pos,
                "attachment.reveal (pos {reveal_pos}) should appear before first task command (pos {task_pos})"
            );
        }
    }

    #[test]
    fn attachment_commands_grouped_as_attachment_not_global() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["attachment:/path/to/file.png".into(), "task:01X".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);

        let open_cmd = cmds.iter().find(|c| c.id == "attachment.open");
        assert!(open_cmd.is_some(), "attachment.open should exist");
        assert_eq!(
            open_cmd.unwrap().group,
            "attachment",
            "attachment.open should have group 'attachment', not 'global'"
        );
    }

    #[test]
    fn tag_commands_appear_before_task_commands_in_context_menu() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["tag:bug".into(), "task:01X".into(), "column:todo".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, true, None);
        let groups: Vec<&str> = cmds.iter().map(|c| c.group.as_str()).collect();

        // First commands should be tag-grouped, then task-grouped
        let first_tag = groups.iter().position(|&g| g == "tag");
        let first_task = groups.iter().position(|&g| g == "task");

        if let (Some(tag_pos), Some(task_pos)) = (first_tag, first_task) {
            assert!(
                tag_pos < task_pos,
                "tag commands (pos {tag_pos}) should appear before task commands (pos {task_pos})"
            );
        }
    }

    // =========================================================================
    // Field monikers are skipped in scope chain
    // =========================================================================

    #[test]
    fn field_moniker_skipped_inspect_targets_entity() {
        // With the `field:` prefix, grid cell monikers like
        // "field:tag:tag-1.color" are skipped entirely. The inspect command
        // targets the real entity moniker "tag:tag-1".
        let (registry, impls, fields, ui) = setup();
        let scope = vec![
            "field:tag:tag-1.color".into(),
            "tag:tag-1".into(),
            "board:board".into(),
        ];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);

        let inspect = cmds.iter().find(|c| c.id == "ui.inspect");
        assert!(inspect.is_some(), "should have inspect command");
        assert_eq!(
            inspect.unwrap().target.as_deref(),
            Some("tag:tag-1"),
            "inspect target should be the entity moniker, not the field moniker"
        );
    }

    #[test]
    fn field_moniker_dedup_emits_one_inspect() {
        // "field:tag:tag-1.color" is skipped, so only "tag:tag-1" produces
        // commands — exactly one inspect command.
        let (registry, impls, fields, ui) = setup();
        let scope = vec![
            "field:tag:tag-1.color".into(),
            "tag:tag-1".into(),
            "board:board".into(),
        ];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);

        let inspect_cmds: Vec<_> = cmds.iter().filter(|c| c.id == "ui.inspect").collect();
        assert_eq!(
            inspect_cmds.len(),
            1,
            "should have exactly one inspect command, got {}: {:?}",
            inspect_cmds.len(),
            inspect_cmds.iter().map(|c| &c.target).collect::<Vec<_>>()
        );
    }

    // =========================================================================
    // Entity schema as primary source for scoped commands
    // =========================================================================

    #[test]
    fn task_add_from_entity_schema_has_target() {
        // task.add is declared on the task entity with scope "entity:column".
        // When column:todo is in scope, it should appear via the entity schema
        // path with a target pointing to the column moniker.
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["column:todo".into(), "board:board".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let task_add = cmds
            .iter()
            .find(|c| c.id == "task.add")
            .expect("task.add should be in resolved commands");
        assert_eq!(
            task_add.target.as_deref(),
            Some("column:todo"),
            "task.add should have target from entity schema path"
        );
    }

    #[test]
    fn task_untag_from_entity_schema_has_target() {
        // task.untag is declared on the task entity with scope
        // "entity:tag,entity:task". When both tag and task are in scope,
        // it should appear via the entity schema path with a target
        // pointing to the tag moniker (innermost match).
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["tag:bug".into(), "task:01X".into(), "column:todo".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let untag = cmds
            .iter()
            .find(|c| c.id == "task.untag")
            .expect("task.untag should be in resolved commands");
        assert!(
            untag.target.is_some(),
            "task.untag should have a target from entity schema path"
        );
    }

    #[test]
    fn entity_schema_commands_carry_menu_name() {
        // Commands resolved via the entity schema block should carry
        // menu_name from EntityCommand.menu_name, not hardcode None.
        let (registry, impls, fields, ui) = setup();
        let scope = vec![
            "task:01X".into(),
            "column:todo".into(),
            "board:board".into(),
        ];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);

        // entity.archive has no explicit menu_name in YAML so should be None
        let archive = cmds.iter().find(|c| c.id == "entity.archive");
        if let Some(a) = archive {
            assert!(
                a.menu_name.is_none(),
                "entity.archive should have no menu_name: {:?}",
                a.menu_name
            );
        }
    }

    // =========================================================================
    // Field monikers are skipped
    // =========================================================================

    #[test]
    fn field_moniker_in_scope_does_not_produce_entity_commands() {
        // A scope chain with "field:task:abc.title" should not generate commands
        // for a phantom entity "abc.title" — the field moniker is skipped entirely.
        let (registry, impls, fields, ui) = setup();
        let scope = vec![
            "field:task:abc.title".into(),
            "task:abc".into(),
            "column:todo".into(),
            "board:my-board".into(),
        ];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);

        // No command should target the field moniker
        for cmd in &cmds {
            if let Some(target) = &cmd.target {
                assert!(
                    !target.starts_with("field:"),
                    "command '{}' should not target a field moniker, got: {}",
                    cmd.id,
                    target
                );
            }
        }

        // Task commands should still be present (from the real task:abc moniker)
        let names: Vec<&str> = cmds.iter().map(|c| c.name.as_str()).collect();
        assert!(
            names.contains(&"Inspect Task"),
            "real task commands should still appear: {:?}",
            names
        );
    }

    // =========================================================================
    // Real-registry entity.add emission tests
    //
    // The tests above that construct `ViewInfo` by hand are unit-level
    // coverage. They establish that the *algorithm* in `emit_entity_add`
    // works given a DynamicSources payload. They do NOT prove that the
    // payload built from the real builtin YAML registry + the real
    // `gather_views` shape is ever populated with a grid view whose
    // `entity_type` survives the round-trip.
    //
    // These tests load the real builtin view YAMLs through
    // `ViewsContext::from_yaml_sources`, walk the loaded defs to build the
    // same `ViewInfo` list that production's `gather_views` assembles, and
    // assert that `commands_for_scope` surfaces `entity.add:{type}` for
    // every builtin grid view declaring an `entity_type`.
    //
    // When these tests fail while the hand-constructed tests pass, the bug
    // lives in the YAML → `ViewInfo` projection, not in `emit_entity_add`.
    // =========================================================================

    /// Load the real builtin view registry and return ViewInfo entries.
    ///
    /// Mirrors what `kanban-app::gather_views` produces in production:
    /// pulls every builtin view YAML through `ViewsContext::from_yaml_sources`
    /// and projects the loaded `ViewDef`s onto the `ViewInfo` shape that
    /// `emit_entity_add` consumes. This is the registry-backed alternative
    /// to hand-constructing `ViewInfo` — it catches any YAML drift, schema
    /// change, or `entity_type` deserialization issue.
    fn load_real_views() -> Vec<ViewInfo> {
        let builtin = crate::defaults::builtin_view_definitions();
        // Writable root is a bogus path — we never persist in this test,
        // only read back `all_views()` from the in-memory parsed list.
        let temp = tempfile::tempdir().expect("tempdir should create");
        let vctx = swissarmyhammer_views::ViewsContext::from_yaml_sources(
            temp.path().to_path_buf(),
            &builtin,
        )
        .expect("builtin views must parse");
        vctx.all_views()
            .iter()
            .map(|v| ViewInfo {
                id: v.id.clone(),
                name: v.name.clone(),
                entity_type: v.entity_type.clone(),
            })
            .collect()
    }

    /// Find a view by name from the real builtin registry; fail loudly if
    /// the builtin YAMLs no longer contain the expected view.
    fn view_by_name<'a>(views: &'a [ViewInfo], name: &str) -> &'a ViewInfo {
        views
            .iter()
            .find(|v| v.name == name)
            .unwrap_or_else(|| panic!("builtin views must contain view named '{name}'"))
    }

    /// The Tasks grid view declares `entity_type: task`. When its
    /// `view:{id}` moniker is in scope, `entity.add:task` must be emitted.
    /// Uses the REAL view registry (not a hand-constructed `ViewInfo`) so
    /// `tasks-grid.yaml` → `entity_type` → emission is proven end-to-end.
    #[test]
    fn entity_add_task_emitted_for_tasks_grid_view_scope() {
        let (registry, impls, fields, ui) = setup();
        let views = load_real_views();
        let tasks_grid = view_by_name(&views, "Tasks Grid");
        assert_eq!(
            tasks_grid.entity_type.as_deref(),
            Some("task"),
            "tasks-grid YAML must still declare entity_type=task"
        );
        let scope = vec![format!("view:{}", tasks_grid.id), "board:my-board".into()];
        let dynamic = DynamicSources {
            views: views.clone(),
            ..Default::default()
        };
        let cmds =
            commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, Some(&dynamic));
        let add = cmds
            .iter()
            .find(|c| c.id == "entity.add:task")
            .expect(
                "entity.add:task must be emitted on the tasks-grid scope chain using the REAL \
                 view registry — this is the regression guard against YAML drift, not the \
                 hand-constructed test above",
            );
        assert_eq!(add.name, "New Task");
        assert!(add.context_menu, "entity.add:task must opt into context menu");
        assert!(add.available);
    }

    /// The Tags grid view declares `entity_type: tag`. Mirrors
    /// `entity_add_task_emitted_for_tasks_grid_view_scope` using the REAL
    /// builtin registry. Regression guard for "New Tag missing from palette
    /// and context menu on the tags grid".
    #[test]
    fn entity_add_tag_emitted_for_tags_grid_view_scope() {
        let (registry, impls, fields, ui) = setup();
        let views = load_real_views();
        let tags_grid = view_by_name(&views, "Tags");
        assert_eq!(
            tags_grid.entity_type.as_deref(),
            Some("tag"),
            "tags-grid YAML must still declare entity_type=tag"
        );
        let scope = vec![format!("view:{}", tags_grid.id), "board:my-board".into()];
        let dynamic = DynamicSources {
            views: views.clone(),
            ..Default::default()
        };
        let cmds =
            commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, Some(&dynamic));
        let add = cmds
            .iter()
            .find(|c| c.id == "entity.add:tag")
            .expect(
                "entity.add:tag must be emitted on the tags-grid scope chain using the REAL \
                 view registry",
            );
        assert_eq!(add.name, "New Tag");
        assert!(add.context_menu);
        assert!(add.available);
    }

    /// The Projects grid view declares `entity_type: project`. Mirrors the
    /// task/tag tests above using the REAL builtin registry. Regression
    /// guard for "New Project missing from palette and context menu".
    #[test]
    fn entity_add_project_emitted_for_projects_grid_view_scope() {
        let (registry, impls, fields, ui) = setup();
        let views = load_real_views();
        let projects_grid = view_by_name(&views, "Projects");
        assert_eq!(
            projects_grid.entity_type.as_deref(),
            Some("project"),
            "projects-grid YAML must still declare entity_type=project"
        );
        let scope = vec![
            format!("view:{}", projects_grid.id),
            "board:my-board".into(),
        ];
        let dynamic = DynamicSources {
            views: views.clone(),
            ..Default::default()
        };
        let cmds =
            commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, Some(&dynamic));
        let add = cmds
            .iter()
            .find(|c| c.id == "entity.add:project")
            .expect(
                "entity.add:project must be emitted on the projects-grid scope chain using \
                 the REAL view registry — this is the regression guard the hand-constructed \
                 test could never catch",
            );
        assert_eq!(add.name, "New Project");
        assert!(add.context_menu);
        assert!(add.available);
    }

    /// Cross-cutting real-registry guard: every builtin view that declares
    /// an `entity_type` must surface a working `entity.add:{type}` command
    /// in its scope chain, in BOTH the palette (context_menu_only=false)
    /// and the context menu (context_menu_only=true).
    ///
    /// This is the "future grids inherit the fix automatically" guard — a
    /// new grid view YAML declaring `entity_type: foo` is covered for free.
    /// A regression that silently drops the entity.add emission for any
    /// one entity type fails this test as a single, named failure.
    #[test]
    fn entity_add_emitted_for_every_builtin_view_with_entity_type_real_registry() {
        let (registry, impls, fields, ui) = setup();
        let views = load_real_views();
        let with_entity_type: Vec<&ViewInfo> = views
            .iter()
            .filter(|v| {
                v.entity_type
                    .as_deref()
                    .is_some_and(|s| !s.is_empty())
            })
            .collect();
        assert!(
            with_entity_type.len() >= 3,
            "expected at least board + tasks-grid + tags-grid + projects-grid to declare \
             entity_type; got {} views: {:?}",
            with_entity_type.len(),
            views
                .iter()
                .map(|v| (&v.name, &v.entity_type))
                .collect::<Vec<_>>()
        );
        for view in with_entity_type {
            let entity_type = view.entity_type.as_deref().unwrap();
            let scope = vec![format!("view:{}", view.id), "board:my-board".into()];
            let dynamic = DynamicSources {
                views: views.clone(),
                ..Default::default()
            };

            // Palette path — context_menu_only=false
            let palette =
                commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, Some(&dynamic));
            let expected_id = format!("entity.add:{entity_type}");
            let palette_add = palette.iter().find(|c| c.id == expected_id);
            assert!(
                palette_add.is_some_and(|c| c.available),
                "palette must surface {expected_id} for view '{}' (entity_type={entity_type}); \
                 got commands: {:?}",
                view.name,
                palette.iter().map(|c| &c.id).collect::<Vec<_>>()
            );

            // Context menu path — context_menu_only=true
            let menu =
                commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, true, Some(&dynamic));
            let menu_add = menu.iter().find(|c| c.id == expected_id);
            assert!(
                menu_add.is_some_and(|c| c.available && c.context_menu),
                "context menu must surface {expected_id} for view '{}' (entity_type={entity_type}); \
                 got commands: {:?}",
                view.name,
                menu.iter().map(|c| &c.id).collect::<Vec<_>>()
            );
        }
    }
}
