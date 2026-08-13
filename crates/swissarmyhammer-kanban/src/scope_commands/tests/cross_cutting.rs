//! The pass that puts target-driven commands on every entity moniker.
//!
//! A registry command whose first parameter declares `from: target` comes out
//! on each entity moniker in scope. A per-type opt-in is not necessary. These
//! tests hold that rule. They also hold the entity schemas to carry no
//! `commands:` key.

use super::*;

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
        let cmds = commands_for_scope(
            &scope,
            &registry,
            &impls,
            Some(&fields),
            &ui,
            false,
            None,
            None,
        );
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
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        Some(&fields),
        &ui,
        false,
        None,
        None,
    );

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
    let mut sources = composed_builtin_yaml_sources();
    sources.push(("stub_opt_out", stub_yaml));
    let registry = CommandsRegistry::from_yaml_sources(&sources);
    let mut impls = crate::commands::register_commands();
    impls.insert("stub.opt_out".to_string(), Arc::new(OptOutCmd));

    let defs = crate::defaults::builtin_field_definitions();
    let entities = crate::defaults::builtin_entity_definitions();
    let fields =
        FieldsContext::from_yaml_sources(std::path::PathBuf::from("/tmp/test"), &defs, &entities)
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
    let mut sources = composed_builtin_yaml_sources();
    sources.push(("stub_task_only", stub_yaml));
    let registry = CommandsRegistry::from_yaml_sources(&sources);
    let impls = crate::commands::register_commands();

    let defs = crate::defaults::builtin_field_definitions();
    let entities = crate::defaults::builtin_entity_definitions();
    let fields =
        FieldsContext::from_yaml_sources(std::path::PathBuf::from("/tmp/test"), &defs, &entities)
            .unwrap();
    let ui = Arc::new(UIState::new());

    // Scope chain contains both a task and a tag moniker. The stub must
    // emit on the task moniker and be filtered on the tag moniker.
    let scope = vec!["task:01X".to_string(), "tag:01T".to_string()];
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        Some(&fields),
        &ui,
        false,
        None,
        None,
    );

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
    let mut sources = composed_builtin_yaml_sources();
    sources.push(("stub_cross_cutting_non_target", stub_yaml));
    let registry = CommandsRegistry::from_yaml_sources(&sources);
    let impls = crate::commands::register_commands();

    let defs = crate::defaults::builtin_field_definitions();
    let entities = crate::defaults::builtin_entity_definitions();
    let fields =
        FieldsContext::from_yaml_sources(std::path::PathBuf::from("/tmp/test"), &defs, &entities)
            .unwrap();
    let ui = Arc::new(UIState::new());

    // Task moniker in scope — the cross-cutting pass would, for a
    // qualifying command, emit it with target == Some("task:01X").
    let scope = vec![
        "task:01X".to_string(),
        "column:todo".to_string(),
        "board:main".to_string(),
    ];
    let cmds = commands_for_scope(
        &scope,
        &registry,
        &impls,
        Some(&fields),
        &ui,
        false,
        None,
        None,
    );

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
