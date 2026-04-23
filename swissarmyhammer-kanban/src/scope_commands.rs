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
//!   4. **dynamic** — runtime-generated commands like `view.switch:{id}`,
//!      `board.switch:{path}`, `entity.add:{type}`.
//!
//! Within each phase, the shared `(id, target)` seen-set guarantees a command
//! cannot double-emit for the same target.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use swissarmyhammer_commands::{
    Command, CommandContext, CommandDef, CommandsRegistry, KeysDef, ParamSource, UIState,
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

/// Push a command once, honoring the `(id, target)` seen-set for dedup.
///
/// Shared across all `emit_*` helpers below so that overlapping emitters (and
/// repeated scope monikers) can never produce duplicate commands in the same
/// `commands_for_scope` result.
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

/// Emit one `view.switch:{id}` command per known view.
///
/// The per-view `context_menu` flag is computed from `scope_chain`: the
/// command is marked `context_menu: true` **only** for the view whose
/// `view:{id}` moniker is present in the scope chain. All other views stay
/// `context_menu: false`.
///
/// This gives the left-nav right-click exactly one "Switch to <ViewName>"
/// entry — the one for the button the user actually right-clicked — while
/// leaving palette behavior (`context_menu_only == false`) untouched: the
/// palette still shows every view.switch command regardless of scope.
///
/// Mirrors the scope-chain-filtering pattern used by `emit_entity_add`.
/// Shares `seen` with the other emit_* helpers so cross-emitter dedup works.
fn emit_view_switch(
    views: &[ViewInfo],
    scope_chain: &[String],
    seen: &mut HashSet<(String, Option<String>)>,
    result: &mut Vec<ResolvedCommand>,
) {
    for view in views {
        let view_moniker = format!("view:{}", view.id);
        let in_scope = scope_chain.iter().any(|m| m == &view_moniker);
        push_dedup(
            seen,
            result,
            ResolvedCommand {
                id: format!("view.switch:{}", view.id),
                name: format!("Switch to {}", view.name),
                menu_name: None,
                target: None,
                group: "view".into(),
                context_menu: in_scope,
                keys: None,
                available: true,
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

/// Emit one `window.focus:{label}` command per known window.
///
/// The displayed name is the window's title (e.g. the board path it shows).
/// Marked `context_menu: false` (palette-only). Shares `seen` with the other
/// emit_* helpers so cross-emitter dedup works.
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

/// Emit one `perspective.goto:{id}` command per known perspective.
///
/// Marked `context_menu: false` (palette-only). Shares `seen` with the other
/// emit_* helpers so cross-emitter dedup works.
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
    seen: &mut HashSet<(String, Option<String>)>,
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
            },
        );
    }
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
fn emit_dynamic_commands(
    dyn_src: &DynamicSources,
    scope_chain: &[String],
    seen: &mut HashSet<(String, Option<String>)>,
    result: &mut Vec<ResolvedCommand>,
) {
    // Index views by id once so the `entity.add` emission below is O(scope)
    // rather than O(scope × views).
    let views_by_id: HashMap<&str, &ViewInfo> =
        dyn_src.views.iter().map(|v| (v.id.as_str(), v)).collect();
    emit_view_switch(&dyn_src.views, scope_chain, seen, result);
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
    seen: &mut HashSet<(String, Option<String>)>,
    result: &mut Vec<ResolvedCommand>,
) {
    let scope_prefixed = format!("entity:{entity_type}");
    // Pre-count target-primary commands so the entry trace records how many
    // candidates this pass sees before per-command filtering — mirrors the
    // shape of `emit_entity_add`'s entry diagnostic (scope size + candidate
    // count).
    let target_primary_count = all_registry_cmds
        .iter()
        .filter(|c| {
            c.params
                .first()
                .is_some_and(|p| p.from == ParamSource::Target)
        })
        .count();
    tracing::debug!(
        scope_moniker = %moniker,
        entity_type = %entity_type,
        entity_moniker = %entity_moniker,
        scope_chain_len = scope_chain.len(),
        registry_total = all_registry_cmds.len(),
        target_primary_count,
        "emit_cross_cutting_commands: entering pass"
    );
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
    let matched = pending.len();
    for p in pending {
        push_dedup(seen, result, p.cmd);
    }
    tracing::debug!(
        scope_moniker = %moniker,
        entity_type = %entity_type,
        matched,
        "emit_cross_cutting_commands: pass complete"
    );
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
    seen: &HashSet<(String, Option<String>)>,
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
    // in the trace — push_dedup itself silently drops duplicates.
    let dedup_key = (cmd_def.id.clone(), target.clone());
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
        // entity.copy / entity.cut are now target-driven cross-cutting
        // commands and auto-emit on every entity moniker in scope, including
        // boards — copying a board to the clipboard is a meaningful op.
    }

    #[test]
    fn board_scope_no_paste_without_clipboard() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["board:my-board".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();
        assert!(!ids.contains(&"entity.paste"), "no paste without clipboard");
    }

    /// With a task on the clipboard and a board in scope, `entity.paste` must
    /// surface as an available command — `PasteEntityCmd::available()` returns
    /// true because task-on-clipboard + board-in-scope is a valid paste target
    /// (paste creates a task in the board's first column).
    ///
    /// This test pins the behavior that drives "right-click on a board
    /// background shows Paste" without `board.yaml` opting into
    /// `entity.paste` directly: the command must come from the registry's
    /// global emission pass alone, gated by `PasteEntityCmd::available()`
    /// against the target moniker and clipboard state.
    #[test]
    fn entity_paste_surfaces_on_board_when_task_clipboard() {
        let (registry, impls, fields, ui) = setup();
        ui.set_clipboard_entity_type("task");
        let scope = vec!["board:main".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);

        let paste = cmds
            .iter()
            .find(|c| c.id == "entity.paste")
            .unwrap_or_else(|| {
                panic!(
                    "entity.paste must surface on board scope when a task is on \
                     the clipboard; got commands: {:?}",
                    cmds.iter().map(|c| &c.id).collect::<Vec<_>>()
                )
            });
        // `commands_for_scope` filters out unavailable commands at the end of
        // its pipeline, so a `find` hit already implies `available: true`.
        // The explicit assertion documents the contract for future readers.
        assert!(
            paste.available,
            "entity.paste must be available (task clipboard + board in scope is a \
             valid paste target)"
        );
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

    /// Regression guard for https://… — right-clicking a task used to render
    /// two identical "Delete Task" entries in the context menu: one from the
    /// cross-cutting `entity.delete` (template-resolved to "Delete Task") and
    /// one from the retired type-specific `task.delete` (hardcoded name).
    ///
    /// The fix removes `task.delete` entirely and migrates its only unique
    /// affordance (the `Mod+Backspace` keybinding) onto `entity.delete`.
    /// This test pins the surface contract: exactly one context-menu command
    /// whose display name is "Delete Task", and its id is `entity.delete`.
    #[test]
    fn task_context_menu_has_exactly_one_delete_task() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["task:01X".into(), "column:todo".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, true, None);

        let deletes: Vec<&ResolvedCommand> =
            cmds.iter().filter(|c| c.name == "Delete Task").collect();

        assert_eq!(
            deletes.len(),
            1,
            "expected exactly one 'Delete Task' in the task context menu, got {}: {:?}",
            deletes.len(),
            deletes.iter().map(|c| &c.id).collect::<Vec<_>>()
        );
        assert_eq!(
            deletes[0].id, "entity.delete",
            "the surviving 'Delete Task' must be the cross-cutting `entity.delete`, \
             not a type-specific `task.delete`"
        );
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
    fn tag_clipboard_column_focused_no_paste() {
        // Tag on clipboard + column focused (no task) → can't paste tag here
        let (registry, impls, fields, ui) = setup();
        ui.set_clipboard_entity_type("tag");
        let scope = vec!["column:todo".into(), "board:board".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let paste: Vec<_> = cmds.iter().filter(|c| c.id == "entity.paste").collect();
        assert!(paste.is_empty(), "can't paste tag without task in scope");
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
        // A tag is in scope but no task — `entity.cut` with a tag target
        // requires a task in scope to untag from. `CutEntityCmd::available()`
        // gates this and the auto-emitted command must be filtered out.
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["tag:bug".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let cut_tag = cmds
            .iter()
            .find(|c| c.id == "entity.cut" && c.target.as_deref() == Some("tag:bug"));
        assert!(
            cut_tag.is_none(),
            "entity.cut on a tag target must NOT surface without a task in \
             scope (no destructive op is defined); got: {:?}",
            cut_tag,
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

    /// Task creation must flow through the dynamic `entity.add:task`
    /// emission (driven by the active view's `entity_type`), NOT the legacy
    /// `task.add` registry entry. Having both live produced duplicate
    /// "New Task" items in the palette and a slug-id collision that caused
    /// the second and later creates to silently drop.
    #[test]
    fn task_add_never_emitted_from_registry() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["column:todo".into(), "board:board".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();
        assert!(
            !ids.contains(&"task.add"),
            "task.add must be gone — creation is dynamic `entity.add:task`. got: {:?}",
            ids
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

    /// `ui.inspect` must surface on an actor scope purely from the cross-cutting
    /// auto-emit pass — `actor.yaml` declares no `commands:` opt-in, so the only
    /// way `ui.inspect` reaches an actor moniker is the dispatcher walking
    /// `from: target` registry commands and emitting them per-moniker.
    ///
    /// This is the GREEN-step companion to the YAML hygiene guard
    /// (`yaml_hygiene_entity_schemas_have_no_commands_key`): that test forbids
    /// entity YAML files from declaring any `commands:` key at all, this test
    /// proves the command still appears without any per-entity opt-in.
    /// Together they pin the "declare once, auto-emit per moniker" contract
    /// for actors.
    #[test]
    fn ui_inspect_auto_emits_on_actor_without_opt_in() {
        // Guard: if a future change re-introduces a `commands:` block on
        // actor.yaml (or otherwise re-lists `ui.inspect` there), this test's
        // premise — that auto-emit alone is responsible for the surfaced
        // command — is invalidated. Fail loudly rather than silently passing
        // for the wrong reason.
        let actor_yaml = builtin_entity_definitions()
            .into_iter()
            .find_map(|(name, yaml)| (name == "actor").then_some(yaml))
            .expect("builtin entity definitions must include actor");
        let actor_raw: serde_yaml_ng::Value = serde_yaml_ng::from_str(actor_yaml)
            .expect("builtin actor.yaml must parse as generic YAML");
        assert!(
            actor_raw.get("commands").is_none(),
            "actor.yaml must not carry a `commands:` key — `ui.inspect` is \
             expected to come from the cross-cutting auto-emit pass, not a \
             per-entity opt-in"
        );

        let (registry, impls, fields, ui) = setup();
        let scope = vec!["actor:alice".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);

        let inspect = cmds
            .iter()
            .find(|c| c.id == "ui.inspect")
            .unwrap_or_else(|| {
                panic!(
                    "ui.inspect must auto-emit on scope [actor:alice] without \
                     a per-entity opt-in; got commands: {:?}",
                    cmds.iter().map(|c| (&c.id, &c.target)).collect::<Vec<_>>()
                )
            });
        assert_eq!(
            inspect.target.as_deref(),
            Some("actor:alice"),
            "ui.inspect target must equal the actor moniker, got: {:?}",
            inspect.target
        );
        assert!(
            inspect.context_menu,
            "ui.inspect must opt into the context menu for an actor scope"
        );
        assert!(
            inspect.available,
            "ui.inspect must be available for an actor scope — \
             first_inspectable + ctx.target both qualify"
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

    /// Right-click on a specific view button must surface exactly one
    /// `view.switch:{id}` command — the one whose moniker is in the scope
    /// chain. The other views' switch commands stay palette-only and must be
    /// filtered out by `context_menu_only`.
    #[test]
    fn view_switch_context_menu_only_emits_in_scope_view() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["view:board-view".into()];
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
            true, // context_menu_only
            Some(&dynamic),
        );
        let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();

        assert!(
            ids.contains(&"view.switch:board-view"),
            "in-scope view.switch should appear in right-click menu: {:?}",
            ids
        );
        assert!(
            !ids.contains(&"view.switch:tasks-grid"),
            "out-of-scope view.switch must NOT appear in right-click menu: {:?}",
            ids
        );
        assert!(
            !ids.contains(&"view.switch:tags-grid"),
            "out-of-scope view.switch must NOT appear in right-click menu: {:?}",
            ids
        );
    }

    /// Palette behavior (`context_menu_only == false`) must be unchanged:
    /// every `view.switch:{id}` still appears regardless of which view
    /// moniker is in the scope chain. Guards against a regression where the
    /// per-view scope filter accidentally suppresses palette entries.
    #[test]
    fn view_switch_palette_still_emits_all_views() {
        let (registry, impls, fields, ui) = setup();
        // No view:* in scope — palette shouldn't care either way.
        let scope: Vec<String> = vec![];
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
            false, // context_menu_only == false → palette
            Some(&dynamic),
        );
        let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();

        assert!(
            ids.contains(&"view.switch:board-view"),
            "palette must show every view.switch: {:?}",
            ids
        );
        assert!(
            ids.contains(&"view.switch:tasks-grid"),
            "palette must show every view.switch: {:?}",
            ids
        );
        assert!(
            ids.contains(&"view.switch:tags-grid"),
            "palette must show every view.switch: {:?}",
            ids
        );
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
        let scope = vec!["view:tasks-grid".into(), "board:my-board".into()];
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
        assert!(
            add_cmd.context_menu,
            "entity.add must opt into context menu"
        );
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
        let scope = vec!["view:tags-grid".into(), "board:my-board".into()];
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
        let scope = vec![
            "view:01JMVIEW0000000000BOARD0".into(),
            "board:my-board".into(),
        ];
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
        let add_cmd = cmds.iter().find(|c| c.id == "entity.add:task").expect(
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
        let scope = vec![
            "view:01JMVIEW0000000000PGRID0".into(),
            "board:my-board".into(),
        ];
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
        let add_cmd = cmds.iter().find(|c| c.id == "entity.add:project").expect(
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
        let scope = vec!["view:dashboard".into(), "board:my-board".into()];
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
    fn perspective_mutation_commands_available_from_palette_scope() {
        // Perspective mutation commands (filter, group, sort) must be
        // available from the palette (no perspective moniker in scope) so
        // keybindings and command palette invocations can succeed. The
        // `resolve_perspective_id` helper resolves the target perspective
        // at execute time via UIState/first-perspective fallback.
        let (registry, impls, fields, ui) = setup();
        let scope = vec![
            "task:01X".into(),
            "column:todo".into(),
            "board:my-board".into(),
        ];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();

        for id in [
            "perspective.filter",
            "perspective.clearFilter",
            "perspective.group",
            "perspective.clearGroup",
            "perspective.sort.set",
            "perspective.sort.clear",
            "perspective.sort.toggle",
        ] {
            assert!(
                ids.contains(&id),
                "{id} should be available without perspective in scope (resolved via UIState at execute time): {ids:?}",
            );
        }
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

        // The semantic claim is that attachment-group commands (anything
        // resolved against the inner `attachment:*` moniker) precede
        // task-group commands (resolved against the outer `task:*` moniker).
        // Match on the resolved `group` field — relying on id prefixes
        // breaks the moment a cross-cutting command (e.g. `ui.inspect`,
        // `entity.archive`) gets emitted with an attachment target.
        let open_pos = ids.iter().position(|&id| id == "attachment.open").unwrap();
        let reveal_pos = ids
            .iter()
            .position(|&id| id == "attachment.reveal")
            .unwrap();

        let first_task_pos = cmds.iter().position(|c| c.group == "task");

        if let Some(task_pos) = first_task_pos {
            assert!(
                open_pos < task_pos,
                "attachment.open (pos {open_pos}) should appear before first task-group command (pos {task_pos})"
            );
            assert!(
                reveal_pos < task_pos,
                "attachment.reveal (pos {reveal_pos}) should appear before first task-group command (pos {task_pos})"
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

    /// Regression guard: after the unified-creation refactor, no entity
    /// schema should declare `task.add`. If one slips back in, this test
    /// fails because the resolved commands contain the legacy id.
    #[test]
    fn task_add_not_emitted_from_entity_schema() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["column:todo".into(), "board:board".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
        assert!(
            cmds.iter().all(|c| c.id != "task.add"),
            "task.add must not be emitted by any path; entity schema duplicates are banned. \
             got: {:?}",
            cmds.iter().map(|c| &c.id).collect::<Vec<_>>()
        );
    }

    /// `entity.archive` is a cross-cutting command and must surface on any
    /// non-task entity scope. With the registry scope pin (`scope: "entity:task"`)
    /// stripped from `entity.yaml`, archive should appear with `available: true`
    /// when a tag moniker is in scope — proving the cross-cutting contract holds
    /// independent of any per-entity schema duplication.
    ///
    /// The cross-cutting pass supplies the resolved command with
    /// `target: Some("tag:01X")` — locking in that cross-cutting commands
    /// reach every entity moniker without needing a per-entity YAML opt-in.
    #[test]
    fn entity_archive_surfaces_on_non_task_entity() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["tag:01X".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);

        let archive = cmds
            .iter()
            .find(|c| c.id == "entity.archive" && c.available)
            .unwrap_or_else(|| {
                panic!(
                    "entity.archive should surface as available on a tag scope; \
                     got: {:?}",
                    cmds.iter()
                        .map(|c| (&c.id, c.available))
                        .collect::<Vec<_>>()
                )
            });
        assert!(
            archive.available,
            "entity.archive must be available on tag scope"
        );
    }

    /// `entity.delete` is a cross-cutting command — it auto-emits per entity
    /// moniker via `emit_cross_cutting_commands`. With `project.delete`
    /// stripped from `project.yaml`, the project's right-click menu still gets
    /// a Delete entry through the registry-driven auto-emit path.
    ///
    /// This locks in the contract that purging the per-entity opt-in does not
    /// regress the user-facing Delete affordance — the cross-cutting pass
    /// supplies an `entity.delete` resolved command with `target ==
    /// "project:backend"` and `available: true` for any project moniker in
    /// scope.
    #[test]
    fn entity_delete_surfaces_on_project_via_autoemit() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["project:backend".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);

        let delete = cmds
            .iter()
            .find(|c| c.id == "entity.delete")
            .unwrap_or_else(|| {
                panic!(
                    "entity.delete must auto-emit on project scope; got: {:?}",
                    cmds.iter()
                        .map(|c| (&c.id, &c.target, c.available))
                        .collect::<Vec<_>>()
                )
            });
        assert_eq!(
            delete.target.as_deref(),
            Some("project:backend"),
            "entity.delete target must equal the project moniker, got: {:?}",
            delete.target
        );
        assert!(
            delete.context_menu,
            "entity.delete must opt into the context menu on a project scope"
        );
        assert!(
            delete.available,
            "entity.delete must be available on a project scope"
        );
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
        let cmds = commands_for_scope(
            &scope,
            &registry,
            &impls,
            Some(&fields),
            &ui,
            false,
            Some(&dynamic),
        );
        let add = cmds.iter().find(|c| c.id == "entity.add:task").expect(
            "entity.add:task must be emitted on the tasks-grid scope chain using the REAL \
                 view registry — this is the regression guard against YAML drift, not the \
                 hand-constructed test above",
        );
        assert_eq!(add.name, "New Task");
        assert!(
            add.context_menu,
            "entity.add:task must opt into context menu"
        );
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
        let cmds = commands_for_scope(
            &scope,
            &registry,
            &impls,
            Some(&fields),
            &ui,
            false,
            Some(&dynamic),
        );
        let add = cmds.iter().find(|c| c.id == "entity.add:tag").expect(
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
        let cmds = commands_for_scope(
            &scope,
            &registry,
            &impls,
            Some(&fields),
            &ui,
            false,
            Some(&dynamic),
        );
        let add = cmds.iter().find(|c| c.id == "entity.add:project").expect(
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
            .filter(|v| v.entity_type.as_deref().is_some_and(|s| !s.is_empty()))
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
            let palette = commands_for_scope(
                &scope,
                &registry,
                &impls,
                Some(&fields),
                &ui,
                false,
                Some(&dynamic),
            );
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
            let menu = commands_for_scope(
                &scope,
                &registry,
                &impls,
                Some(&fields),
                &ui,
                true,
                Some(&dynamic),
            );
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

    // =========================================================================
    // Cross-cutting emission pass — surfaces target-driven commands on every
    // entity moniker without per-type opt-in.
    // =========================================================================

    /// `ui.inspect` is the pilot cross-cutting command after migration.
    /// Its primary param is `from: target` and it has no scope pin, so the
    /// dispatcher must surface it on every entity moniker — task, tag,
    /// project, column, board, actor — with `target == moniker`.
    ///
    /// This is the TDD anchor for the cross-cutting pass: until the pass
    /// exists AND the entity schemas drop their `ui.inspect` opt-ins, this
    /// test fails.
    #[test]
    fn ui_inspect_auto_emits_on_every_entity_type() {
        let (registry, impls, fields, ui) = setup();
        let monikers = [
            "task:01X",
            "tag:01T",
            "project:backend",
            "column:todo",
            "board:main",
            "actor:alice",
        ];
        for moniker in monikers {
            let scope = vec![moniker.to_string()];
            let cmds =
                commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);
            let inspect = cmds
                .iter()
                .find(|c| c.id == "ui.inspect")
                .unwrap_or_else(|| {
                    panic!(
                        "ui.inspect must auto-emit on scope [{moniker}]; got commands: {:?}",
                        cmds.iter().map(|c| (&c.id, &c.target)).collect::<Vec<_>>()
                    )
                });
            assert_eq!(
                inspect.target.as_deref(),
                Some(moniker),
                "ui.inspect target must equal the moniker for scope [{moniker}], got: {:?}",
                inspect.target
            );
            assert!(
                inspect.context_menu,
                "ui.inspect must opt into the context menu for scope [{moniker}]"
            );
            assert!(
                inspect.available,
                "ui.inspect must be available for scope [{moniker}] — \
                 first_inspectable + ctx.target both qualify"
            );
        }
    }

    /// The cross-cutting pass shares the `(id, target)` seen-set with the
    /// other emit_* helpers, so a multi-moniker scope chain produces exactly
    /// one resolved command per `(id, target)` tuple. `ui.inspect` walking
    /// `task → column → board` should emit three resolved commands (one per
    /// distinct target) but never duplicate any single target.
    #[test]
    fn cross_cutting_dedupes_per_target() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["task:01X".into(), "column:todo".into(), "board:main".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);

        // dedupe_by_id (the final pass) keeps only the innermost emission per
        // command id — so ui.inspect appears exactly once with the innermost
        // (task) target. The cross-cutting pass already prevented per-target
        // duplication; the global dedupe collapses across-target duplicates.
        let inspect_cmds: Vec<_> = cmds.iter().filter(|c| c.id == "ui.inspect").collect();
        assert_eq!(
            inspect_cmds.len(),
            1,
            "ui.inspect should appear exactly once after dedup, got {}: {:?}",
            inspect_cmds.len(),
            inspect_cmds.iter().map(|c| &c.target).collect::<Vec<_>>()
        );
        assert_eq!(
            inspect_cmds[0].target.as_deref(),
            Some("task:01X"),
            "innermost (task) should win the dedup"
        );
    }

    /// A Rust `Command::available()` impl is the final opt-out: even when a
    /// command's YAML declaration qualifies it as cross-cutting (no scope pin,
    /// `from: target` primary param), an impl that returns `false` for a
    /// given moniker type causes the resolved command to be filtered out by
    /// `commands_for_scope`. This guards the contract that commands like
    /// `entity.archive` can reject attachments via Rust without YAML drift.
    #[test]
    fn cross_cutting_respects_available_opt_out() {
        // Stub: a cross-cutting command (`from: target`, no scope pin) that
        // declares it is unavailable for tag monikers but available for tasks.
        struct OptOutCmd;
        #[async_trait::async_trait]
        impl Command for OptOutCmd {
            fn available(&self, ctx: &CommandContext) -> bool {
                ctx.target
                    .as_deref()
                    .and_then(|m| m.split_once(':').map(|(t, _)| t))
                    .is_some_and(|t| t != "tag")
            }
            async fn execute(
                &self,
                _ctx: &CommandContext,
            ) -> swissarmyhammer_commands::Result<serde_json::Value> {
                Ok(serde_json::Value::Null)
            }
        }

        // Build a registry with a single cross-cutting command alongside the
        // builtins so the lookup paths are exercised against a realistic mix.
        let stub_yaml = r#"
- id: stub.opt_out
  name: "Opt Out {{entity.type}}"
  context_menu: true
  params:
    - name: moniker
      from: target
"#;
        let mut sources = swissarmyhammer_commands::builtin_yaml_sources();
        sources.push(("stub_opt_out", stub_yaml));
        let registry = CommandsRegistry::from_yaml_sources(&sources);
        let mut impls = crate::commands::register_commands();
        impls.insert("stub.opt_out".to_string(), Arc::new(OptOutCmd));

        let defs = crate::defaults::builtin_field_definitions();
        let entities = crate::defaults::builtin_entity_definitions();
        let fields = FieldsContext::from_yaml_sources(
            std::path::PathBuf::from("/tmp/test"),
            &defs,
            &entities,
        )
        .unwrap();
        let ui = Arc::new(UIState::new());

        // Tag scope — opt-out fires, no resolved command.
        let tag_scope = vec!["tag:bug".to_string()];
        let tag_cmds = commands_for_scope(
            &tag_scope,
            &registry,
            &impls,
            Some(&fields),
            &ui,
            false,
            None,
        );
        assert!(
            !tag_cmds.iter().any(|c| c.id == "stub.opt_out"),
            "stub.opt_out must be filtered out for tag scope (Command::available returned false), \
             got: {:?}",
            tag_cmds.iter().map(|c| &c.id).collect::<Vec<_>>()
        );

        // Task scope — opt-out passes, command surfaces with the task target.
        let task_scope = vec!["task:01X".to_string()];
        let task_cmds = commands_for_scope(
            &task_scope,
            &registry,
            &impls,
            Some(&fields),
            &ui,
            false,
            None,
        );
        let stub = task_cmds
            .iter()
            .find(|c| c.id == "stub.opt_out")
            .unwrap_or_else(|| {
                panic!(
                    "stub.opt_out must surface for task scope; got: {:?}",
                    task_cmds.iter().map(|c| &c.id).collect::<Vec<_>>()
                )
            });
        assert_eq!(stub.target.as_deref(), Some("task:01X"));
    }

    /// The cross-cutting pass honors a `entity_type` constraint declared on the
    /// target param (Rule 3 of `emit_cross_cutting_commands`): a command with
    /// `params: [{name: moniker, from: target, entity_type: task}]` must emit
    /// only on monikers whose type matches `task`, even though it otherwise
    /// qualifies as cross-cutting (no scope pin, target-primary param).
    ///
    /// Regression guard: removing the Rule 3 filter would let the stub emit on
    /// every entity moniker (including `tag:01T`), failing the second assert.
    #[test]
    fn cross_cutting_respects_target_entity_type_constraint() {
        // Stub: cross-cutting command (no scope pin, `from: target`) that
        // pins its target param to entity_type=task. Cross-cutting Rule 3
        // must filter it out for non-task monikers.
        let stub_yaml = r#"
- id: stub.task_only
  name: "Task Only {{entity.type}}"
  context_menu: true
  params:
    - name: moniker
      from: target
      entity_type: task
"#;
        let mut sources = swissarmyhammer_commands::builtin_yaml_sources();
        sources.push(("stub_task_only", stub_yaml));
        let registry = CommandsRegistry::from_yaml_sources(&sources);
        let impls = crate::commands::register_commands();

        let defs = crate::defaults::builtin_field_definitions();
        let entities = crate::defaults::builtin_entity_definitions();
        let fields = FieldsContext::from_yaml_sources(
            std::path::PathBuf::from("/tmp/test"),
            &defs,
            &entities,
        )
        .unwrap();
        let ui = Arc::new(UIState::new());

        // Scope chain contains both a task and a tag moniker. The stub must
        // emit on the task moniker and be filtered on the tag moniker.
        let scope = vec!["task:01X".to_string(), "tag:01T".to_string()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);

        let stub_emissions: Vec<&ResolvedCommand> =
            cmds.iter().filter(|c| c.id == "stub.task_only").collect();

        assert!(
            stub_emissions
                .iter()
                .any(|c| c.target.as_deref() == Some("task:01X")),
            "stub.task_only must emit with target=task:01X (entity_type=task matches); \
             got emissions: {:?}",
            stub_emissions.iter().map(|c| &c.target).collect::<Vec<_>>()
        );
        assert!(
            !stub_emissions
                .iter()
                .any(|c| c.target.as_deref() == Some("tag:01T")),
            "stub.task_only must NOT emit with target=tag:01T (entity_type=task constraint \
             rejects tag monikers); got emissions: {:?}",
            stub_emissions.iter().map(|c| &c.target).collect::<Vec<_>>()
        );
    }

    // =========================================================================
    // YAML hygiene
    // =========================================================================

    /// IDs that are declared once in `swissarmyhammer-commands/builtin/commands/`
    /// (in `entity.yaml` or `ui.yaml`) and auto-emit per entity moniker via the
    /// scope_commands dispatcher. They MUST NOT appear in any per-entity
    /// schema (`swissarmyhammer-kanban/builtin/entities/*.yaml`).
    ///
    /// See the rule-comment header at the top of
    /// `swissarmyhammer-commands/builtin/commands/entity.yaml` and
    /// `feedback_command_organization.md` in the project memory.
    /// Hygiene guard: entity schemas must not carry a `commands:` key at all.
    ///
    /// Post-retirement of `EntityDef.commands`, the type-specific command
    /// declarations live in `swissarmyhammer-commands/builtin/commands/*.yaml`
    /// and cross-cutting ones auto-emit from the registry per entity moniker.
    /// Entity schemas under `swissarmyhammer-kanban/builtin/entities/*.yaml`
    /// describe fields only. Re-introducing a `commands:` key would bring
    /// back the duplicate-overlay pattern we deleted.
    ///
    /// This test scans every builtin entity YAML and fails if any of them
    /// carries a `commands:` key — stricter than the original which only
    /// flagged cross-cutting ids.
    #[test]
    fn yaml_hygiene_entity_schemas_have_no_commands_key() {
        let mut violations: Vec<String> = Vec::new();

        for (entity_name, yaml) in builtin_entity_definitions() {
            let raw: serde_yaml_ng::Value = serde_yaml_ng::from_str(yaml)
                .unwrap_or_else(|e| panic!("failed to parse builtin entity '{entity_name}': {e}"));
            if raw.get("commands").is_some() {
                violations.push(entity_name.to_string());
            }
        }

        assert!(
            violations.is_empty(),
            "Entity schemas must not carry a `commands:` key — type-specific \
             commands live in `swissarmyhammer-commands/builtin/commands/<noun>.yaml` \
             and cross-cutting commands auto-emit from the registry. \
             Found `commands:` on: {}. \
             See `feedback_command_organization.md` in project memory.",
            violations.join(", ")
        );
    }

    /// `emit_cross_cutting_commands` keys off `ParamSource::Target` on the
    /// FIRST param to decide whether a registry command should auto-emit per
    /// entity moniker in scope. Only `from: target` qualifies — `from: args`
    /// and `from: scope_chain` must not. This guard pins that contract: if a
    /// future refactor loosened the check (e.g. accepted "any param is target",
    /// or treated `scope_chain` as equivalent to `target`), the cross-cutting
    /// pass would silently surface commands whose primary value comes from the
    /// caller (args) or the scope walk (scope_chain), producing wrong context
    /// menu entries with a per-entity target the command was never designed to
    /// receive.
    ///
    /// Both stubs are registered without a `Command` impl, so `check_available`
    /// returns `true` by default — the assertion is purely about whether the
    /// cross-cutting pass *emits* the command with a task target, independent
    /// of the availability gate. The stubs may still surface from the
    /// global/scoped registry passes with `target: None`; the assertion
    /// narrows on `(id, target == Some("task:01X"))` so those unrelated
    /// emissions don't mask the regression.
    #[test]
    fn cross_cutting_ignores_from_args_commands() {
        // Two stubs, both context_menu commands with a single primary param,
        // distinguished only by `from:`. Neither uses `from: target`, so
        // neither should be picked up by the cross-cutting pass.
        let stub_yaml = r#"
- id: stub.from_args
  name: "From Args {{entity.type}}"
  context_menu: true
  params:
    - name: moniker
      from: args
- id: stub.from_scope_chain
  name: "From Scope Chain {{entity.type}}"
  context_menu: true
  params:
    - name: moniker
      from: scope_chain
"#;
        let mut sources = swissarmyhammer_commands::builtin_yaml_sources();
        sources.push(("stub_cross_cutting_non_target", stub_yaml));
        let registry = CommandsRegistry::from_yaml_sources(&sources);
        let impls = crate::commands::register_commands();

        let defs = crate::defaults::builtin_field_definitions();
        let entities = crate::defaults::builtin_entity_definitions();
        let fields = FieldsContext::from_yaml_sources(
            std::path::PathBuf::from("/tmp/test"),
            &defs,
            &entities,
        )
        .unwrap();
        let ui = Arc::new(UIState::new());

        // Task moniker in scope — the cross-cutting pass would, for a
        // qualifying command, emit it with target == Some("task:01X").
        let scope = vec![
            "task:01X".to_string(),
            "column:todo".to_string(),
            "board:main".to_string(),
        ];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false, None);

        // Assert: neither stub appears with a task target. (They may still
        // surface with `target: None` from the global registry pass — that's
        // unrelated to the cross-cutting contract under test.)
        let from_args_with_task_target: Vec<_> = cmds
            .iter()
            .filter(|c| c.id == "stub.from_args" && c.target.as_deref() == Some("task:01X"))
            .collect();
        assert!(
            from_args_with_task_target.is_empty(),
            "stub.from_args (primary param `from: args`) must NOT be emitted \
             by the cross-cutting pass with a task target — only `from: target` \
             qualifies a command as cross-cutting; got: {:?}",
            from_args_with_task_target
                .iter()
                .map(|c| (&c.id, &c.target))
                .collect::<Vec<_>>()
        );

        let from_scope_chain_with_task_target: Vec<_> = cmds
            .iter()
            .filter(|c| c.id == "stub.from_scope_chain" && c.target.as_deref() == Some("task:01X"))
            .collect();
        assert!(
            from_scope_chain_with_task_target.is_empty(),
            "stub.from_scope_chain (primary param `from: scope_chain`) must \
             NOT be emitted by the cross-cutting pass with a task target — \
             only `from: target` qualifies a command as cross-cutting; got: {:?}",
            from_scope_chain_with_task_target
                .iter()
                .map(|c| (&c.id, &c.target))
                .collect::<Vec<_>>()
        );
    }

    // =========================================================================
    // Context-menu ordering and grouping
    // =========================================================================

    /// Right-clicking a task must produce the cross-cutting commands in a
    /// stable, grouped order with distinct `group` strings that trigger
    /// separator insertion in the frontend renderer:
    ///
    ///   1. Cut / Copy / Paste    (group ctx1)
    ///   2. Delete / Archive      (group ctx2)
    ///   3. Inspect               (group ctx3)
    ///
    /// The frontend renderer at `context-menu.ts` inserts a separator
    /// whenever `cmd.group !== lastGroup`, so three distinct group strings
    /// yield the two user-visible separators the design calls for.
    #[test]
    fn cross_cutting_context_menu_is_ordered_and_grouped() {
        let (registry, impls, fields, ui) = setup();
        // Put a task on the clipboard so `entity.paste` is available on a task
        // scope (PasteEntityCmd validates clipboard-vs-target compatibility).
        ui.set_clipboard_entity_type("tag");
        let scope = vec!["task:01X".into(), "column:todo".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, true, None);

        // Filter down to the cross-cutting entries we care about, in the order
        // `commands_for_scope` emitted them.
        let cross_cutting: Vec<&ResolvedCommand> = cmds
            .iter()
            .filter(|c| {
                matches!(
                    c.id.as_str(),
                    "entity.cut"
                        | "entity.copy"
                        | "entity.paste"
                        | "entity.delete"
                        | "entity.archive"
                        | "entity.unarchive"
                        | "ui.inspect"
                )
            })
            .collect();
        let ids: Vec<&str> = cross_cutting.iter().map(|c| c.id.as_str()).collect();

        // Expected order. `entity.unarchive` is unavailable on a todo-column
        // task (nothing to unarchive) so it's filtered out by `available`.
        let expected: &[&str] = &[
            "entity.cut",
            "entity.copy",
            "entity.paste",
            "entity.delete",
            "entity.archive",
            "ui.inspect",
        ];
        assert_eq!(
            ids, expected,
            "cross-cutting context-menu commands must appear in the documented \
             order (cut/copy/paste → delete/archive → inspect); got {:?}",
            ids
        );

        // Group strings must partition the list into three contiguous buckets
        // so the frontend separator logic (new group → separator) triggers at
        // the right spots. All buckets are per-entity-type-suffixed so
        // `dedupe_by_id` across monikers still sees them as the same command.
        let group_of = |id: &str| -> &str {
            cross_cutting
                .iter()
                .find(|c| c.id == id)
                .expect("command in filtered list")
                .group
                .as_str()
        };
        let g_cut = group_of("entity.cut");
        let g_copy = group_of("entity.copy");
        let g_paste = group_of("entity.paste");
        let g_delete = group_of("entity.delete");
        let g_archive = group_of("entity.archive");
        let g_inspect = group_of("ui.inspect");

        assert_eq!(
            g_cut, g_copy,
            "cut and copy must share a group to render contiguously"
        );
        assert_eq!(
            g_copy, g_paste,
            "copy and paste must share a group to render contiguously"
        );
        assert_eq!(
            g_delete, g_archive,
            "delete and archive must share a group to render contiguously"
        );
        assert_ne!(
            g_paste, g_delete,
            "cut/copy/paste group must differ from delete/archive group so a \
             separator appears between them"
        );
        assert_ne!(
            g_archive, g_inspect,
            "delete/archive group must differ from inspect group so a \
             separator appears between them"
        );
        assert!(
            g_cut.contains("ctx1"),
            "cut/copy/paste bucket should be tagged ctx1; got {:?}",
            g_cut
        );
        assert!(
            g_delete.contains("ctx2"),
            "delete/archive bucket should be tagged ctx2; got {:?}",
            g_delete
        );
        assert!(
            g_inspect.contains("ctx3"),
            "inspect bucket should be tagged ctx3; got {:?}",
            g_inspect
        );
    }

    /// Two back-to-back calls to `commands_for_scope` must return the
    /// cross-cutting context-menu commands in the exact same order.
    ///
    /// The registry is backed by a `HashMap<String, CommandDef>`, and Rust's
    /// `DefaultHasher` reseeds per process — so iteration order is stable
    /// within one process run but different across runs. Running twice in one
    /// test guards the *intra-process* invariant, which is what matters for a
    /// single UI session: the menu doesn't reshuffle when you right-click a
    /// second time.
    #[test]
    fn cross_cutting_order_is_stable_across_runs() {
        let (registry, impls, fields, ui) = setup();
        ui.set_clipboard_entity_type("tag");
        let scope = vec!["task:01X".into(), "column:todo".into()];

        let extract = || -> Vec<String> {
            commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, true, None)
                .into_iter()
                .filter(|c| {
                    matches!(
                        c.id.as_str(),
                        "entity.cut"
                            | "entity.copy"
                            | "entity.paste"
                            | "entity.delete"
                            | "entity.archive"
                            | "entity.unarchive"
                            | "ui.inspect"
                    )
                })
                .map(|c| c.id)
                .collect()
        };

        let first = extract();
        let second = extract();
        assert_eq!(
            first, second,
            "cross-cutting command order must be deterministic — HashMap \
             iteration order must not leak into the emission sequence"
        );
    }

    // =========================================================================
    // Menu-bar dedupe
    // =========================================================================

    /// Build a synthetic `ResolvedCommand` carrying just the fields the
    /// menu-bar dedupe helper inspects. Mirrors what
    /// `emit_cross_cutting_commands` would produce for a given (id, target)
    /// pair before the global `dedupe_by_id` pass collapses them.
    fn make_resolved(id: &str, target: &str) -> ResolvedCommand {
        ResolvedCommand {
            id: id.into(),
            name: format!("Cmd {id} on {target}"),
            menu_name: None,
            target: Some(target.into()),
            group: target
                .split_once(':')
                .map(|(t, _)| t.to_string())
                .unwrap_or_default(),
            context_menu: true,
            keys: None,
            available: true,
        }
    }

    /// `dedupe_for_menu_bar` collapses per-target emissions of the same
    /// cross-cutting command id (e.g. `entity.copy` once per moniker in a
    /// `[tag, task, column]` scope) down to a single menu-bar row. The raw
    /// per-target list is what `emit_cross_cutting_commands` produces before
    /// `commands_for_scope`'s final `dedupe_by_id` pass — exactly what a
    /// menu-bar caller would receive if it bypassed the inner dedupe to keep
    /// per-target context-menu entries.
    #[test]
    fn menu_bar_dedupes_cross_cutting_commands() {
        // Simulate the cross-cutting pass output for a `[tag, task, column]`
        // scope: entity.copy emitted once per entity moniker, innermost first.
        let mut menu_bar = vec![
            make_resolved("entity.copy", "tag:01T"),
            make_resolved("entity.copy", "task:01X"),
            make_resolved("entity.copy", "column:todo"),
        ];

        // Pre-dedupe: the raw cross-cutting stream carries one entity.copy per
        // target — that's what a context-menu renderer wants. (This mirrors the
        // task acceptance criterion: "context-menu output contains it three
        // times, one per target".)
        let copies_before: Vec<&ResolvedCommand> =
            menu_bar.iter().filter(|c| c.id == "entity.copy").collect();
        assert_eq!(
            copies_before.len(),
            3,
            "raw cross-cutting stream should carry one entity.copy per moniker, \
             got: {:?}",
            copies_before.iter().map(|c| &c.target).collect::<Vec<_>>()
        );

        // Apply the menu-bar dedupe: collapse to a single row per id.
        dedupe_for_menu_bar(&mut menu_bar);

        let copies_after: Vec<&ResolvedCommand> =
            menu_bar.iter().filter(|c| c.id == "entity.copy").collect();
        assert_eq!(
            copies_after.len(),
            1,
            "menu-bar dedupe must leave entity.copy exactly once regardless of \
             how many entity monikers were in scope, got: {:?}",
            copies_after.iter().map(|c| &c.target).collect::<Vec<_>>()
        );
    }

    /// The menu-bar dedupe must keep the **innermost** target so that picking
    /// Edit → Cut from the macOS menu bar dispatches to the most-specific
    /// entity in the current scope (matching what the user would right-click
    /// on). `commands_for_scope` emits monikers innermost-first, so retaining
    /// the first occurrence per id satisfies this contract.
    #[test]
    fn menu_bar_entry_targets_innermost() {
        // Same `[tag, task, column]` scope, this time also varying the command
        // id so the assertion narrows on the innermost target for entity.copy
        // without picking up unrelated entries.
        let mut menu_bar = vec![
            make_resolved("entity.copy", "tag:01T"),
            make_resolved("entity.copy", "task:01X"),
            make_resolved("entity.copy", "column:todo"),
            make_resolved("entity.cut", "tag:01T"),
            make_resolved("entity.cut", "task:01X"),
            make_resolved("entity.cut", "column:todo"),
        ];

        dedupe_for_menu_bar(&mut menu_bar);

        let copy = menu_bar
            .iter()
            .find(|c| c.id == "entity.copy")
            .expect("entity.copy must survive menu-bar dedupe");
        assert_eq!(
            copy.target.as_deref(),
            Some("tag:01T"),
            "menu-bar entry for entity.copy must dispatch to the innermost \
             target (tag), got: {:?}",
            copy.target
        );

        let cut = menu_bar
            .iter()
            .find(|c| c.id == "entity.cut")
            .expect("entity.cut must survive menu-bar dedupe");
        assert_eq!(
            cut.target.as_deref(),
            Some("tag:01T"),
            "menu-bar entry for entity.cut must dispatch to the innermost \
             target (tag), got: {:?}",
            cut.target
        );
    }

    /// A list that already has at most one entry per id is a no-op for the
    /// menu-bar dedupe — the helper must not reorder or drop entries that
    /// don't share an id. This guards against accidentally narrowing the
    /// dedupe key beyond `id` (e.g. keying on `(id, target)` would still
    /// retain everything but break the cross-cutting dedupe contract above).
    #[test]
    fn menu_bar_dedupe_is_noop_on_already_unique_list() {
        let mut menu_bar = vec![
            make_resolved("entity.copy", "tag:01T"),
            make_resolved("entity.cut", "tag:01T"),
            make_resolved("entity.paste", "column:todo"),
            make_resolved("ui.inspect", "task:01X"),
        ];
        let before = menu_bar.clone();

        dedupe_for_menu_bar(&mut menu_bar);

        assert_eq!(
            menu_bar.len(),
            before.len(),
            "dedupe_for_menu_bar must be a no-op on a list with no duplicate ids"
        );
        for (after, before) in menu_bar.iter().zip(before.iter()) {
            assert_eq!(after.id, before.id, "order must be preserved");
            assert_eq!(after.target, before.target, "target must be preserved");
        }
    }
}
