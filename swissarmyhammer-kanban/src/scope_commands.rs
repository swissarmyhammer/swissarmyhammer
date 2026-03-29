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

/// A fully resolved command ready for display in a menu, palette, or context menu.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ResolvedCommand {
    /// Command ID (e.g. "entity.copy").
    pub id: String,
    /// Fully resolved display name (e.g. "Copy Tag", never "Copy {{entity.type}}").
    pub name: String,
    /// Target moniker (e.g. "tag:01X") or None for global commands.
    pub target: Option<String>,
    /// Whether this command should appear in context menus.
    pub context_menu: bool,
    /// Keybindings per keymap mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keys: Option<KeysDef>,
    /// Whether the command is currently available (enabled).
    pub available: bool,
}

/// Resolve `{{entity.type}}` in a command name.
fn resolve_name_template(name: &str, entity_type: &str) -> String {
    if !name.contains("{{entity.type}}") {
        return name.to_string();
    }
    let capitalized = format!(
        "{}{}",
        &entity_type[..1].to_uppercase(),
        &entity_type[1..]
    );
    name.replace("{{entity.type}}", &capitalized)
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
pub fn commands_for_scope(
    scope_chain: &[String],
    registry: &CommandsRegistry,
    command_impls: &HashMap<String, Arc<dyn Command>>,
    fields: Option<&FieldsContext>,
    ui_state: &Arc<UIState>,
    context_menu_only: bool,
) -> Vec<ResolvedCommand> {
    let mut result: Vec<ResolvedCommand> = Vec::new();
    let mut seen: HashSet<(String, Option<String>)> = HashSet::new();

    let clipboard_type = ui_state.clipboard_entity_type();

    // 1. Walk scope chain: for each entity moniker, get its schema commands
    if let Some(fields) = fields {
        for moniker in scope_chain {
            let Some((entity_type, _entity_id)) = moniker.split_once(':') else {
                continue;
            };
            let Some(entity_def) = fields.get_entity(entity_type) else {
                continue;
            };

            for cmd in &entity_def.commands {
                let key = (cmd.id.clone(), Some(moniker.clone()));
                if seen.contains(&key) {
                    continue;
                }
                seen.insert(key);

                // Resolve name template
                let name = if cmd.id == "entity.paste" {
                    // Paste name comes from clipboard entity type
                    resolve_name_template(
                        &cmd.name,
                        clipboard_type.as_deref().unwrap_or("entity"),
                    )
                } else {
                    resolve_name_template(&cmd.name, entity_type)
                };

                // Convert entity command keys to registry KeysDef
                let keys = cmd.keys.as_ref().map(|k| KeysDef {
                    vim: k.vim.clone(),
                    cua: k.cua.clone(),
                    emacs: k.emacs.clone(),
                });

                let available = check_available(
                    &cmd.id,
                    scope_chain,
                    Some(moniker),
                    command_impls,
                    ui_state,
                );

                result.push(ResolvedCommand {
                    id: cmd.id.clone(),
                    name,
                    target: Some(moniker.clone()),
                    context_menu: cmd.context_menu,
                    keys,
                    available,
                });
            }
        }
    }

    // 2. Add global commands from the registry (no scope requirement)
    for cmd_def in registry.all_commands() {
        // Skip commands that are entity-scoped (handled above via schema)
        if cmd_def.scope.is_some() {
            continue;
        }
        // Skip invisible commands
        if !cmd_def.visible {
            continue;
        }

        let key = (cmd_def.id.clone(), None);
        if seen.contains(&key) {
            continue;
        }
        seen.insert(key);

        let name = resolve_name_template(
            &cmd_def.name,
            clipboard_type.as_deref().unwrap_or("entity"),
        );

        let keys = cmd_def.keys.clone();

        let available = check_available(
            &cmd_def.id,
            scope_chain,
            None,
            command_impls,
            ui_state,
        );

        result.push(ResolvedCommand {
            id: cmd_def.id.clone(),
            name,
            target: None,
            context_menu: cmd_def.context_menu,
            keys,
            available,
        });
    }

    // 3. Filter
    if context_menu_only {
        result.retain(|c| c.context_menu);
    }
    // Only return available commands
    result.retain(|c| c.available);

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defaults::{builtin_entity_definitions, builtin_field_definitions};
    use swissarmyhammer_commands::builtin_yaml_sources;

    /// Build a test harness with registry, command impls, and fields context.
    fn setup() -> (
        CommandsRegistry,
        HashMap<String, Arc<dyn Command>>,
        FieldsContext,
        Arc<UIState>,
    ) {
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
        let scope = vec!["board:my-board".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false);

        let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();
        assert!(ids.contains(&"app.undo"), "board scope should have undo");
        assert!(ids.contains(&"app.redo"), "board scope should have redo");
        assert!(!ids.contains(&"entity.copy"), "board scope should NOT have copy (no task/tag)");
        assert!(!ids.contains(&"entity.cut"), "board scope should NOT have cut");
    }

    #[test]
    fn board_scope_no_paste_without_clipboard() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec!["board:my-board".into()];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false);
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
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false);

        let paste = cmds.iter().find(|c| c.id == "entity.paste");
        assert!(paste.is_some(), "paste should be available with task on clipboard + column in scope");
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
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false);
        let names: Vec<&str> = cmds.iter().map(|c| c.name.as_str()).collect();

        assert!(names.contains(&"Copy Task"), "should have Copy Task: {:?}", names);
        assert!(names.contains(&"Cut Task"), "should have Cut Task: {:?}", names);
        assert!(names.contains(&"Inspect Task"), "should have Inspect Task: {:?}", names);
        assert!(names.contains(&"Archive Task"), "should have Archive Task: {:?}", names);
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
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false);
        let paste = cmds.iter().find(|c| c.id == "entity.paste");
        assert!(paste.is_some(), "paste should be available");
        assert_eq!(paste.unwrap().name, "Paste Tag");
    }

    // =========================================================================
    // Tag on task scope
    // =========================================================================

    #[test]
    fn tag_on_task_has_both_copy_tag_and_copy_task() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec![
            "tag:bug".into(),
            "task:01X".into(),
            "column:todo".into(),
            "board:my-board".into(),
        ];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false);
        let names: Vec<&str> = cmds.iter().map(|c| c.name.as_str()).collect();

        assert!(names.contains(&"Copy Tag"), "should have Copy Tag: {:?}", names);
        assert!(names.contains(&"Copy Task"), "should have Copy Task: {:?}", names);
        assert!(names.contains(&"Cut Tag"), "should have Cut Tag: {:?}", names);
        assert!(names.contains(&"Cut Task"), "should have Cut Task: {:?}", names);
        assert!(names.contains(&"Inspect Tag"), "should have Inspect Tag: {:?}", names);
        assert!(names.contains(&"Inspect Task"), "should have Inspect Task: {:?}", names);
    }

    #[test]
    fn tag_on_task_with_tag_clipboard_has_paste_tag() {
        let (registry, impls, fields, ui) = setup();
        ui.set_clipboard_entity_type("tag");
        let scope = vec![
            "tag:bug".into(),
            "task:01X".into(),
            "column:todo".into(),
        ];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false);
        let paste = cmds.iter().find(|c| c.id == "entity.paste");
        assert!(paste.is_some(), "should have paste");
        assert_eq!(paste.unwrap().name, "Paste Tag");
    }

    #[test]
    fn tag_on_task_no_paste_without_clipboard() {
        let (registry, impls, fields, ui) = setup();
        let scope = vec![
            "tag:bug".into(),
            "task:01X".into(),
            "column:todo".into(),
        ];
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false);
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
        let cmds = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false);
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
        let all = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, false);
        let ctx_only = commands_for_scope(&scope, &registry, &impls, Some(&fields), &ui, true);

        assert!(ctx_only.len() < all.len(), "context menu should have fewer commands");
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
        let cmds = commands_for_scope(&[], &registry, &impls, Some(&fields), &ui, false);

        // Should have global commands like undo/redo
        let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();
        assert!(ids.contains(&"app.undo"));
        // Should NOT have entity-scoped commands
        assert!(!ids.contains(&"entity.copy"));
        // All targets should be None (global)
        for cmd in &cmds {
            assert!(cmd.target.is_none(), "'{}' should have no target", cmd.id);
        }
    }
}
