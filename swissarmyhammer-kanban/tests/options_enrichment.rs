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
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use swissarmyhammer_commands::{Command, CommandsRegistry, UIState};
use swissarmyhammer_kanban::commands::options_resolvers::default_options_registry;
use swissarmyhammer_kanban::scope_commands::{
    commands_for_scope, DynamicSources, PerspectiveInfo, ResolvedCommand,
};
use swissarmyhammer_perspectives::PerspectiveFieldInfo;
use swissarmyhammer_views::ViewInfo;

/// Build a [`CommandsRegistry`] with a single synthetic command
/// matching the given YAML — keeps test fixtures small and explicit.
fn registry_with(yaml: &str) -> CommandsRegistry {
    CommandsRegistry::from_yaml_sources(&[("synthetic", yaml)])
}

/// Build a [`DynamicSources`] with one perspective carrying three
/// fields so the `perspective.fields` resolver has data to project.
fn dynamic_with_perspective() -> DynamicSources {
    DynamicSources {
        perspectives: vec![PerspectiveInfo {
            id: "01P".into(),
            name: "Active Sprint".into(),
            view: "grid".into(),
            fields: vec![
                PerspectiveFieldInfo {
                    id: "01F1".into(),
                    display_name: "Title".into(),
                },
                PerspectiveFieldInfo {
                    id: "01F2".into(),
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
    assert_eq!(options[0].value, "01F1");
    assert_eq!(options[0].label, "Title");
    assert_eq!(options[1].value, "01F2");
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
