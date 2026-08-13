//! The name templates, such as `{{entity.type}}`.
//!
//! These tests hold `resolve_name_template` to each variable that it must
//! replace. They also hold the dynamic board rows to the same templates.

use super::*;

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
        None,
    );
    let board_cmd = cmds
        .iter()
        .find(|c| c.id.starts_with("board.switch:"))
        .expect("should have board switch command");

    assert_eq!(board_cmd.name, "Switch to Board: my-project (my-project)");
    assert_eq!(board_cmd.menu_name.as_deref(), Some("my-project"));
}
