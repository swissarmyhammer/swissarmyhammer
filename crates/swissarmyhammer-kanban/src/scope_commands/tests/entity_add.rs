//! The `entity.add` row.
//!
//! The first tests give `emit_entity_add` hand-built `ViewInfo` entries. They
//! show that the algorithm is correct. The tests that follow load the real
//! builtin view YAML. They show that the projection from YAML to `ViewInfo`
//! keeps the `entity_type`. A defect in only the projection fails the second
//! set and not the first.

use super::*;

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
            kind: "grid".into(),
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
        None,
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
            kind: "grid".into(),
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
        None,
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
    // Unlike the view / board / perspective / window-focus navigation
    // rows (all context_menu: false), entity.add:* is a first-class
    // creation action and IS present with context_menu_only=true.
    let (registry, impls, fields, ui) = setup();
    let scope = vec!["view:tags-grid".into(), "board:my-board".into()];
    let dynamic = DynamicSources {
        views: vec![ViewInfo {
            id: "tags-grid".into(),
            name: "Tag Grid".into(),
            entity_type: Some("tag".into()),
            kind: "grid".into(),
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
        None,
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
            kind: "board".into(),
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
        None,
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
            kind: "grid".into(),
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
        None,
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
            kind: "unknown".into(),
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
        None,
    );
    assert!(
        !cmds.iter().any(|c| c.id.starts_with("entity.add:")),
        "view without entity_type must not emit entity.add"
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
    let vctx =
        swissarmyhammer_views::ViewsContext::from_yaml_sources(temp.path().to_path_buf(), &builtin)
            .expect("builtin views must parse");
    vctx.all_views()
        .iter()
        .map(|v| ViewInfo {
            id: v.id.clone(),
            name: v.name.clone(),
            entity_type: v.entity_type.clone(),
            kind: v.kind.as_kebab_str().to_string(),
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
        None,
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
        None,
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
        None,
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
            None,
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
            None,
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
