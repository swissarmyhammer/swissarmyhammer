//! Integration coverage for the `commands_for_scope` options-enrichment
//! pass.
//!
//! Pins two end-to-end behaviors the rest of the
//! `command-driven-ui` epic depends on:
//!
//! 1. A synthetic command with an enum-shaped param whose
//!    `options_from` is wired to a registered resolver
//!    (`perspective.fields`) carries a populated `options` list on
//!    the emitted [`ResolvedCommand`].
//! 2. A synthetic command with `options_from` pointing at an
//!    unregistered resolver key leaves `options: None` on every
//!    emitted param — the resolver registry warns once per emission
//!    and the frontend is responsible for the "command can't be
//!    picked right now" UX.
//!
//! Each test stands up a minimal in-memory registry containing a
//! single synthetic command whose `scope` matches the test's scope
//! chain — no kanban-app/Tauri scaffolding, no on-disk state.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use swissarmyhammer_commands::{Command, CommandsRegistry};
use swissarmyhammer_common::WindowInfo;
use swissarmyhammer_ui_state::{UIState};
use swissarmyhammer_kanban::commands::options_resolvers::default_options_registry;
use swissarmyhammer_kanban::dynamic_sources::{build_dynamic_sources, DynamicSourcesInputs};
use swissarmyhammer_kanban::scope_commands::{commands_for_scope, DynamicSources, ResolvedCommand};
use swissarmyhammer_kanban::{board::InitBoard, Execute, KanbanContext};
use swissarmyhammer_perspectives::{PerspectiveFieldInfo, PerspectiveInfo};
use swissarmyhammer_views::ViewInfo;
use tempfile::TempDir;

/// Build a [`CommandsRegistry`] with a single synthetic command
/// matching the given YAML — keeps test fixtures small and explicit.
fn registry_with(yaml: &str) -> CommandsRegistry {
    CommandsRegistry::from_yaml_sources(&[("synthetic", yaml)])
}

/// Build a [`DynamicSources`] with one perspective carrying three
/// fields so the `perspective.fields` resolver has data to project.
///
/// The fixture uses schema-slug names (`"title"`, `"status"`) on the
/// `name` field because that is the wire `value` the resolver emits
/// and the key downstream consumers (`<GroupedBoardView>`,
/// `computeGroups`, persisted perspective YAMLs) expect.
fn dynamic_with_perspective() -> DynamicSources {
    DynamicSources {
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
            ],
        }],
        ..Default::default()
    }
}

/// Look up the test's synthetic command in a `commands_for_scope`
/// result. Panics with a helpful message when the command did not
/// survive emission (usually a sign that the YAML's scope didn't
/// match the test's scope chain or the view-kind filter dropped it).
fn find_cmd<'a>(cmds: &'a [ResolvedCommand], id: &str) -> &'a ResolvedCommand {
    cmds.iter().find(|c| c.id == id).unwrap_or_else(|| {
        panic!(
            "expected `{id}` in emitted commands; got: {:?}",
            cmds.iter().map(|c| &c.id).collect::<Vec<_>>()
        )
    })
}

/// A synthetic command tagged with `shape: enum,
/// options_from: "perspective.fields"` should carry the resolved
/// option list on its emitted param. This is the happy path the
/// frontend `<CommandPopover>` consumes.
#[test]
fn commands_for_scope_populates_enum_options() {
    let yaml = r#"
- id: test.pick.field
  name: Pick a field
  scope: "entity:perspective"
  visible: true
  params:
    - name: field
      from: args
      shape: enum
      options_from: "perspective.fields"
"#;
    let registry = registry_with(yaml);
    let impls: HashMap<String, Arc<dyn Command>> = HashMap::new();
    let ui_state = Arc::new(UIState::new());
    let scope = vec!["perspective:01P".to_string()];
    let dynamic = dynamic_with_perspective();
    let opts_registry = default_options_registry();
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        None,
        &ui_state,
        false,
        Some(&dynamic),
        Some(&opts_registry),
    );
    let cmd = find_cmd(&cmds, "test.pick.field");
    assert_eq!(cmd.params.len(), 1, "test.pick.field has one param");
    let param = &cmd.params[0];
    let options = param
        .options
        .as_ref()
        .expect("perspective.fields resolved → options must be Some");
    assert_eq!(
        options.len(),
        2,
        "perspective.fields resolver projects every PerspectiveFieldInfo"
    );
    // `value` carries the field NAME (slug), not the field ID, so the
    // dispatched `perspective.group` arg lines up with `task.fields[<name>]`
    // in the frontend's `computeGroups`. See task
    // `01KRH2EX1N1CA2HA3B4NMWZH67`.
    assert_eq!(options[0].value, "title");
    assert_eq!(options[0].label, "Title");
    assert_eq!(options[1].value, "status");
    assert_eq!(options[1].label, "Status");
}

/// A synthetic command with `options_from` pointing at a key that
/// is NOT registered in the resolver registry must leave
/// `options: None` on the emitted param. The frontend treats that
/// as "this command can't be picked right now" (the warn is emitted
/// via `tracing::warn!` and is not asserted on here — log capture
/// belongs to a separate test surface).
#[test]
fn commands_for_scope_leaves_options_none_for_unknown_resolver() {
    let yaml = r#"
- id: test.pick.broken
  name: Pick something (broken)
  scope: "entity:perspective"
  visible: true
  params:
    - name: thing
      from: args
      shape: enum
      options_from: "nonexistent.resolver"
"#;
    let registry = registry_with(yaml);
    let impls: HashMap<String, Arc<dyn Command>> = HashMap::new();
    let ui_state = Arc::new(UIState::new());
    let scope = vec!["perspective:01P".to_string()];
    let dynamic = dynamic_with_perspective();
    let opts_registry = default_options_registry();
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        None,
        &ui_state,
        false,
        Some(&dynamic),
        Some(&opts_registry),
    );
    let cmd = find_cmd(&cmds, "test.pick.broken");
    assert_eq!(cmd.params.len(), 1, "test.pick.broken has one param");
    let param = &cmd.params[0];
    assert!(
        param.options.is_none(),
        "unknown `options_from` resolver must leave options as None; \
         got options: {:?}",
        param.options
    );
}

/// When no options registry is threaded into `commands_for_scope`,
/// the enrichment pass is a no-op — every param keeps whatever the
/// YAML declared (including inline `options`). This is the
/// degenerate path used by surfaces like the native menu bar that
/// don't render picker UI.
#[test]
fn commands_for_scope_leaves_options_untouched_when_registry_is_none() {
    let yaml = r#"
- id: test.pick.no_registry
  name: Pick something
  scope: "entity:perspective"
  visible: true
  params:
    - name: thing
      from: args
      shape: enum
      options_from: "perspective.fields"
"#;
    let registry = registry_with(yaml);
    let impls: HashMap<String, Arc<dyn Command>> = HashMap::new();
    let ui_state = Arc::new(UIState::new());
    let scope = vec!["perspective:01P".to_string()];
    let dynamic = dynamic_with_perspective();
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        None,
        &ui_state,
        false,
        Some(&dynamic),
        None,
    );
    let cmd = find_cmd(&cmds, "test.pick.no_registry");
    let param = &cmd.params[0];
    assert!(
        param.options.is_none(),
        "without a registry threaded in, options stays at its YAML value (None here)",
    );
}

/// When a `perspective:{id}` moniker is in scope but the id does
/// not match any perspective in `DynamicSources.perspectives`, the
/// resolver returns an empty `Vec` (NOT `None`) — the registry
/// answered the key, the perspective just had nothing to offer.
/// This is the contract that lets the frontend distinguish
/// "resolver returned 0 options" from "no resolver registered".
#[test]
fn commands_for_scope_resolves_to_empty_options_when_perspective_unknown() {
    let yaml = r#"
- id: test.pick.empty
  name: Pick a field
  scope: "entity:perspective"
  visible: true
  params:
    - name: field
      from: args
      shape: enum
      options_from: "perspective.fields"
"#;
    let registry = registry_with(yaml);
    let impls: HashMap<String, Arc<dyn Command>> = HashMap::new();
    let ui_state = Arc::new(UIState::new());
    // Scope references a perspective id that the DynamicSources
    // doesn't carry — the resolver answers with an empty list.
    let scope = vec!["perspective:not-real".to_string()];
    let dynamic = dynamic_with_perspective();
    let opts_registry = default_options_registry();
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        None,
        &ui_state,
        false,
        Some(&dynamic),
        Some(&opts_registry),
    );
    let cmd = find_cmd(&cmds, "test.pick.empty");
    let param = &cmd.params[0];
    let options = param
        .options
        .as_ref()
        .expect("registered resolver always answers Some, even when the answer is empty");
    assert!(
        options.is_empty(),
        "resolver returned an empty list for a missing perspective; got {:?}",
        options
    );
}

/// A counting [`OptionsResolver`] for the ordering-contract test below.
///
/// Increments a shared [`AtomicUsize`] every time `resolve` is called
/// so the test can assert "no resolver was invoked for the dropped
/// command" — the strongest possible pin on the
/// `filter_by_view_kind` BEFORE `enrich_options` ordering.
struct CountingResolver {
    key: &'static str,
    calls: Arc<AtomicUsize>,
}

impl swissarmyhammer_commands::OptionsResolver for CountingResolver {
    fn key(&self) -> &'static str {
        self.key
    }

    fn resolve(
        &self,
        _ctx: &swissarmyhammer_commands::OptionsContext<'_>,
    ) -> Vec<swissarmyhammer_commands::ParamOption> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Vec::new()
    }
}

/// Pin the ordering contract: `enrich_options` runs AFTER
/// `filter_by_view_kind` inside `commands_for_scope`.
///
/// Register a synthetic command that declares both `view_kinds: [grid]`
/// AND an `options_from: "test.counting"`-tagged param, then emit it
/// under a `view:{id}` whose kind is `board`. The view-kind filter
/// must drop the command BEFORE the options-enrichment pass walks
/// `result.params[]` — so:
///
/// 1. The command must not appear in the emitted list.
/// 2. The counting resolver must observe zero `resolve` calls for the
///    dropped command (the only command in the fixture).
///
/// If a regression swapped the two passes inside `commands_for_scope`,
/// `enrich_options` would run BEFORE `filter_by_view_kind`, the
/// counting resolver would be invoked for the doomed command, and
/// this test would fail on the call-count assertion. Without this
/// test, the existing suite would silently accept that swap because
/// every other test exercises filtering and enrichment in isolation.
#[test]
fn commands_for_scope_skips_options_resolution_for_view_kind_filtered_commands() {
    // YAML for a synthetic command that both:
    //   - declares `view_kinds: [grid]` so a board-kind view drops it
    //   - declares an enum param with `options_from: "test.counting"`
    // so any pre-filter enrichment pass would invoke the counting
    // resolver and trip the assertion below.
    let yaml = r#"
- id: test.ordering.grid_only
  name: Pick a field (grid only)
  scope: "entity:perspective"
  visible: true
  view_kinds: [grid]
  params:
    - name: field
      from: args
      shape: enum
      options_from: "test.counting"
"#;
    let registry = registry_with(yaml);
    let impls: HashMap<String, Arc<dyn Command>> = HashMap::new();
    let ui_state = Arc::new(UIState::new());

    // Build dynamic sources with a board-kind view referenced by the
    // scope chain. The `perspective:01P` moniker keeps the command's
    // `scope: "entity:perspective"` satisfied so the command survives
    // emission and reaches the filter pass.
    let dynamic = DynamicSources {
        views: vec![ViewInfo {
            id: "V1".into(),
            name: "Board View".into(),
            entity_type: None,
            kind: "board".into(),
        }],
        perspectives: vec![PerspectiveInfo {
            id: "01P".into(),
            name: "Active Sprint".into(),
            view: "board".into(),
            fields: vec![PerspectiveFieldInfo {
                id: "01F1".into(),
                name: "title".into(),
                display_name: "Title".into(),
            }],
        }],
        ..Default::default()
    };
    let scope = vec!["perspective:01P".to_string(), "view:V1".to_string()];

    // Counting resolver wired into a fresh registry — we deliberately
    // do NOT use `default_options_registry()` here so the only
    // resolver that COULD fire for this fixture is the counter.
    let calls = Arc::new(AtomicUsize::new(0));
    let mut opts_registry = swissarmyhammer_commands::OptionsRegistry::new();
    opts_registry.register(Box::new(CountingResolver {
        key: "test.counting",
        calls: Arc::clone(&calls),
    }));

    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        None,
        &ui_state,
        false,
        Some(&dynamic),
        Some(&opts_registry),
    );

    // 1. Ordering claim, part one: the doomed command is dropped.
    assert!(
        cmds.iter().all(|c| c.id != "test.ordering.grid_only"),
        "view_kinds: [grid] under a board-kind view must drop \
         test.ordering.grid_only; got: {:?}",
        cmds.iter().map(|c| &c.id).collect::<Vec<_>>()
    );

    // 2. Ordering claim, part two: the resolver was never asked. If
    // `enrich_options` ran first, this count would be 1 (one param,
    // one call). That a count of 0 holds is the load-bearing pin on
    // "filter first, enrich second".
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "test.counting resolver must NOT be invoked for a command \
         dropped by filter_by_view_kind — enrich_options must run AFTER \
         filtering"
    );
}

/// End-to-end pin for the Group tab-button migration (task
/// 01KRE1ZTYJ5PPTQ29K72KE88B5): emit the REAL `perspective.group`
/// command (from the kanban-app builtin YAMLs) through
/// `commands_for_scope` with a perspective in scope that carries three
/// fields, and assert the emitted command's `group` param carries a
/// three-entry `options` list.
///
/// This is the picker-pipeline contract the frontend
/// `<CommandPopover>` consumes: backend YAML tags the param with
/// `options_from: "perspective.fields"`, the resolver projects
/// `PerspectiveFieldInfo` onto `ParamOption{value=id, label=display_name}`,
/// and the enriched command flows out via `commands_for_scope`. The
/// new test wires up real builtins (not a synthetic registry) so a
/// YAML regression that drops the `options_from` annotation or flips
/// `shape` off would surface here.
#[test]
fn perspective_group_command_carries_field_options_when_perspective_in_scope() {
    let sources = swissarmyhammer_kanban::builtin_yaml_sources();
    let sources_ref: Vec<(&str, &str)> = sources.iter().map(|(n, c)| (*n, *c)).collect();
    let registry = CommandsRegistry::from_yaml_sources(&sources_ref);

    let impls: HashMap<String, Arc<dyn Command>> = HashMap::new();
    let ui_state = Arc::new(UIState::new());

    // Three-field fixture — pins the `options.len() == 3` claim in the
    // task description's acceptance criteria.
    let dynamic = DynamicSources {
        perspectives: vec![PerspectiveInfo {
            id: "01P".into(),
            name: "Active Sprint".into(),
            view: "board".into(),
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
        views: vec![ViewInfo {
            id: "V1".into(),
            name: "Board".into(),
            entity_type: Some("task".into()),
            kind: "board".into(),
        }],
        ..Default::default()
    };

    let scope = vec!["perspective:01P".to_string(), "view:V1".to_string()];
    let opts_registry = default_options_registry();
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        None,
        &ui_state,
        false,
        Some(&dynamic),
        Some(&opts_registry),
    );

    let cmd = find_cmd(&cmds, "perspective.group");

    // Tab-button annotation survives the round-trip — the frontend
    // tab bar relies on `tab_button != null` to render the icon.
    assert!(
        cmd.tab_button.is_some(),
        "perspective.group must carry `tab_button` after the migration; \
         got: {cmd:?}"
    );
    assert_eq!(cmd.tab_button.as_ref().unwrap().icon, "group");

    // The `group` param is the enum-shaped picker target. Find it by
    // name (rather than positional index) so a future YAML reordering
    // doesn't silently shift the assertion onto `perspective_id`.
    let group_param = cmd
        .params
        .iter()
        .find(|p| p.name == "group")
        .expect("perspective.group YAML must declare a `group` param");
    assert_eq!(
        group_param.shape,
        Some(swissarmyhammer_commands::ParamShape::Enum),
        "the `group` param must carry shape: enum for the picker"
    );
    assert_eq!(
        group_param.options_from.as_deref(),
        Some("perspective.fields"),
        "the `group` param must wire `options_from: perspective.fields` \
         so the backend resolver fills options at emit time"
    );
    // Pin the full YAML → commands_for_scope → emitted-command
    // pipeline for `clear_command`. The frontend `<CommandPopover>`
    // reads this annotation to render the "(none)" affordance, and
    // `<CommandButton>.handleCommit` reads it to redirect the
    // empty-string sentinel to `perspective.clearGroup` instead of
    // dispatching `perspective.group`. A YAML regression that drops
    // the annotation would silently re-introduce the legacy "no way
    // to clear from the popover" UX bug.
    assert_eq!(
        group_param.clear_command.as_deref(),
        Some("perspective.clearGroup"),
        "the `group` param must carry `clear_command: perspective.clearGroup` \
         end-to-end so the popover's \"(none)\" affordance redirects to the \
         clearGroup command"
    );

    let options = group_param.options.as_ref().expect(
        "perspective.fields resolved against the three-field perspective \
         — `options` must be Some",
    );
    assert_eq!(
        options.len(),
        3,
        "three-field perspective must project to three ParamOption entries; \
         got: {options:?}"
    );
    // `value` carries the field NAME (slug), the wire format the
    // frontend's `computeGroups` and the persisted perspective YAML's
    // `group:` key both consume. See task `01KRH2EX1N1CA2HA3B4NMWZH67`.
    assert_eq!(options[0].value, "title");
    assert_eq!(options[0].label, "Title");
    assert_eq!(options[1].value, "status");
    assert_eq!(options[1].label, "Status");
    assert_eq!(options[2].value, "priority");
    assert_eq!(options[2].label, "Priority");
}

/// Negative-case companion to
/// `perspective_group_command_carries_field_options_when_perspective_in_scope`:
/// fields that are NOT `groupable: true` on the entity schema must not
/// reach the Group By picker, even when the perspective lists them in
/// its `fields[]` column order.
///
/// This is the regression review-finding 1 caught: the legacy
/// `<GroupSelector>` filtered with `f.groupable === true` on the
/// frontend; the command-driven-ui migration moved that responsibility
/// onto `denormalize_perspective_fields` in
/// `swissarmyhammer-kanban/src/dynamic_sources.rs`. Pinning the
/// contract end-to-end here (real `build_dynamic_sources` + real
/// `commands_for_scope` + real `perspective.group` YAML) guards
/// against a future change that quietly drops the filter on either
/// side of the pipeline.
///
/// Setup:
///   1. Open a fresh board (which loads the builtin field registry).
///   2. Add a perspective whose `fields[]` lists one non-groupable
///      field (`title`, id `00000000000000000000000001`) and one
///      groupable field (`assignees`, id `00000000000000000000000005`).
///   3. Pipe through `build_dynamic_sources` and then
///      `commands_for_scope`.
///   4. Locate the emitted `perspective.group` row's `group` param.
///
/// Assertions: `options` must be `Some`; the groupable field must
/// appear; the non-groupable field must NOT appear.
#[tokio::test]
async fn perspective_group_command_drops_non_groupable_fields_end_to_end() {
    use swissarmyhammer_kanban::perspective::AddPerspective;
    use swissarmyhammer_perspectives::PerspectiveFieldEntry;

    // Builtin field IDs — chosen so we have one of each `groupable` shape.
    // `title` (00...001) has no `groupable` annotation on the entity
    // schema → must be dropped. `assignees` (00...005) is annotated
    // `groupable: true` → must survive.
    //
    // The `*_NAME` constants are the schema-slug shape — the wire value
    // the resolver emits on `ParamOption.value` after task
    // `01KRH2EX1N1CA2HA3B4NMWZH67`. The `*_ID` constants are still used
    // for the intermediate denormalisation assertions (`f.id`).
    const FIELD_TITLE: &str = "00000000000000000000000001";
    const FIELD_ASSIGNEES: &str = "00000000000000000000000005";
    const FIELD_TITLE_NAME: &str = "title";
    const FIELD_ASSIGNEES_NAME: &str = "assignees";
    const BUILTIN_BOARD_VIEW_ID: &str = "01JMVIEW0000000000BOARD0";

    let temp = TempDir::new().expect("TempDir must allocate");
    let kanban_dir = temp.path().join(".kanban");
    let ctx = KanbanContext::open(&kanban_dir)
        .await
        .expect("KanbanContext::open must succeed");
    InitBoard::new("Sample")
        .execute(&ctx)
        .await
        .into_result()
        .expect("InitBoard must succeed");
    let canonical = kanban_dir
        .canonicalize()
        .unwrap_or_else(|_| kanban_dir.clone());
    let ctx = Arc::new(ctx);

    // Seed a perspective whose `fields[]` lists both a non-groupable
    // and a groupable field, in that order, so the assertion below can
    // pin "drops the non-groupable one regardless of position".
    let add_result = AddPerspective::new("Active Sprint", "board")
        .with_fields(vec![
            PerspectiveFieldEntry::new(FIELD_TITLE),
            PerspectiveFieldEntry::new(FIELD_ASSIGNEES),
        ])
        .execute(&ctx)
        .await
        .into_result()
        .expect("AddPerspective must succeed");
    let persp_id = add_result["id"]
        .as_str()
        .expect("add perspective must return an id")
        .to_string();

    // UIState — mark the board open and point at the builtin board view
    // so `resolve_active_view` returns a real id (otherwise
    // `gather_perspectives` short-circuits and returns nothing).
    let ui = UIState::new();
    let board_path_str = canonical.display().to_string();
    ui.add_open_board(&board_path_str);
    ui.set_active_view("main", BUILTIN_BOARD_VIEW_ID);

    let mut open_boards: HashMap<PathBuf, Arc<KanbanContext>> = HashMap::new();
    open_boards.insert(canonical.clone(), Arc::clone(&ctx));
    let windows = vec![WindowInfo {
        label: "main".to_string(),
        title: "SwissArmyHammer — Sample".to_string(),
        focused: true,
    }];

    let inputs = DynamicSourcesInputs {
        ui_state: &ui,
        active_ctx: Some(&ctx),
        open_board_ctxs: &open_boards,
        active_window_label: Some("main"),
        windows,
        ai_models: vec![],
    };
    let dynamic = build_dynamic_sources(inputs).await;

    // The denormalised perspective should already be filtered — pin
    // that intermediate contract so a regression on
    // `denormalize_perspective_fields` surfaces close to its source.
    let denormalised = dynamic
        .perspectives
        .iter()
        .find(|p| p.id == persp_id)
        .expect("seeded perspective must appear in DynamicSources");
    let denormalised_ids: Vec<&str> = denormalised.fields.iter().map(|f| f.id.as_str()).collect();
    assert!(
        denormalised_ids.contains(&FIELD_ASSIGNEES),
        "groupable field must survive denormalisation; got {denormalised_ids:?}"
    );
    assert!(
        !denormalised_ids.contains(&FIELD_TITLE),
        "non-groupable field must NOT survive denormalisation; got {denormalised_ids:?}"
    );

    // Pipe through `commands_for_scope` with the real registry so the
    // `perspective.group` YAML drives emission and the
    // `PerspectiveFieldsResolver` projects the (already-filtered)
    // perspective onto the param's `options`.
    let sources = swissarmyhammer_kanban::builtin_yaml_sources();
    let sources_ref: Vec<(&str, &str)> = sources.iter().map(|(n, c)| (*n, *c)).collect();
    let registry = CommandsRegistry::from_yaml_sources(&sources_ref);
    let impls: HashMap<String, Arc<dyn Command>> = HashMap::new();
    let ui_arc = Arc::new(ui);
    let scope = vec![
        format!("perspective:{persp_id}"),
        format!("view:{BUILTIN_BOARD_VIEW_ID}"),
        format!("board:{board_path_str}"),
    ];
    let opts_registry = default_options_registry();
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        ctx.fields(),
        &ui_arc,
        false,
        Some(&dynamic),
        Some(&opts_registry),
    );

    let cmd = find_cmd(&cmds, "perspective.group");
    let group_param = cmd
        .params
        .iter()
        .find(|p| p.name == "group")
        .expect("perspective.group YAML must declare a `group` param");
    let options = group_param
        .options
        .as_ref()
        .expect("perspective.fields resolved → options must be Some");
    // `value` carries the field NAME (slug). See task
    // `01KRH2EX1N1CA2HA3B4NMWZH67` — the wire format is name-shaped end
    // to end so the dispatched `perspective.group` arg lines up with
    // `task.fields[<name>]` in `<GroupedBoardView>` and the persisted
    // `group:` key in `.kanban/perspectives/*.yaml`.
    let option_values: Vec<&str> = options.iter().map(|o| o.value.as_str()).collect();
    assert!(
        option_values.contains(&FIELD_ASSIGNEES_NAME),
        "groupable field must appear in Group By options; got {option_values:?}"
    );
    assert!(
        !option_values.contains(&FIELD_TITLE_NAME),
        "non-groupable field MUST NOT appear in Group By options \
         (this is the legacy <GroupSelector>'s `f.groupable === true` filter, \
         now moved to denormalize_perspective_fields); got {option_values:?}"
    );
}

/// Regression for task `01KRGW1DYD0T05PSTEDPT5D076` — when a perspective
/// is opened with an empty `fields[]` (the shape every real user
/// perspective at `.kanban/perspectives/*.yaml` actually has), the Group
/// By picker is still expected to be populated with the *entity
/// schema's* groupable fields. Pre-fix, the picker reads from
/// `perspective.fields[]` and is empty whenever the perspective has no
/// column overrides — which is the common case.
///
/// Mirrors the legacy `<GroupSelector>` contract: its `fields` prop was
/// `schemaFields = getSchema(entityType)?.fields ?? []` filtered by
/// `f.groupable === true`. The command-driven-ui migration must
/// preserve that source — the picker emits every groupable field on the
/// perspective's entity type, regardless of whether the perspective
/// pins them in its visible column list.
///
/// Setup:
///   1. Open a fresh board (which loads the builtin field registry +
///      builtin views, including the board view scoped to `task`).
///   2. Add a perspective with `view: "board"`, `view_id` pinned to the
///      builtin board view, and an EMPTY `fields[]`.
///   3. Pipe through `build_dynamic_sources` and then
///      `commands_for_scope`.
///   4. Locate the emitted `perspective.group` row's `group` param.
///
/// Assertions: `options` must be `Some` and non-empty; the groupable
/// task field `assignees` must appear; the non-groupable `title` must
/// NOT appear.
#[tokio::test]
async fn perspective_group_command_emits_groupable_fields_from_live_field_loader() {
    use swissarmyhammer_kanban::perspective::AddPerspective;

    // Builtin field IDs / names. The picker emits `value = field_name`
    // (slug) after task `01KRH2EX1N1CA2HA3B4NMWZH67`; the IDs are kept
    // here for documentation continuity but unused below.
    // `assignees` (00...005) has `groupable: true` → must appear.
    // `title` (00...001) has no `groupable` annotation → must NOT.
    const _FIELD_TITLE: &str = "00000000000000000000000001";
    const _FIELD_ASSIGNEES: &str = "00000000000000000000000005";
    const FIELD_TITLE_NAME: &str = "title";
    const FIELD_ASSIGNEES_NAME: &str = "assignees";
    const BUILTIN_BOARD_VIEW_ID: &str = "01JMVIEW0000000000BOARD0";

    let temp = TempDir::new().expect("TempDir must allocate");
    let kanban_dir = temp.path().join(".kanban");
    let ctx = KanbanContext::open(&kanban_dir)
        .await
        .expect("KanbanContext::open must succeed");
    InitBoard::new("Sample")
        .execute(&ctx)
        .await
        .into_result()
        .expect("InitBoard must succeed");
    let canonical = kanban_dir
        .canonicalize()
        .unwrap_or_else(|_| kanban_dir.clone());
    let ctx = Arc::new(ctx);

    // Seed a perspective with NO `fields[]` — the shape every real
    // user perspective at `.kanban/perspectives/*.yaml` has. Pin the
    // perspective to the builtin board view so the resolver can map
    // back to entity_type=task.
    let add_result = AddPerspective::new("Active Sprint", "board")
        .with_view_id(BUILTIN_BOARD_VIEW_ID)
        .execute(&ctx)
        .await
        .into_result()
        .expect("AddPerspective must succeed");
    let persp_id = add_result["id"]
        .as_str()
        .expect("add perspective must return an id")
        .to_string();

    let ui = UIState::new();
    let board_path_str = canonical.display().to_string();
    ui.add_open_board(&board_path_str);
    ui.set_active_view("main", BUILTIN_BOARD_VIEW_ID);

    let mut open_boards: HashMap<PathBuf, Arc<KanbanContext>> = HashMap::new();
    open_boards.insert(canonical.clone(), Arc::clone(&ctx));
    let windows = vec![WindowInfo {
        label: "main".to_string(),
        title: "SwissArmyHammer — Sample".to_string(),
        focused: true,
    }];

    let inputs = DynamicSourcesInputs {
        ui_state: &ui,
        active_ctx: Some(&ctx),
        open_board_ctxs: &open_boards,
        active_window_label: Some("main"),
        windows,
        ai_models: vec![],
    };
    let dynamic = build_dynamic_sources(inputs).await;

    // Pipe through `commands_for_scope` with the real registry so the
    // `perspective.group` YAML drives emission and the
    // `PerspectiveFieldsResolver` projects the perspective's entity-
    // schema groupable fields onto the param's `options`.
    let sources = swissarmyhammer_kanban::builtin_yaml_sources();
    let sources_ref: Vec<(&str, &str)> = sources.iter().map(|(n, c)| (*n, *c)).collect();
    let registry = CommandsRegistry::from_yaml_sources(&sources_ref);
    let impls: HashMap<String, Arc<dyn Command>> = HashMap::new();
    let ui_arc = Arc::new(ui);
    let scope = vec![
        format!("perspective:{persp_id}"),
        format!("view:{BUILTIN_BOARD_VIEW_ID}"),
        format!("board:{board_path_str}"),
    ];
    let opts_registry = default_options_registry();
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        ctx.fields(),
        &ui_arc,
        false,
        Some(&dynamic),
        Some(&opts_registry),
    );

    let cmd = find_cmd(&cmds, "perspective.group");
    let group_param = cmd
        .params
        .iter()
        .find(|p| p.name == "group")
        .expect("perspective.group YAML must declare a `group` param");
    let options = group_param
        .options
        .as_ref()
        .expect("perspective.fields resolved → options must be Some");
    let option_values: Vec<&str> = options.iter().map(|o| o.value.as_str()).collect();
    assert!(
        !options.is_empty(),
        "Group By options must NOT be empty when the entity schema has \
         groupable fields — pre-fix the picker sourced from `perspective.fields[]` \
         which is empty for every real user perspective. The fix routes the picker \
         to the entity schema's groupable fields. Got: {option_values:?}"
    );
    assert!(
        option_values.contains(&FIELD_ASSIGNEES_NAME),
        "groupable task field `assignees` must appear in Group By options \
         even when `perspective.fields[]` is empty; got {option_values:?}"
    );
    assert!(
        !option_values.contains(&FIELD_TITLE_NAME),
        "non-groupable task field `title` must NOT appear in Group By options; \
         got {option_values:?}"
    );
}

/// **Iteration-4 regression** for task `01KRGW1DYD0T05PSTEDPT5D076`.
///
/// Reproduces the user's actual production setup verbatim: a legacy
/// perspective with `view: "board"` and `view_id: None`, viewed on the
/// builtin board view (`01JMVIEW0000000000BOARD0`). Pre-fix, the Group
/// By picker is empty for this configuration.
///
/// Iteration-1's test pinned a similar shape via
/// `AddPerspective::with_view_id(BOARD)`, which routes through
/// `maybe_pin_view_id_on_save` and persists `view_id: Some(BOARD)` —
/// taking the **strict** path in `entity_type_for_perspective`. That
/// test would not have caught the iteration-4 bug because the strict
/// path never engages with the legacy `view_id: None` shape.
///
/// Iteration-2's test pinned the legacy `view_id: None` shape but for
/// the **grid** view kind on a workspace with multiple grid-kind
/// builtins (`tasks-grid`, `projects-grid`, `tags-grid`) with
/// conflicting `entity_type`. There the legacy by-kind fallback fails
/// (ambiguous), and the **active-view tiebreaker** path is the one
/// being exercised. That test would not have caught the iteration-4 bug
/// because the active-view tiebreaker only engages when
/// `active_view.kind == perspective.view`, and the iteration-4 user is
/// on the board view.
///
/// The iteration-4 test exercises a third code path neither prior test
/// touched: legacy `view_id: None` + board kind, where the workspace
/// has exactly ONE board-kind view. By-kind matching alone should
/// resolve unambiguously to `entity_type=task`. If that pathway is
/// broken (e.g. the perspective is filtered out before its entity_type
/// is resolved, or the entity-type derivation drops the result, or the
/// scope chain doesn't reach the resolver), this test fails and the
/// user's bug is reproduced.
///
/// Assertions: `options` non-empty; `assignees`, `tags`, and `project`
/// (all groupable on the `task` entity schema per the builtin YAMLs)
/// all appear.
#[tokio::test]
async fn perspective_group_options_include_assignees_and_tags_for_board_task_perspective() {
    use swissarmyhammer_perspectives::Perspective;

    // Builtin field IDs and names — all three fields are groupable on
    // the `task` entity per the builtin YAMLs at
    // swissarmyhammer-kanban/builtin/definitions/. The `*_NAME`
    // constants are the schema-slug shape — the wire `value` the
    // resolver emits after task `01KRH2EX1N1CA2HA3B4NMWZH67`. The
    // `*_ID` constants are kept for the intermediate denormalisation
    // assertions that read `f.id`.
    const FIELD_ASSIGNEES: &str = "00000000000000000000000005";
    const FIELD_TAGS: &str = "00000000000000000000000004";
    const FIELD_PROJECT: &str = "00000000000000000000000010";
    const FIELD_ASSIGNEES_NAME: &str = "assignees";
    const FIELD_TAGS_NAME: &str = "tags";
    const FIELD_PROJECT_NAME: &str = "project";
    const BUILTIN_BOARD_VIEW_ID: &str = "01JMVIEW0000000000BOARD0";

    let temp = TempDir::new().expect("TempDir must allocate");
    let kanban_dir = temp.path().join(".kanban");
    let ctx = KanbanContext::open(&kanban_dir)
        .await
        .expect("KanbanContext::open must succeed");
    InitBoard::new("Sample")
        .execute(&ctx)
        .await
        .into_result()
        .expect("InitBoard must succeed");
    let canonical = kanban_dir
        .canonicalize()
        .unwrap_or_else(|_| kanban_dir.clone());
    let ctx = Arc::new(ctx);

    // Write the perspective directly through PerspectiveContext::write
    // to preserve `view_id: None` — `AddPerspective::execute` would
    // route through `maybe_pin_view_id_on_save` and pin the perspective
    // to the unambiguous builtin board view, masking the bug.
    //
    // This shape — `view: "board"`, `view_id: None` — is exactly what
    // every pre-migration perspective on the user's disk has.
    let persp_id = "01PFIXTUREVIEWIDLESSBOARD0".to_string();
    let legacy_perspective = Perspective::new(&persp_id, "Default", "board");
    {
        let pctx = ctx
            .perspective_context()
            .await
            .expect("perspective_context must open");
        let mut pctx = pctx.write().await;
        pctx.write(&legacy_perspective)
            .await
            .expect("legacy perspective must persist");
    }

    // Active view = the builtin Board view. This matches the user's
    // runtime: when the app opens the board, `UIState::set_active_view`
    // is set to the board view's id for the main window.
    let ui = UIState::new();
    let board_path_str = canonical.display().to_string();
    ui.add_open_board(&board_path_str);
    ui.set_active_view("main", BUILTIN_BOARD_VIEW_ID);

    let mut open_boards: HashMap<PathBuf, Arc<KanbanContext>> = HashMap::new();
    open_boards.insert(canonical.clone(), Arc::clone(&ctx));
    let windows = vec![WindowInfo {
        label: "main".to_string(),
        title: "SwissArmyHammer — Sample".to_string(),
        focused: true,
    }];

    let inputs = DynamicSourcesInputs {
        ui_state: &ui,
        active_ctx: Some(&ctx),
        open_board_ctxs: &open_boards,
        active_window_label: Some("main"),
        windows,
        ai_models: vec![],
    };
    let dynamic = build_dynamic_sources(inputs).await;

    // First: pin the intermediate denormalisation result. With the
    // legacy `view_id: None` + active board view shape, the resolver
    // should land on `entity_type=task` and the field list should
    // include `assignees`, `tags`, and `project`.
    let denormalised = dynamic
        .perspectives
        .iter()
        .find(|p| p.id == persp_id)
        .expect(
            "seeded board perspective must appear in DynamicSources — \
             if this fails the perspective was filtered out by \
             `perspective_belongs_to_active_view` before the resolver \
             ever ran",
        );
    let denormalised_ids: Vec<&str> = denormalised.fields.iter().map(|f| f.id.as_str()).collect();
    assert!(
        !denormalised.fields.is_empty(),
        "view-id-less perspective on an active board view MUST denormalise \
         to a non-empty field list — the task entity schema has groupable \
         fields (assignees, tags, project). got fields: {denormalised_ids:?}"
    );
    assert!(
        denormalised_ids.contains(&FIELD_ASSIGNEES),
        "groupable task field `assignees` must surface; got {denormalised_ids:?}"
    );
    assert!(
        denormalised_ids.contains(&FIELD_TAGS),
        "groupable task field `tags` must surface; got {denormalised_ids:?}"
    );
    assert!(
        denormalised_ids.contains(&FIELD_PROJECT),
        "groupable task field `project` must surface; got {denormalised_ids:?}"
    );

    // Now: pipe through `commands_for_scope_with_context` — the exact
    // call path the live `kanban-app::list_commands_for_scope` uses. The
    // helper sources the options-resolver registry from the
    // `KanbanContext`, eliminating the foot-gun (a caller threading
    // fields but passing `None` for the options registry) that produced
    // the empty-popover bug at iter-3.
    let sources = swissarmyhammer_kanban::builtin_yaml_sources();
    let sources_ref: Vec<(&str, &str)> = sources.iter().map(|(n, c)| (*n, *c)).collect();
    let registry = CommandsRegistry::from_yaml_sources(&sources_ref);
    let impls: HashMap<String, Arc<dyn Command>> = HashMap::new();
    let ui_arc = Arc::new(ui);
    let scope = vec![
        format!("perspective:{persp_id}"),
        format!("view:{BUILTIN_BOARD_VIEW_ID}"),
        format!("board:{board_path_str}"),
    ];
    let cmds = swissarmyhammer_kanban::scope_commands::commands_for_scope_with_context(
        &scope,
        &registry,
        &impls,
        Some(ctx.as_ref()),
        &ui_arc,
        false,
        Some(&dynamic),
    );

    let cmd = find_cmd(&cmds, "perspective.group");
    let group_param = cmd
        .params
        .iter()
        .find(|p| p.name == "group")
        .expect("perspective.group YAML must declare a `group` param");
    let options = group_param.options.as_ref().expect(
        "perspective.fields resolved → options must be Some. \
             If this assertion trips, the options-resolver registry was \
             not threaded through from the active context — that is the \
             exact regression task 01KRGW1DYD0T05PSTEDPT5D076 (iter-4) \
             fixes.",
    );
    let option_values: Vec<&str> = options.iter().map(|o| o.value.as_str()).collect();
    assert!(
        !options.is_empty(),
        "Group By options MUST be populated for the user's actual data \
         shape (legacy view-id-less perspective + builtin board view + \
         board kind matching unambiguously to entity_type=task). \
         got: {option_values:?}"
    );
    assert!(
        option_values.contains(&FIELD_ASSIGNEES_NAME),
        "groupable task field `assignees` must appear in Group By options \
         when viewing on the board; got {option_values:?}"
    );
    assert!(
        option_values.contains(&FIELD_TAGS_NAME),
        "groupable task field `tags` must appear in Group By options \
         when viewing on the board; got {option_values:?}"
    );
    assert!(
        option_values.contains(&FIELD_PROJECT_NAME),
        "groupable task field `project` must appear in Group By options \
         when viewing on the board; got {option_values:?}"
    );
}

/// **Iteration-2 regression** for task `01KRGW1DYD0T05PSTEDPT5D076`.
///
/// The iteration-1 fix sourced picker options from the entity schema
/// (instead of `perspective.fields[]`) and passed a regression test —
/// but the user's empty popover persisted in production. The reason:
/// the user's `.kanban/perspectives/*.yaml` files were saved BEFORE
/// `maybe_pin_view_id_on_save` was introduced, so they have
/// `view_id: None`. With multiple grid-kind builtin views in the
/// workspace (`tasks-grid` → task, `projects-grid` → project,
/// `tags-grid` → tag), the legacy by-kind entity-type derivation
/// found conflicting entity types and bailed out with `None` →
/// empty picker.
///
/// The iteration-2 fix uses the **active view in scope** as a
/// tiebreaker: when the perspective's `view_id` is `None` and the
/// active view's kind matches the perspective's `view`, the
/// resolver answers for the active view's `entity_type`. This pins
/// that contract end-to-end through `build_dynamic_sources` and
/// `commands_for_scope`, with a fixture that matches the user's
/// production data shape (legacy view-id-less perspective +
/// multiple grid-kind views with conflicting entity types + active
/// view in scope).
///
/// Setup:
///   1. Open a fresh board (loads builtin views: 1 board-kind +
///      tasks-grid + projects-grid + tags-grid, all grid-kind with
///      conflicting entity types).
///   2. Persist a perspective with `view: "grid"`, `view_id: None`,
///      empty `fields[]` — bypasses `AddPerspective` (which would
///      auto-pin via `maybe_pin_view_id_on_save`) to preserve the
///      `view_id: None` shape the user actually has on disk.
///   3. Set the active view to the builtin Tasks Grid via UIState.
///   4. Pipe through `build_dynamic_sources` and `commands_for_scope`.
///
/// Assertions: `options` non-empty, `assignees` present (task entity
/// schema's groupable field), `tag_name` absent (tag entity, the
/// wrong sibling grid → not the active view's entity).
#[tokio::test]
async fn perspective_group_options_use_active_view_when_perspective_view_id_is_none() {
    use swissarmyhammer_perspectives::Perspective;
    // Re-imported locally to keep this fixture's deps obvious and to
    // avoid touching the file-level imports used by older tests.

    // Builtin field IDs and names.
    // `assignees` (00...005) is groupable on the `task` entity schema
    // → must appear when the active view's entity_type is task.
    // `tag_name` (00...020) is groupable on the `tag` entity schema —
    // a sibling `grid`-kind view points at it; this field must NOT
    // surface because the active view is the Tasks Grid, not the
    // Tags Grid.
    //
    // After task `01KRH2EX1N1CA2HA3B4NMWZH67` the resolver emits
    // `value = field_name` (slug). The `*_ID` constants are kept for
    // the intermediate denormalisation assertions on `f.id`.
    const FIELD_ASSIGNEES: &str = "00000000000000000000000005";
    const FIELD_TAG_NAME: &str = "00000000000000000000000009";
    const FIELD_ASSIGNEES_NAME: &str = "assignees";
    const FIELD_TAG_NAME_NAME: &str = "tag_name";
    const BUILTIN_TASKS_GRID_VIEW_ID: &str = "01JMVIEW0000000000TGRID0";

    let temp = TempDir::new().expect("TempDir must allocate");
    let kanban_dir = temp.path().join(".kanban");
    let ctx = KanbanContext::open(&kanban_dir)
        .await
        .expect("KanbanContext::open must succeed");
    InitBoard::new("Sample")
        .execute(&ctx)
        .await
        .into_result()
        .expect("InitBoard must succeed");
    let canonical = kanban_dir
        .canonicalize()
        .unwrap_or_else(|_| kanban_dir.clone());
    let ctx = Arc::new(ctx);

    // Write the perspective directly through the lower-level
    // PerspectiveContext::write to preserve `view_id: None` —
    // `AddPerspective::execute` would route through
    // `maybe_pin_view_id_on_save` and auto-pin to the unambiguous
    // builtin view, masking the bug.
    let persp_id = "01PFIXTUREVIEWIDLESSGRID01".to_string();
    // Build via the public constructor — `Perspective` is `#[non_exhaustive]`
    // so callers must route through the builder. NO `with_view_id` call
    // keeps the fixture in the legacy `view_id: None` shape, which is the
    // shape every pre-migration perspective on the user's disk actually has.
    let legacy_perspective = Perspective::new(&persp_id, "Default", "grid");
    {
        let pctx = ctx
            .perspective_context()
            .await
            .expect("perspective_context must open");
        let mut pctx = pctx.write().await;
        pctx.write(&legacy_perspective)
            .await
            .expect("legacy perspective must persist");
    }

    // Active view = Tasks Grid. The perspective above is legacy
    // (view_id: None) with `view: "grid"`. The user's actual setup —
    // multiple grid-kind views in the workspace, one of which is
    // the active view — is what tripped the iteration-1 fix.
    let ui = UIState::new();
    let board_path_str = canonical.display().to_string();
    ui.add_open_board(&board_path_str);
    ui.set_active_view("main", BUILTIN_TASKS_GRID_VIEW_ID);

    let mut open_boards: HashMap<PathBuf, Arc<KanbanContext>> = HashMap::new();
    open_boards.insert(canonical.clone(), Arc::clone(&ctx));
    let windows = vec![WindowInfo {
        label: "main".to_string(),
        title: "SwissArmyHammer — Sample".to_string(),
        focused: true,
    }];

    let inputs = DynamicSourcesInputs {
        ui_state: &ui,
        active_ctx: Some(&ctx),
        open_board_ctxs: &open_boards,
        active_window_label: Some("main"),
        windows,
        ai_models: vec![],
    };
    let dynamic = build_dynamic_sources(inputs).await;

    // First, pin the intermediate denormalisation result so a
    // regression on `denormalize_perspective_fields` surfaces close
    // to its source. With `view_id: None` + active Tasks Grid in
    // scope, the active-view tiebreaker should resolve
    // entity_type=task and the perspective's fields list should
    // include `assignees`.
    let denormalised = dynamic
        .perspectives
        .iter()
        .find(|p| p.id == persp_id)
        .expect("seeded perspective must appear in DynamicSources");
    let denormalised_ids: Vec<&str> = denormalised.fields.iter().map(|f| f.id.as_str()).collect();
    assert!(
        !denormalised.fields.is_empty(),
        "view-id-less perspective on an active grid view MUST denormalise \
         to a non-empty field list — the active-view tiebreaker should \
         resolve entity_type=task. Pre-iter-2 the by-kind fallback bailed \
         on conflicting entity types and returned an empty list. \
         got fields: {denormalised_ids:?}"
    );
    assert!(
        denormalised_ids.contains(&FIELD_ASSIGNEES),
        "task entity's groupable field `assignees` must surface; got {denormalised_ids:?}"
    );
    assert!(
        !denormalised_ids.contains(&FIELD_TAG_NAME),
        "tag entity's groupable field `tag_name` MUST NOT surface — the \
         active view is the Tasks Grid, not the Tags Grid; got {denormalised_ids:?}"
    );

    // Now pipe through commands_for_scope and assert the same
    // contract at the wire-format boundary the frontend reads.
    let sources = swissarmyhammer_kanban::builtin_yaml_sources();
    let sources_ref: Vec<(&str, &str)> = sources.iter().map(|(n, c)| (*n, *c)).collect();
    let registry = CommandsRegistry::from_yaml_sources(&sources_ref);
    let impls: HashMap<String, Arc<dyn Command>> = HashMap::new();
    let ui_arc = Arc::new(ui);
    let scope = vec![
        format!("perspective:{persp_id}"),
        format!("view:{BUILTIN_TASKS_GRID_VIEW_ID}"),
        format!("board:{board_path_str}"),
    ];
    let opts_registry = default_options_registry();
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        ctx.fields(),
        &ui_arc,
        false,
        Some(&dynamic),
        Some(&opts_registry),
    );

    let cmd = find_cmd(&cmds, "perspective.group");
    let group_param = cmd
        .params
        .iter()
        .find(|p| p.name == "group")
        .expect("perspective.group YAML must declare a `group` param");
    let options = group_param
        .options
        .as_ref()
        .expect("perspective.fields resolved → options must be Some");
    let option_values: Vec<&str> = options.iter().map(|o| o.value.as_str()).collect();
    assert!(
        !options.is_empty(),
        "Group By options MUST be populated for the user's actual \
         data shape (legacy view-id-less perspective + multiple \
         grid-kind views with conflicting entity types + active view \
         in scope). Pre-iter-2 the picker was empty because the \
         entity-type resolver bailed on by-kind ambiguity. \
         got: {option_values:?}"
    );
    assert!(
        option_values.contains(&FIELD_ASSIGNEES_NAME),
        "groupable task field `assignees` must appear when the active \
         view is the Tasks Grid; got {option_values:?}"
    );
    assert!(
        !option_values.contains(&FIELD_TAG_NAME_NAME),
        "tag entity's `tag_name` must NOT appear when the active view \
         is the Tasks Grid — the tiebreaker must prefer the active \
         view's entity_type over wrong-sibling entries; got {option_values:?}"
    );
}

/// End-to-end pin for the Sort tab-button migration (task
/// 01KRE21GJMPP289N1HSTMJG5HE): emit the REAL `perspective.sort.set`
/// command (from the kanban-app builtin YAMLs) through
/// `commands_for_scope` with a perspective in scope that carries two
/// fields and an active grid view, and assert:
///
///   * The emitted command carries `tab_button` after the migration
///     (the icon survives the YAML → wire-format round trip).
///   * The `field` enum param carries `options_from: "perspective.fields"`
///     and is populated by the backend `PerspectiveFieldsResolver`.
///   * The `direction` enum param carries `options_from: "sort.directions"`
///     and is populated by the backend `SortDirectionsResolver` —
///     EXACTLY two entries, `[asc, desc]`, in that order, with labels
///     `Ascending` / `Descending`. The `SortDirection` serde
///     representation in `swissarmyhammer-perspectives` is
///     `#[serde(rename_all = "lowercase")]`, so drift on the resolver's
///     `value` would break round-trip when the dispatcher re-reads the
///     persisted perspective.
///
/// This is the picker-pipeline contract the frontend
/// `<CommandPopover>` consumes for the multi-param form branch — Sort
/// is the first command in the epic to have TWO pickable enum params
/// in one popover.
#[test]
fn perspective_sort_set_command_carries_field_and_direction_options() {
    let sources = swissarmyhammer_kanban::builtin_yaml_sources();
    let sources_ref: Vec<(&str, &str)> = sources.iter().map(|(n, c)| (*n, *c)).collect();
    let registry = CommandsRegistry::from_yaml_sources(&sources_ref);

    let impls: HashMap<String, Arc<dyn Command>> = HashMap::new();
    let ui_state = Arc::new(UIState::new());

    // Grid-kind view in the dynamic source so `filter_by_view_kind`
    // keeps `perspective.sort.set` (which carries `view_kinds: [grid]`)
    // — the same gate that hides the Sort tab button on board views in
    // the frontend. Without a grid view in scope the command would be
    // dropped and the assertions below would never run.
    let dynamic = DynamicSources {
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
            ],
        }],
        views: vec![ViewInfo {
            id: "V1".into(),
            name: "Tasks Grid".into(),
            entity_type: Some("task".into()),
            kind: "grid".into(),
        }],
        ..Default::default()
    };

    let scope = vec!["perspective:01P".to_string(), "view:V1".to_string()];
    let opts_registry = default_options_registry();
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        None,
        &ui_state,
        false,
        Some(&dynamic),
        Some(&opts_registry),
    );

    let cmd = find_cmd(&cmds, "perspective.sort.set");

    // Tab-button annotation survives the round-trip — the frontend
    // tab bar relies on `tab_button != null` to render the icon.
    assert!(
        cmd.tab_button.is_some(),
        "perspective.sort.set must carry `tab_button` after the migration; \
         got: {cmd:?}"
    );
    assert_eq!(
        cmd.tab_button.as_ref().unwrap().icon,
        "arrow-up-down",
        "the `arrow-up-down` lucide icon is the YAML annotation; if this \
         changes, update `command-icon-registry.ts` in lockstep"
    );

    // Field param — enum-shaped, sourced from the same resolver as
    // Group By so the picker offers the active perspective's sortable
    // fields. Find by name so a future YAML reorder doesn't silently
    // shift the assertion onto `direction` or `perspective_id`.
    let field_param = cmd
        .params
        .iter()
        .find(|p| p.name == "field")
        .expect("perspective.sort.set YAML must declare a `field` param");
    assert_eq!(
        field_param.shape,
        Some(swissarmyhammer_commands::ParamShape::Enum),
        "the `field` param must carry shape: enum for the picker"
    );
    assert_eq!(
        field_param.options_from.as_deref(),
        Some("perspective.fields"),
        "the `field` param must wire `options_from: perspective.fields`"
    );
    let field_options = field_param
        .options
        .as_ref()
        .expect("perspective.fields resolved → field.options must be Some");
    assert!(
        !field_options.is_empty(),
        "perspective.fields resolved against the two-field perspective \
         — `field.options` must be non-empty; got: {field_options:?}"
    );
    let field_values: Vec<&str> = field_options.iter().map(|o| o.value.as_str()).collect();
    // The resolver projects every PerspectiveFieldInfo's `name` (slug)
    // onto `ParamOption.value`. Both seeded fields must appear; the
    // ordering matches the perspective's `fields[]` order.
    assert!(
        field_values.contains(&"title"),
        "groupable field `title` must appear in Sort options; got {field_values:?}"
    );
    assert!(
        field_values.contains(&"status"),
        "groupable field `status` must appear in Sort options; got {field_values:?}"
    );

    // Direction param — enum-shaped, sourced from the static
    // `SortDirectionsResolver`. Exact-match list so drift on the
    // resolver's serde wire format breaks this test before it breaks
    // perspective load round-trip.
    let direction_param = cmd
        .params
        .iter()
        .find(|p| p.name == "direction")
        .expect("perspective.sort.set YAML must declare a `direction` param");
    assert_eq!(
        direction_param.shape,
        Some(swissarmyhammer_commands::ParamShape::Enum),
        "the `direction` param must carry shape: enum for the picker"
    );
    assert_eq!(
        direction_param.options_from.as_deref(),
        Some("sort.directions"),
        "the `direction` param must wire `options_from: sort.directions`"
    );
    let direction_options = direction_param
        .options
        .as_ref()
        .expect("sort.directions resolved → direction.options must be Some");
    assert_eq!(
        direction_options.len(),
        2,
        "SortDirectionsResolver returns exactly two entries; got: {direction_options:?}"
    );
    assert_eq!(direction_options[0].value, "asc");
    assert_eq!(direction_options[0].label, "Ascending");
    assert_eq!(direction_options[1].value, "desc");
    assert_eq!(direction_options[1].label, "Descending");
}

/// End-to-end pin for the `ai.model` model-picker resolver (task
/// 01KRRN69YDB2B03RB1N9G6RR3J, review finding): emit the REAL `ai.model`
/// command (from the kanban builtin `ai.yaml`) through
/// `commands_for_scope` with two AI models supplied in
/// `DynamicSources.ai_models`, and assert:
///
///   * The `model` param carries `shape: enum` — so the palette renders
///     a `<CommandPopover>` picker rather than a free-text input.
///   * The `model` param carries `options_from: "ai.models"` — the YAML
///     annotation that wires it to the backend `AiModelsResolver`.
///   * The param's `options` are populated by the resolver with EXACTLY
///     the two supplied models, in enumeration order, with
///     `value = model id` and `label = model label`.
///
/// This is the picker-pipeline contract the frontend `<CommandPopover>`
/// consumes — the same path `perspective.sort.set`'s `field` param uses
/// (see `perspective_sort_set_command_carries_field_and_direction_options`
/// above). A YAML regression that drops `options_from` or flips `shape`
/// back to `text` — re-introducing the raw-model-id-typing UX the review
/// flagged — surfaces here.
#[test]
fn ai_model_command_carries_model_options_from_resolver() {
    use swissarmyhammer_kanban::commands::options_resolvers::AiModelInfo;

    let sources = swissarmyhammer_kanban::builtin_yaml_sources();
    let sources_ref: Vec<(&str, &str)> = sources.iter().map(|(n, c)| (*n, *c)).collect();
    let registry = CommandsRegistry::from_yaml_sources(&sources_ref);

    let impls: HashMap<String, Arc<dyn Command>> = HashMap::new();
    let ui_state = Arc::new(UIState::new());

    // Two models in the dynamic source — `ai.model` is a window-layer
    // command with no `scope:` pin, so it resolves under a bare board
    // scope. The `ai.models` resolver is scope-independent: it reads the
    // supplied `ai_models` list verbatim regardless of the scope chain.
    let dynamic = DynamicSources {
        ai_models: vec![
            AiModelInfo {
                id: "claude-code".into(),
                label: "Claude Code".into(),
            },
            AiModelInfo {
                id: "qwen-coder".into(),
                label: "Qwen Coder".into(),
            },
        ],
        ..Default::default()
    };

    let scope = vec!["board:my-board".to_string()];
    let opts_registry = default_options_registry();
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        None,
        &ui_state,
        false,
        Some(&dynamic),
        Some(&opts_registry),
    );

    let cmd = find_cmd(&cmds, "ai.model");

    // The `model` enum param is the picker target. Find it by name so a
    // future YAML reorder doesn't shift the assertion.
    let model_param = cmd
        .params
        .iter()
        .find(|p| p.name == "model")
        .expect("ai.model YAML must declare a `model` param");
    assert_eq!(
        model_param.shape,
        Some(swissarmyhammer_commands::ParamShape::Enum),
        "the `model` param must carry shape: enum so the palette renders a \
         picker, not a free-text box"
    );
    assert_eq!(
        model_param.options_from.as_deref(),
        Some("ai.models"),
        "the `model` param must wire `options_from: ai.models` so the \
         backend resolver fills the picker options at emit time"
    );

    let options = model_param
        .options
        .as_ref()
        .expect("ai.models resolved against the two-model fixture — `options` must be Some");
    assert_eq!(
        options.len(),
        2,
        "two supplied models must project to two ParamOption entries; got: {options:?}"
    );
    // `value` carries the model id — the wire value the dispatched
    // `ai.model` arg lands on and the frontend's per-board model-selection
    // handler applies. Enumeration order is preserved.
    assert_eq!(options[0].value, "claude-code");
    assert_eq!(options[0].label, "Claude Code");
    assert_eq!(options[1].value, "qwen-coder");
    assert_eq!(options[1].label, "Qwen Coder");
}

/// Companion negative case: when no AI models are supplied (an empty
/// `DynamicSources.ai_models`), the `ai.model` command's `model` param
/// resolves to an empty `options` list — `Some(vec![])`, NOT `None`.
///
/// The registered `AiModelsResolver` always *answers* the `ai.models`
/// key; an empty answer means "this machine has no selectable models"
/// (e.g. agent discovery failed and the GUI threaded an empty list).
/// The frontend distinguishes that from "no resolver registered"
/// (`options: None`). Pins the resolver's empty-answer contract through
/// the real `ai.yaml` end to end.
#[test]
fn ai_model_command_resolves_to_empty_options_when_no_models_configured() {
    let sources = swissarmyhammer_kanban::builtin_yaml_sources();
    let sources_ref: Vec<(&str, &str)> = sources.iter().map(|(n, c)| (*n, *c)).collect();
    let registry = CommandsRegistry::from_yaml_sources(&sources_ref);

    let impls: HashMap<String, Arc<dyn Command>> = HashMap::new();
    let ui_state = Arc::new(UIState::new());

    // Empty `ai_models` — the resolver still answers, with an empty list.
    let dynamic = DynamicSources::default();

    let scope = vec!["board:my-board".to_string()];
    let opts_registry = default_options_registry();
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        None,
        &ui_state,
        false,
        Some(&dynamic),
        Some(&opts_registry),
    );

    let cmd = find_cmd(&cmds, "ai.model");
    let model_param = cmd
        .params
        .iter()
        .find(|p| p.name == "model")
        .expect("ai.model YAML must declare a `model` param");
    let options = model_param
        .options
        .as_ref()
        .expect("registered resolver always answers Some, even when the answer is empty");
    assert!(
        options.is_empty(),
        "no configured models → an empty option list; got {options:?}"
    );
}
