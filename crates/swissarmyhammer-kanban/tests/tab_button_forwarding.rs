//! Integration coverage for `tab_button` forwarding through
//! `commands_for_scope`.
//!
//! The `command-driven-ui` epic's per-surface `<CommandButton>` consumer
//! (today: the perspective tab bar) filters the emitted command list to
//! commands whose `tab_button` is set. For that filter to work, the
//! [`ResolvedCommand`] emitted by [`commands_for_scope`] must carry the
//! source [`CommandDef::tab_button`] verbatim — a backend-side oversight
//! that drops the field would silently turn the tab-button render path
//! into a no-op.
//!
//! Pins three end-to-end behaviors:
//!
//! 1. A `CommandDef` carrying `tab_button: { icon: "filter" }` survives
//!    emission with the same metadata on the [`ResolvedCommand`].
//! 2. A `CommandDef` with `tab_button: None` (the common case) emits a
//!    [`ResolvedCommand`] with `tab_button: None` — no spurious
//!    defaulting.
//! 3. The forwarding happens for scoped-registry commands AND global
//!    (unscoped) commands — both code paths copy the field.

use std::collections::HashMap;
use std::sync::Arc;

use swissarmyhammer_commands::{Command, CommandsRegistry, UIState};
use swissarmyhammer_kanban::scope_commands::{commands_for_scope, ResolvedCommand};

/// Build a [`CommandsRegistry`] with one or more synthetic commands.
fn registry_with(yaml: &str) -> CommandsRegistry {
    CommandsRegistry::from_yaml_sources(&[("synthetic", yaml)])
}

/// Look up the test's synthetic command in a `commands_for_scope`
/// result.
fn find_cmd<'a>(cmds: &'a [ResolvedCommand], id: &str) -> &'a ResolvedCommand {
    cmds.iter().find(|c| c.id == id).unwrap_or_else(|| {
        panic!(
            "expected `{id}` in emitted commands; got: {:?}",
            cmds.iter().map(|c| &c.id).collect::<Vec<_>>()
        )
    })
}

/// A scoped-registry command carrying `tab_button` survives emission
/// with the icon name intact. This is the path the perspective tab
/// bar's `<CommandButton>` consumes.
#[test]
fn commands_for_scope_forwards_tab_button_on_scoped_registry_command() {
    let yaml = r#"
- id: perspective.focusFilter
  name: Focus filter
  scope: "entity:perspective"
  visible: true
  tab_button:
    icon: filter
"#;
    let registry = registry_with(yaml);
    let impls: HashMap<String, Arc<dyn Command>> = HashMap::new();
    let ui_state = Arc::new(UIState::new());
    let scope = vec!["perspective:01P".to_string()];
    let cmds = commands_for_scope(
        &scope, &registry, &impls, None, &ui_state, false, None, None,
    );
    let cmd = find_cmd(&cmds, "perspective.focusFilter");
    let tab_button = cmd
        .tab_button
        .as_ref()
        .expect("tab_button must be forwarded to ResolvedCommand");
    assert_eq!(tab_button.icon, "filter");
}

/// A scoped-registry command WITHOUT `tab_button` emits with
/// `tab_button: None` — no spurious defaulting in the forwarding code.
#[test]
fn commands_for_scope_leaves_tab_button_none_when_unset() {
    let yaml = r#"
- id: perspective.rename
  name: Rename
  scope: "entity:perspective"
  visible: true
"#;
    let registry = registry_with(yaml);
    let impls: HashMap<String, Arc<dyn Command>> = HashMap::new();
    let ui_state = Arc::new(UIState::new());
    let scope = vec!["perspective:01P".to_string()];
    let cmds = commands_for_scope(
        &scope, &registry, &impls, None, &ui_state, false, None, None,
    );
    let cmd = find_cmd(&cmds, "perspective.rename");
    assert!(
        cmd.tab_button.is_none(),
        "command without tab_button must emit ResolvedCommand.tab_button as None; \
         got: {:?}",
        cmd.tab_button
    );
}

/// A global (unscoped) registry command also forwards its
/// `tab_button` — both `emit_scoped_registry_commands` and
/// `emit_global_registry_commands` copy the field.
#[test]
fn commands_for_scope_forwards_tab_button_on_global_command() {
    let yaml = r#"
- id: app.demo
  name: Demo
  visible: true
  tab_button:
    icon: arrow-up-down
"#;
    let registry = registry_with(yaml);
    let impls: HashMap<String, Arc<dyn Command>> = HashMap::new();
    let ui_state = Arc::new(UIState::new());
    let scope: Vec<String> = vec![];
    let cmds = commands_for_scope(
        &scope, &registry, &impls, None, &ui_state, false, None, None,
    );
    let cmd = find_cmd(&cmds, "app.demo");
    let tab_button = cmd
        .tab_button
        .as_ref()
        .expect("tab_button must be forwarded for global commands");
    assert_eq!(tab_button.icon, "arrow-up-down");
}
