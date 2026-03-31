use std::collections::HashMap;
use std::path::Path;

use crate::context::parse_moniker;
use crate::types::CommandDef;

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
pub fn builtin_yaml_sources() -> Vec<(&'static str, &'static str)> {
    vec![
        ("app", include_str!("../builtin/commands/app.yaml")),
        ("entity", include_str!("../builtin/commands/entity.yaml")),
        ("ui", include_str!("../builtin/commands/ui.yaml")),
        (
            "settings",
            include_str!("../builtin/commands/settings.yaml"),
        ),
        ("file", include_str!("../builtin/commands/file.yaml")),
        ("drag", include_str!("../builtin/commands/drag.yaml")),
    ]
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

    const ENTITY_YAML: &str = r#"
- id: task.add
  name: New Task
  scope: "entity:column"
  undoable: true
  keys:
    cua: Mod+N
    vim: a
  params:
    - name: column
      from: scope_chain
      entity_type: column

- id: task.untag
  name: Remove Tag
  scope: "entity:tag,entity:task"
  undoable: true
  context_menu: true
  keys:
    vim: x
    cua: Delete
  params:
    - name: tag
      from: scope_chain
      entity_type: tag
    - name: task
      from: scope_chain
      entity_type: task
"#;

    #[test]
    fn load_builtin_yaml_files() {
        let registry =
            CommandsRegistry::from_yaml_sources(&[("app", APP_YAML), ("entity", ENTITY_YAML)]);
        assert_eq!(registry.all_commands().len(), 4);
        assert!(registry.get("app.quit").is_some());
        assert!(registry.get("app.undo").is_some());
        assert!(registry.get("task.add").is_some());
        assert!(registry.get("task.untag").is_some());
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
- id: task.add
  name: Create Task
"#;
        let registry = CommandsRegistry::from_yaml_sources(&[
            ("entity", ENTITY_YAML),
            ("override", override_yaml),
        ]);

        let add = registry.get("task.add").unwrap();
        assert_eq!(add.name, "Create Task"); // overridden
        assert_eq!(add.scope.as_deref(), Some("entity:column")); // preserved
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
        assert!(!ids.contains(&"task.add")); // needs column
        assert!(!ids.contains(&"task.untag")); // needs tag + task
    }

    #[test]
    fn available_commands_includes_when_scope_matches() {
        let registry =
            CommandsRegistry::from_yaml_sources(&[("app", APP_YAML), ("entity", ENTITY_YAML)]);

        let scope = vec!["column:todo".to_string()];
        let avail = registry.available_commands(&scope);
        let ids: Vec<&str> = avail.iter().map(|d| d.id.as_str()).collect();
        assert!(ids.contains(&"app.quit")); // global
        assert!(ids.contains(&"task.add")); // column in scope
        assert!(!ids.contains(&"task.untag")); // needs tag + task
    }

    #[test]
    fn available_commands_multi_scope_requirement() {
        let registry = CommandsRegistry::from_yaml_sources(&[("entity", ENTITY_YAML)]);

        // Only tag — not enough for task.untag
        let scope = vec!["tag:bug".to_string()];
        let avail = registry.available_commands(&scope);
        let ids: Vec<&str> = avail.iter().map(|d| d.id.as_str()).collect();
        assert!(!ids.contains(&"task.untag"));

        // Both tag and task — matches
        let scope = vec!["tag:bug".to_string(), "task:01ABC".to_string()];
        let avail = registry.available_commands(&scope);
        let ids: Vec<&str> = avail.iter().map(|d| d.id.as_str()).collect();
        assert!(ids.contains(&"task.untag"));
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
        let app = include_str!("../builtin/commands/app.yaml");
        let entity = include_str!("../builtin/commands/entity.yaml");
        let ui = include_str!("../builtin/commands/ui.yaml");
        let settings = include_str!("../builtin/commands/settings.yaml");

        let registry = CommandsRegistry::from_yaml_sources(&[
            ("app", app),
            ("entity", entity),
            ("ui", ui),
            ("settings", settings),
        ]);

        // app: about, help, quit, command, palette, search, dismiss, undo, redo = 9
        // entity: task.add, task.move, task.delete, task.untag, entity.update_field,
        //         entity.delete, entity.archive, entity.unarchive, tag.update,
        //         column.reorder, attachment.delete,
        //         entity.copy, entity.cut, entity.paste = 14
        // ui: inspect, inspector.close, inspector.close_all, palette.open,
        //     palette.close, view.set, setFocus, window.new = 8
        // settings: keymap.vim, keymap.cua, keymap.emacs = 3
        assert_eq!(registry.all_commands().len(), 34);

        // Spot checks
        assert!(registry.get("app.quit").is_some());
        assert!(registry.get("task.add").is_some());
        assert!(registry.get("ui.palette.open").is_some());
        assert!(registry.get("settings.keymap.vim").is_some());
        assert!(registry.get("task.untag").unwrap().context_menu);
        assert!(registry.get("task.add").unwrap().undoable);
        assert!(!registry.get("app.undo").unwrap().undoable);
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
        let chain = vec!["column:todo".to_string()];
        assert!(scope_matches(Some("entity:column"), &chain));
        assert!(!scope_matches(Some("entity:task"), &chain));
    }

    #[test]
    fn scope_matches_multi() {
        let chain = vec!["tag:bug".to_string(), "task:01ABC".to_string()];
        assert!(scope_matches(Some("entity:tag,entity:task"), &chain));
        assert!(!scope_matches(Some("entity:tag,entity:column"), &chain));
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
        let base = vec![("base", "- id: task.add\n  name: Add Task\n")];
        let mut reg = CommandsRegistry::from_yaml_sources(&base);
        assert!(reg.get("task.add").is_some());
        assert!(reg.get("task.delete").is_none());

        let extra = vec![("extra", "- id: task.delete\n  name: Delete Task\n")];
        reg.merge_yaml_sources(&extra);
        assert!(reg.get("task.delete").is_some());
        assert_eq!(reg.get("task.delete").unwrap().name, "Delete Task");
    }

    #[test]
    fn merge_yaml_sources_overrides_existing_fields() {
        let base = vec![("base", "- id: task.add\n  name: Add Task\n")];
        let mut reg = CommandsRegistry::from_yaml_sources(&base);
        assert_eq!(reg.get("task.add").unwrap().name, "Add Task");

        let over = vec![("over", "- id: task.add\n  name: Add Task Updated\n")];
        reg.merge_yaml_sources(&over);
        assert_eq!(reg.get("task.add").unwrap().name, "Add Task Updated");
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
    fn merge_yaml_sources_invalid_yaml_skipped() {
        let base = vec![("base", "- id: task.add\n  name: Add Task\n")];
        let mut reg = CommandsRegistry::from_yaml_sources(&base);

        let invalid = vec![("bad", "{{{{not valid yaml")];
        reg.merge_yaml_sources(&invalid);
        // Original command still intact
        assert!(reg.get("task.add").is_some());
    }
}
