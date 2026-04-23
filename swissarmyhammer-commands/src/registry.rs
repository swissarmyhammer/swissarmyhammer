use std::collections::HashMap;
use std::path::Path;

use include_dir::{include_dir, Dir};

use crate::context::parse_moniker;
use crate::types::CommandDef;

/// Builtin command YAML files, embedded at compile time.
///
/// Each file in `builtin/commands/` is picked up automatically — adding a new
/// YAML file requires no Rust changes. The source name is the file stem
/// (e.g. `app.yaml` → `"app"`), matching the convention used by
/// `load_yaml_dir` for on-disk overrides.
static BUILTIN_COMMANDS: Dir = include_dir!("$CARGO_MANIFEST_DIR/builtin/commands");

/// Registry of command definitions loaded from YAML sources.
///
/// Supports layered loading: builtins first, then user overrides from
/// `.kanban/commands/`. Later sources override earlier ones by command ID
/// with partial merge (only specified fields replace existing values).
#[derive(Debug)]
pub struct CommandsRegistry {
    commands: HashMap<String, CommandDef>,
}

impl CommandsRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
        }
    }

    /// Build from pre-resolved YAML sources.
    ///
    /// Each source is `(name, yaml_content)`. YAML files contain a list of
    /// command definitions. Later sources override earlier by ID with partial
    /// merge.
    pub fn from_yaml_sources(sources: &[(&str, &str)]) -> Self {
        let mut registry = Self::new();

        for (name, yaml) in sources {
            // Each YAML file is a list of command definitions
            match serde_yaml_ng::from_str::<Vec<serde_yaml_ng::Value>>(yaml) {
                Ok(defs) => {
                    for val in defs {
                        registry.merge_yaml_value(val, name);
                    }
                }
                Err(e) => {
                    tracing::warn!(name = %name, %e, "skipping invalid commands YAML");
                }
            }
        }

        tracing::debug!(
            commands = registry.commands.len(),
            "commands registry built"
        );
        registry
    }

    /// Merge additional YAML sources into the registry.
    ///
    /// Existing commands with matching IDs receive a partial merge (only
    /// fields present in the override replace existing values). New IDs
    /// are inserted as-is.
    pub fn merge_yaml_sources(&mut self, sources: &[(&str, &str)]) {
        for (name, yaml) in sources {
            match serde_yaml_ng::from_str::<Vec<serde_yaml_ng::Value>>(yaml) {
                Ok(defs) => {
                    for val in defs {
                        self.merge_yaml_value(val, name);
                    }
                }
                Err(e) => {
                    tracing::warn!(name = %name, %e, "skipping invalid commands YAML override");
                }
            }
        }
    }

    /// Get a command definition by ID.
    pub fn get(&self, id: &str) -> Option<&CommandDef> {
        self.commands.get(id)
    }

    /// All command definitions.
    pub fn all_commands(&self) -> Vec<&CommandDef> {
        self.commands.values().collect()
    }

    /// Return commands whose scope requirement is satisfied by the given
    /// scope chain. This is a static pre-filter — it does NOT call the
    /// `Command::available()` trait method.
    ///
    /// A command with no scope is always included. A command with
    /// `scope: "entity:tag"` is included only if the scope chain contains
    /// a moniker with entity type `tag`.
    pub fn available_commands(&self, scope_chain: &[String]) -> Vec<&CommandDef> {
        self.commands
            .values()
            .filter(|def| scope_matches(def.scope.as_deref(), scope_chain))
            .collect()
    }

    /// Merge a single YAML value into the registry.
    ///
    /// If a command with the same ID already exists, perform a partial merge:
    /// only fields present in the override replace existing values.
    fn merge_yaml_value(&mut self, val: serde_yaml_ng::Value, source_name: &str) {
        let id = match val.get("id").and_then(|v| v.as_str()) {
            Some(id) => id.to_string(),
            None => {
                tracing::warn!(source = %source_name, "skipping command def without id");
                return;
            }
        };

        if let Some(existing) = self.commands.get(&id) {
            // Partial merge: serialize existing to YAML value, overlay new fields
            let mut base = match serde_yaml_ng::to_value(existing) {
                Ok(serde_yaml_ng::Value::Mapping(m)) => m,
                _ => return,
            };
            if let serde_yaml_ng::Value::Mapping(overlay) = val {
                for (k, v) in overlay {
                    base.insert(k, v);
                }
            }
            match serde_yaml_ng::from_value::<CommandDef>(serde_yaml_ng::Value::Mapping(base)) {
                Ok(merged) => {
                    self.commands.insert(id, merged);
                }
                Err(e) => {
                    tracing::warn!(id = %id, source = %source_name, %e, "failed to merge command override");
                }
            }
        } else {
            // New command — parse directly
            match serde_yaml_ng::from_value::<CommandDef>(val) {
                Ok(def) => {
                    self.commands.insert(id, def);
                }
                Err(e) => {
                    tracing::warn!(id = %id, source = %source_name, %e, "skipping invalid command def");
                }
            }
        }
    }
}

impl Default for CommandsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a scope requirement is satisfied by the scope chain.
///
/// - `None` → always matches (global command)
/// - `Some("entity:tag")` → matches if any moniker has type `tag`
/// - `Some("entity:task,entity:tag")` → matches if ALL listed types present
fn scope_matches(scope: Option<&str>, scope_chain: &[String]) -> bool {
    let scope = match scope {
        None => return true,
        Some("") => return true,
        Some(s) => s,
    };

    // Parse scope requirements — comma-separated "entity:type" patterns
    for requirement in scope.split(',') {
        let requirement = requirement.trim();
        if let Some(entity_type) = requirement.strip_prefix("entity:") {
            let found = scope_chain
                .iter()
                .any(|m| parse_moniker(m).is_some_and(|(t, _)| t == entity_type));
            if !found {
                return false;
            }
        }
    }
    true
}

/// Returns the builtin command YAML sources embedded at compile time.
///
/// Enumerates every `*.yaml` file directly under `builtin/commands/` via
/// `include_dir!` — adding a new builtin command file requires no Rust
/// changes. The source name is the file stem (e.g. `app.yaml` → `"app"`).
///
/// The loader enforces a flat layout: only files whose parent path is the
/// root of the embedded directory are returned. `include_dir!` walks
/// recursively, but keys here are basenames only, so a nested
/// `commands/sub/foo.yaml` would silently shadow `commands/foo.yaml` on
/// `HashMap` insert downstream. Filtering to the root prevents that
/// class of bug at the loader.
pub fn builtin_yaml_sources() -> Vec<(&'static str, &'static str)> {
    BUILTIN_COMMANDS
        .files()
        .filter(|file| file.path().extension().and_then(|e| e.to_str()) == Some("yaml"))
        .filter(|file| file.path().parent() == Some(Path::new("")))
        .filter_map(|file| {
            let name = file.path().file_stem()?.to_str()?;
            let content = file.contents_utf8()?;
            Some((name, content))
        })
        .collect()
}

/// Load YAML files from a directory as `(name, content)` pairs.
///
/// Note: identical copies exist in `swissarmyhammer-fields` and
/// `swissarmyhammer-views`. The function is trivial and the crates are
/// independent (no shared dependency path that avoids a heavy import),
/// so the duplication is intentional.
pub fn load_yaml_dir(dir: &Path) -> Vec<(String, String)> {
    let mut entries = Vec::new();
    if !dir.exists() {
        return entries;
    }
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        return entries;
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
            continue;
        }
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        if let Ok(content) = std::fs::read_to_string(&path) {
            entries.push((name, content));
        }
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    const APP_YAML: &str = r#"
- id: app.quit
  name: Quit
  keys:
    cua: Mod+Q
    vim: ":q"

- id: app.undo
  name: Undo
  undoable: false
  keys:
    cua: Mod+Z
    vim: u
"#;

    // Synthetic test-fixture YAML. The ids (`foo.add`, `foo.remove`) and entity
    // types (`widget`, `gadget`) are deliberately generic placeholders — this
    // crate is consumer-agnostic and must not reference specific domain types
    // (task, column, tag, etc.) even in its own fixtures.
    const ENTITY_YAML: &str = r#"
- id: foo.add
  name: New Foo
  scope: "entity:widget"
  undoable: true
  keys:
    cua: Mod+N
    vim: a
  params:
    - name: widget
      from: scope_chain
      entity_type: widget

- id: foo.remove
  name: Remove Foo
  scope: "entity:widget,entity:gadget"
  undoable: true
  context_menu: true
  keys:
    vim: x
    cua: Delete
  params:
    - name: widget
      from: scope_chain
      entity_type: widget
    - name: gadget
      from: scope_chain
      entity_type: gadget
"#;

    #[test]
    fn load_builtin_yaml_files() {
        let registry =
            CommandsRegistry::from_yaml_sources(&[("app", APP_YAML), ("entity", ENTITY_YAML)]);
        assert_eq!(registry.all_commands().len(), 4);
        assert!(registry.get("app.quit").is_some());
        assert!(registry.get("app.undo").is_some());
        assert!(registry.get("foo.add").is_some());
        assert!(registry.get("foo.remove").is_some());
    }

    #[test]
    fn override_keybinding_preserves_other_fields() {
        let override_yaml = r#"
- id: app.quit
  keys:
    cua: Mod+W
"#;
        let registry =
            CommandsRegistry::from_yaml_sources(&[("app", APP_YAML), ("override", override_yaml)]);

        let quit = registry.get("app.quit").unwrap();
        // Keybinding was overridden
        assert_eq!(quit.keys.as_ref().unwrap().cua.as_deref(), Some("Mod+W"));
        // Name preserved from builtin
        assert_eq!(quit.name, "Quit");
        // Vim key was NOT in the override, but keys is a whole struct replacement
        // so vim is gone from the override's keys block
    }

    #[test]
    fn override_preserves_unspecified_fields() {
        let override_yaml = r#"
- id: foo.add
  name: Create Foo
"#;
        let registry = CommandsRegistry::from_yaml_sources(&[
            ("entity", ENTITY_YAML),
            ("override", override_yaml),
        ]);

        let add = registry.get("foo.add").unwrap();
        assert_eq!(add.name, "Create Foo"); // overridden
        assert_eq!(add.scope.as_deref(), Some("entity:widget")); // preserved
        assert!(add.undoable); // preserved
        assert_eq!(add.params.len(), 1); // preserved
    }

    #[test]
    fn available_commands_filters_by_scope() {
        let registry =
            CommandsRegistry::from_yaml_sources(&[("app", APP_YAML), ("entity", ENTITY_YAML)]);

        // No scope chain — only global commands
        let avail = registry.available_commands(&[]);
        let ids: Vec<&str> = avail.iter().map(|d| d.id.as_str()).collect();
        assert!(ids.contains(&"app.quit"));
        assert!(ids.contains(&"app.undo"));
        assert!(!ids.contains(&"foo.add")); // needs widget
        assert!(!ids.contains(&"foo.remove")); // needs widget + gadget
    }

    #[test]
    fn available_commands_includes_when_scope_matches() {
        let registry =
            CommandsRegistry::from_yaml_sources(&[("app", APP_YAML), ("entity", ENTITY_YAML)]);

        let scope = vec!["widget:42".to_string()];
        let avail = registry.available_commands(&scope);
        let ids: Vec<&str> = avail.iter().map(|d| d.id.as_str()).collect();
        assert!(ids.contains(&"app.quit")); // global
        assert!(ids.contains(&"foo.add")); // widget in scope
        assert!(!ids.contains(&"foo.remove")); // needs widget + gadget
    }

    #[test]
    fn available_commands_multi_scope_requirement() {
        let registry = CommandsRegistry::from_yaml_sources(&[("entity", ENTITY_YAML)]);

        // Only widget — not enough for foo.remove
        let scope = vec!["widget:42".to_string()];
        let avail = registry.available_commands(&scope);
        let ids: Vec<&str> = avail.iter().map(|d| d.id.as_str()).collect();
        assert!(!ids.contains(&"foo.remove"));

        // Both widget and gadget — matches
        let scope = vec!["widget:42".to_string(), "gadget:99".to_string()];
        let avail = registry.available_commands(&scope);
        let ids: Vec<&str> = avail.iter().map(|d| d.id.as_str()).collect();
        assert!(ids.contains(&"foo.remove"));
    }

    #[test]
    fn user_defined_command_loads_alongside_builtins() {
        let user_yaml = r#"
- id: custom.hello
  name: Say Hello
"#;
        let registry =
            CommandsRegistry::from_yaml_sources(&[("app", APP_YAML), ("user", user_yaml)]);

        assert_eq!(registry.all_commands().len(), 3); // 2 app + 1 custom
        assert!(registry.get("custom.hello").is_some());
        assert!(registry.get("app.quit").is_some());
    }

    #[test]
    fn unknown_fields_in_yaml_rejected() {
        let yaml = r#"
- id: app.test
  name: Test
  future_field: some_value
  another_unknown: 42
"#;
        let registry = CommandsRegistry::from_yaml_sources(&[("test", yaml)]);
        // deny_unknown_fields causes this to be skipped with a warning
        assert!(registry.get("app.test").is_none());
    }

    #[test]
    fn invalid_yaml_skipped() {
        let bad = "not valid: [[[";
        let registry = CommandsRegistry::from_yaml_sources(&[("app", APP_YAML), ("bad", bad)]);
        // Should still have the app commands
        assert_eq!(registry.all_commands().len(), 2);
    }

    #[test]
    fn builtin_yaml_files_parse() {
        let sources = builtin_yaml_sources();
        let sources_ref: Vec<(&str, &str)> = sources.iter().map(|(n, c)| (*n, *c)).collect();
        let registry = CommandsRegistry::from_yaml_sources(&sources_ref);

        // This crate is consumer-agnostic — it ships only generic command
        // YAML. Domain-specific commands (task, column, tag, attachment,
        // perspective, file) are contributed by their owning crates and
        // composed at app startup. See
        // `swissarmyhammer_kanban::builtin_yaml_sources` and
        // `swissarmyhammer-kanban/tests/builtin_commands.rs` for the kanban
        // side, which together reach the app-composed total.
        //
        // Per-file breakdown of the generic YAMLs that remain here:
        //   app:      about, help, quit, command, palette, search, dismiss,
        //             undo, redo = 9
        //   entity:   entity.add, entity.update_field, entity.delete,
        //             entity.archive, entity.unarchive, entity.copy,
        //             entity.cut, entity.paste = 8
        //             (task.add and project.add were retired in favor of the
        //             dynamic entity.add:{type} pipeline — see commit
        //             8973cf694.)
        //   ui:       inspect, inspector.close, inspector.close_all,
        //             palette.open, palette.close,
        //             entity.startRename, setFocus, window.new,
        //             mode.set = 9
        //             (`ui.view.set` and `ui.perspective.set` were relocated
        //             to the kanban crate in 01KPY02X405QTP5ACH67THHSN8 —
        //             "view" and "perspective" are kanban concepts, not
        //             generic UI primitives.)
        //   settings: keymap.vim, keymap.cua, keymap.emacs = 3
        //   drag:     start, cancel, complete = 3
        assert_eq!(registry.all_commands().len(), 32);

        // Spot checks — only generic commands remain.
        assert!(registry.get("app.quit").is_some());
        assert!(registry.get("entity.add").is_some());
        assert!(registry.get("ui.palette.open").is_some());
        assert!(registry.get("settings.keymap.vim").is_some());
        assert!(registry.get("entity.add").unwrap().undoable);
        assert!(!registry.get("app.undo").unwrap().undoable);
        assert!(registry.get("drag.start").is_some());
        // Kanban-specific commands must NOT be present — they live in
        // `swissarmyhammer-kanban/builtin/commands/`.
        assert!(registry.get("task.untag").is_none());
        assert!(registry.get("task.move").is_none());
        assert!(registry.get("task.doThisNext").is_none());
        assert!(registry.get("column.reorder").is_none());
        assert!(registry.get("tag.update").is_none());
        assert!(registry.get("attachment.open").is_none());
        assert!(registry.get("attachment.reveal").is_none());
        assert!(registry.get("file.switchBoard").is_none());
        assert!(registry.get("file.closeBoard").is_none());
        assert!(registry.get("file.newBoard").is_none());
        assert!(registry.get("file.openBoard").is_none());
        assert!(registry.get("perspective.load").is_none());
        assert!(registry.get("perspective.goto").is_none());
        // task.add and project.add must NOT be registered — they were
        // replaced by the dynamic entity.add:{type} pipeline.
        assert!(registry.get("task.add").is_none());
        assert!(registry.get("project.add").is_none());
    }

    #[test]
    fn empty_registry() {
        let registry = CommandsRegistry::new();
        assert!(registry.all_commands().is_empty());
        assert!(registry.get("anything").is_none());
        assert!(registry.available_commands(&[]).is_empty());
    }

    #[test]
    fn scope_matches_none_always_true() {
        assert!(scope_matches(None, &[]));
        assert!(scope_matches(Some(""), &[]));
    }

    #[test]
    fn scope_matches_single() {
        let chain = vec!["widget:42".to_string()];
        assert!(scope_matches(Some("entity:widget"), &chain));
        assert!(!scope_matches(Some("entity:gadget"), &chain));
    }

    #[test]
    fn scope_matches_multi() {
        let chain = vec!["widget:42".to_string(), "gadget:99".to_string()];
        assert!(scope_matches(Some("entity:widget,entity:gadget"), &chain));
        assert!(!scope_matches(Some("entity:widget,entity:thing"), &chain));
    }

    // --- load_yaml_dir tests ---

    #[test]
    fn load_yaml_dir_nonexistent_returns_empty() {
        let result = load_yaml_dir(std::path::Path::new("/nonexistent/path/xyz"));
        assert!(result.is_empty());
    }

    #[test]
    fn load_yaml_dir_with_yaml_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("foo.yaml"), "- id: foo\n").unwrap();
        std::fs::write(dir.path().join("bar.yaml"), "- id: bar\n").unwrap();
        let result = load_yaml_dir(dir.path());
        assert_eq!(result.len(), 2);
        let names: Vec<&str> = result.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"foo"));
        assert!(names.contains(&"bar"));
    }

    #[test]
    fn load_yaml_dir_skips_non_yaml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("commands.yaml"), "- id: test\n").unwrap();
        std::fs::write(dir.path().join("readme.txt"), "not yaml").unwrap();
        std::fs::write(dir.path().join("data.json"), "{}").unwrap();
        let result = load_yaml_dir(dir.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "commands");
    }

    #[test]
    fn load_yaml_dir_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let result = load_yaml_dir(dir.path());
        assert!(result.is_empty());
    }

    // --- merge_yaml_sources tests ---

    #[test]
    fn merge_yaml_sources_adds_new_commands() {
        // `foo.add` / `foo.remove` are synthetic inline strings used here to
        // exercise the merge logic — they're generic placeholders and do not
        // correspond to any real builtin commands.
        let base = vec![("base", "- id: foo.add\n  name: Add Foo\n")];
        let mut reg = CommandsRegistry::from_yaml_sources(&base);
        assert!(reg.get("foo.add").is_some());
        assert!(reg.get("foo.remove").is_none());

        let extra = vec![("extra", "- id: foo.remove\n  name: Remove Foo\n")];
        reg.merge_yaml_sources(&extra);
        assert!(reg.get("foo.remove").is_some());
        assert_eq!(reg.get("foo.remove").unwrap().name, "Remove Foo");
    }

    #[test]
    fn merge_yaml_sources_overrides_existing_fields() {
        let base = vec![("base", "- id: foo.add\n  name: Add Foo\n")];
        let mut reg = CommandsRegistry::from_yaml_sources(&base);
        assert_eq!(reg.get("foo.add").unwrap().name, "Add Foo");

        let over = vec![("over", "- id: foo.add\n  name: Add Foo Updated\n")];
        reg.merge_yaml_sources(&over);
        assert_eq!(reg.get("foo.add").unwrap().name, "Add Foo Updated");
    }

    #[test]
    fn keymap_commands_are_visible_in_palette() {
        let settings = include_str!("../builtin/commands/settings.yaml");
        let registry = CommandsRegistry::from_yaml_sources(&[("settings", settings)]);

        for cmd_id in &[
            "settings.keymap.vim",
            "settings.keymap.cua",
            "settings.keymap.emacs",
        ] {
            let cmd = registry
                .get(cmd_id)
                .unwrap_or_else(|| panic!("{cmd_id} missing"));
            assert!(
                cmd.visible,
                "{cmd_id} should be visible in the command palette"
            );
        }
    }

    #[test]
    fn ui_yaml_arg_only_commands_are_hidden_from_palette() {
        // Hygiene test: any `ui.*` command whose params come `from: args`
        // cannot be invoked from the command palette (the palette has no UI
        // for collecting arbitrary args), so it must be marked
        // `visible: false` in ui.yaml. User-facing palette entries for those
        // operations are synthesized elsewhere (e.g. `view.switch:{id}` in
        // `scope_commands::emit_view_switch`, rewritten back to `view.set`
        // by the dispatcher — `view.set` lives in
        // `swissarmyhammer-kanban/builtin/commands/view.yaml` because
        // "view" is a kanban concept, not a generic UI primitive).
        //
        // See task 01KPTHX6J2K28GMMV6YQVJWYCE. `ui.view.set` and
        // `ui.perspective.set` were relocated to the kanban crate in
        // 01KPY02X405QTP5ACH67THHSN8 and are covered by the kanban-side
        // `view_set_and_perspective_set_registered_hidden` integration test.
        let ui = include_str!("../builtin/commands/ui.yaml");
        let registry = CommandsRegistry::from_yaml_sources(&[("ui", ui)]);

        // Commands that must be hidden from the palette.
        let hidden = ["ui.mode.set", "ui.palette.close", "ui.setFocus"];
        for cmd_id in &hidden {
            let cmd = registry
                .get(cmd_id)
                .unwrap_or_else(|| panic!("{cmd_id} missing from ui.yaml"));
            assert!(
                !cmd.visible,
                "{cmd_id} requires args the palette cannot provide — \
                 the command must be `visible: false`. See ui.yaml."
            );
        }

        // Commands that are user-facing and must remain visible.
        let visible = [
            "ui.inspect",
            "ui.inspector.close",
            "ui.inspector.close_all",
            "ui.palette.open",
            "ui.entity.startRename",
        ];
        for cmd_id in &visible {
            let cmd = registry
                .get(cmd_id)
                .unwrap_or_else(|| panic!("{cmd_id} missing from ui.yaml"));
            assert!(
                cmd.visible,
                "{cmd_id} is a user-facing palette entry and must remain \
                 visible. See ui.yaml."
            );
        }
    }

    #[test]
    fn merge_yaml_sources_invalid_yaml_skipped() {
        let base = vec![("base", "- id: foo.add\n  name: Add Foo\n")];
        let mut reg = CommandsRegistry::from_yaml_sources(&base);

        let invalid = vec![("bad", "{{{{not valid yaml")];
        reg.merge_yaml_sources(&invalid);
        // Original command still intact
        assert!(reg.get("foo.add").is_some());
    }

    // --- merge_yaml_value edge cases ---

    #[test]
    fn merge_yaml_value_skips_entry_without_id() {
        // A YAML entry missing the `id` field should be silently skipped.
        let yaml = r#"
- name: No ID Command
  keys:
    cua: Mod+X
"#;
        let registry = CommandsRegistry::from_yaml_sources(&[("test", yaml)]);
        assert!(registry.all_commands().is_empty());
    }

    #[test]
    fn merge_yaml_value_skips_invalid_override_on_existing() {
        // When an override introduces an unknown field on an existing command,
        // the merge deserialization should fail and the original remains.
        let base_yaml = r#"
- id: app.test
  name: Test Command
"#;
        let override_yaml = r#"
- id: app.test
  name: Updated
  unknown_field_that_breaks: true
"#;
        let registry = CommandsRegistry::from_yaml_sources(&[
            ("base", base_yaml),
            ("override", override_yaml),
        ]);
        // The merge fails due to deny_unknown_fields, so the original is kept
        let cmd = registry.get("app.test").unwrap();
        assert_eq!(cmd.name, "Test Command");
    }

    #[test]
    fn from_yaml_sources_empty_sources() {
        // An empty sources slice produces an empty registry.
        let registry = CommandsRegistry::from_yaml_sources(&[]);
        assert!(registry.all_commands().is_empty());
    }

    #[test]
    fn from_yaml_sources_empty_yaml_list() {
        // A YAML source that parses to an empty list adds no commands.
        let registry = CommandsRegistry::from_yaml_sources(&[("empty", "[]")]);
        assert!(registry.all_commands().is_empty());
    }

    #[test]
    fn merge_yaml_sources_multiple_sources_at_once() {
        // Verify that merge_yaml_sources handles multiple sources in a single call.
        let mut reg = CommandsRegistry::new();
        let sources: Vec<(&str, &str)> = vec![
            ("a", "- id: cmd.a\n  name: A\n"),
            ("b", "- id: cmd.b\n  name: B\n"),
        ];
        reg.merge_yaml_sources(&sources);
        assert_eq!(reg.all_commands().len(), 2);
        assert!(reg.get("cmd.a").is_some());
        assert!(reg.get("cmd.b").is_some());
    }

    #[test]
    fn all_commands_returns_all_registered() {
        // Verify all_commands returns every command, regardless of scope.
        let yaml = r#"
- id: global.cmd
  name: Global

- id: scoped.cmd
  name: Scoped
  scope: "entity:widget"
"#;
        let registry = CommandsRegistry::from_yaml_sources(&[("test", yaml)]);
        let all = registry.all_commands();
        assert_eq!(all.len(), 2);
        let ids: Vec<&str> = all.iter().map(|c| c.id.as_str()).collect();
        assert!(ids.contains(&"global.cmd"));
        assert!(ids.contains(&"scoped.cmd"));
    }

    #[test]
    fn load_yaml_dir_reads_file_content() {
        // Verify that load_yaml_dir reads actual file content, not just names.
        let dir = tempfile::tempdir().unwrap();
        let content = "- id: loaded.cmd\n  name: Loaded Command\n";
        std::fs::write(dir.path().join("commands.yaml"), content).unwrap();
        let result = load_yaml_dir(dir.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "commands");
        assert_eq!(result[0].1, content);
    }

    #[test]
    fn load_yaml_dir_then_merge_into_registry() {
        // End-to-end: load YAML files from a directory and merge into a registry.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("custom.yaml"),
            "- id: custom.greet\n  name: Greet\n",
        )
        .unwrap();

        let base_yaml = "- id: app.quit\n  name: Quit\n";
        let mut registry = CommandsRegistry::from_yaml_sources(&[("app", base_yaml)]);

        let dir_sources = load_yaml_dir(dir.path());
        let refs: Vec<(&str, &str)> = dir_sources
            .iter()
            .map(|(n, c)| (n.as_str(), c.as_str()))
            .collect();
        registry.merge_yaml_sources(&refs);

        assert_eq!(registry.all_commands().len(), 2);
        assert!(registry.get("app.quit").is_some());
        assert!(registry.get("custom.greet").is_some());
    }

    #[test]
    fn default_creates_empty_registry() {
        // Verify the Default impl produces an empty registry.
        let registry = CommandsRegistry::default();
        assert!(registry.all_commands().is_empty());
    }

    #[test]
    fn merge_yaml_value_override_with_invalid_merged_result_preserves_original() {
        // Start with a valid command, then overlay an override that makes the
        // merged result invalid (e.g., wrong type for a field).
        let base = r#"
- id: foo.add
  name: Add Foo
  undoable: true
"#;
        let override_yaml = r#"
- id: foo.add
  undoable: not_a_bool
"#;
        let registry =
            CommandsRegistry::from_yaml_sources(&[("base", base), ("override", override_yaml)]);
        let cmd = registry.get("foo.add");
        if let Some(cmd) = cmd {
            assert_eq!(cmd.name, "Add Foo");
        }
    }

    #[test]
    fn merge_yaml_sources_override_adds_new_via_merge() {
        let base = vec![("base", "- id: app.quit\n  name: Quit\n  undoable: false\n")];
        let mut reg = CommandsRegistry::from_yaml_sources(&base);

        let over = vec![("over", "- id: app.quit\n  name: Quit App\n")];
        reg.merge_yaml_sources(&over);
        let cmd = reg.get("app.quit").unwrap();
        assert_eq!(cmd.name, "Quit App");
        assert!(!cmd.undoable);
    }

    // --- scope_matches edge cases ---

    #[test]
    fn scope_matches_non_entity_requirement_is_ignored() {
        let chain = vec!["task:01ABC".to_string()];
        assert!(scope_matches(Some("custom_scope"), &chain));
    }
}
