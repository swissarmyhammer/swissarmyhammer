//! Shared AVP install/uninstall logic.
//!
//! This module contains the core logic for installing and removing AVP hooks
//! in Claude Code settings files. It is used by both `avp init` and `sah init`.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};
use swissarmyhammer_common::lifecycle::InitScope;

/// Map an `InitScope` to the Claude Code settings file path.
///
/// Returns an error if `scope` is `User` and the home directory cannot be determined.
pub fn settings_path(scope: InitScope) -> Result<PathBuf, String> {
    match scope {
        InitScope::Project => Ok(PathBuf::from(".claude/settings.json")),
        InitScope::Local => Ok(PathBuf::from(".claude/settings.local.json")),
        InitScope::User => dirs::home_dir()
            .map(|h| h.join(".claude/settings.json"))
            .ok_or_else(|| "Could not find home directory".to_string()),
    }
}

/// Generate the AVP hooks configuration for all Claude Code hook events.
///
/// Returns a JSON object keyed by event name (e.g. `"PreToolUse"`, `"Stop"`).
/// Tool-based events include a `"matcher": "*"` field; simple events do not.
pub fn avp_hooks_config() -> Value {
    // Hook events that require a matcher (tool-based hooks)
    let tool_hooks = [
        "PreToolUse",
        "PostToolUse",
        "PostToolUseFailure",
        "PermissionRequest",
    ];

    // Hook events that don't use a matcher
    let simple_hooks = [
        "SessionStart",
        "UserPromptSubmit",
        "Stop",
        "SubagentStop",
        "SessionEnd",
        "Notification",
        "PreCompact",
        "Setup",
        "SubagentStart",
    ];

    let mut hooks = Map::new();

    for event in tool_hooks {
        hooks.insert(
            event.to_string(),
            json!([{
                "matcher": "*",
                "hooks": [{ "type": "command", "command": "avp" }]
            }]),
        );
    }

    for event in simple_hooks {
        hooks.insert(
            event.to_string(),
            json!([{
                "hooks": [{ "type": "command", "command": "avp" }]
            }]),
        );
    }

    Value::Object(hooks)
}

/// Check if a hook entry is an AVP hook.
///
/// Returns `true` if any entry in the `"hooks"` array has `"command"` equal to
/// `"avp"` or ending with `"/avp"` (to match full-path invocations).
pub fn is_avp_hook(hook: &Value) -> bool {
    if let Some(hooks_array) = hook.get("hooks").and_then(|h| h.as_array()) {
        for h in hooks_array {
            if let Some(cmd) = h.get("command").and_then(|c| c.as_str()) {
                if cmd == "avp" || cmd.ends_with("/avp") {
                    return true;
                }
            }
        }
    }
    false
}

/// Merge AVP hooks into existing settings (idempotent).
///
/// `settings` must be a JSON object. `avp_hooks` should be the output of
/// [`avp_hooks_config`]. Existing non-AVP hooks are preserved. Calling this
/// multiple times with the same inputs produces the same result.
///
/// # Panics
///
/// Panics if `settings` is not a JSON object. Callers should validate first
/// (see [`install`] which checks this before calling).
pub fn merge_hooks(settings: &mut Value, avp_hooks: Value) {
    if settings.get("hooks").is_none() {
        settings
            .as_object_mut()
            .unwrap()
            .insert("hooks".to_string(), json!({}));
    }

    let settings_hooks = settings.get_mut("hooks").unwrap().as_object_mut().unwrap();
    let avp_hooks_obj = avp_hooks.as_object().unwrap();

    for (event_name, avp_event_hooks) in avp_hooks_obj {
        let avp_hook_entry = avp_event_hooks.as_array().unwrap().first().unwrap();

        if let Some(existing_event_hooks) = settings_hooks.get_mut(event_name) {
            if let Some(arr) = existing_event_hooks.as_array_mut() {
                let already_installed = arr.iter().any(is_avp_hook);
                if !already_installed {
                    arr.push(avp_hook_entry.clone());
                }
            }
        } else {
            settings_hooks.insert(event_name.clone(), avp_event_hooks.clone());
        }
    }
}

/// Remove AVP hooks from settings.
///
/// Removes all hook entries where [`is_avp_hook`] returns `true`, and cleans up
/// empty event arrays. Does **not** remove the top-level `"hooks"` key even if
/// empty — callers should handle that (see [`uninstall`]).
pub fn remove_hooks(settings: &mut Value) {
    if let Some(hooks) = settings.get_mut("hooks").and_then(|h| h.as_object_mut()) {
        let event_names: Vec<String> = hooks.keys().cloned().collect();

        for event_name in event_names {
            if let Some(event_hooks) = hooks.get_mut(&event_name).and_then(|h| h.as_array_mut()) {
                event_hooks.retain(|hook| !is_avp_hook(hook));
            }
        }

        // Remove empty event arrays
        hooks.retain(|_, v| {
            if let Some(arr) = v.as_array() {
                !arr.is_empty()
            } else {
                true
            }
        });
    }
}

/// Install AVP hooks into the settings file for the given scope.
///
/// This is the core install logic shared by both `avp init` and `sah init`.
/// Pass a `base_dir` to control where relative paths resolve (for project/local scopes).
pub fn install(scope: InitScope, base_dir: &Path) -> Result<(), String> {
    let path = if matches!(scope, InitScope::User) {
        settings_path(scope)?
    } else {
        base_dir.join(settings_path(scope)?)
    };

    // Create parent directory first so both read and write have a valid path.
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory {}: {}", parent.display(), e))?;
    }

    let mut settings: Value = if path.exists() {
        let content = fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
        let parsed: Value = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))?;
        if !parsed.is_object() {
            return Err(format!("{} is not a JSON object", path.display()));
        }
        parsed
    } else {
        json!({})
    };

    let avp_hooks = avp_hooks_config();
    merge_hooks(&mut settings, avp_hooks);

    let content = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;
    fs::write(&path, content).map_err(|e| format!("Failed to write {}: {}", path.display(), e))?;

    tracing::info!("AVP hooks installed to {}", path.display());

    if matches!(scope, InitScope::Project | InitScope::Local) {
        create_avp_project_structure(base_dir)?;
    }

    Ok(())
}

/// Create the .avp project directory with README.
pub fn create_avp_project_structure(base_dir: &Path) -> Result<(), String> {
    let avp_dir = base_dir.join(".avp");
    let validators_dir = avp_dir.join("validators");

    fs::create_dir_all(&validators_dir)
        .map_err(|e| format!("Failed to create .avp/validators: {}", e))?;

    let readme_path = avp_dir.join("README.md");
    if !readme_path.exists() {
        fs::write(&readme_path, AVP_README)
            .map_err(|e| format!("Failed to write .avp/README.md: {}", e))?;
        tracing::info!("Created {}", readme_path.display());
    }

    Ok(())
}

/// Uninstall AVP hooks from the settings file for the given scope.
pub fn uninstall(scope: InitScope, base_dir: &Path) -> Result<(), String> {
    let path = if matches!(scope, InitScope::User) {
        settings_path(scope)?
    } else {
        base_dir.join(settings_path(scope)?)
    };

    if !path.exists() {
        tracing::info!(
            "No settings file at {}, nothing to uninstall",
            path.display()
        );
    } else {
        let content = fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
        let mut settings: Value = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))?;

        remove_hooks(&mut settings);

        if let Some(hooks) = settings.get("hooks").and_then(|h| h.as_object()) {
            if hooks.is_empty() {
                settings.as_object_mut().unwrap().remove("hooks");
            }
        }

        if settings.as_object().map(|o| o.is_empty()).unwrap_or(false) {
            fs::remove_file(&path)
                .map_err(|e| format!("Failed to remove {}: {}", path.display(), e))?;
            tracing::info!("AVP hooks uninstalled, removed empty {}", path.display());
        } else {
            let content = serde_json::to_string_pretty(&settings)
                .map_err(|e| format!("Failed to serialize settings: {}", e))?;
            fs::write(&path, content)
                .map_err(|e| format!("Failed to write {}: {}", path.display(), e))?;
            tracing::info!("AVP hooks uninstalled from {}", path.display());
        }
    }

    // Only remove .avp directory for project-scoped uninstalls (mirrors install behavior).
    if matches!(scope, InitScope::Project | InitScope::Local) {
        let avp_dir = base_dir.join(".avp");
        if avp_dir.exists() {
            fs::remove_dir_all(&avp_dir)
                .map_err(|e| format!("Failed to remove {}: {}", avp_dir.display(), e))?;
            tracing::info!("Removed {}", avp_dir.display());
        }
    }

    Ok(())
}

/// Default README content written to `.avp/README.md` during project setup.
pub const AVP_README: &str = r#"# AVP - Agent Validator Protocol

This directory contains validators for Claude Code hooks. Validators are markdown
files with YAML frontmatter that define validation rules.

## Directory Structure

```
.avp/
├── README.md           # This file
├── log                 # Hook event log (auto-generated, gitignored)
└── validators/         # Your validator files
```

## Validator Format

Validators use markdown with YAML frontmatter:

```markdown
---
name: validator-name
description: What this validator does
severity: warn          # info, warn, or error (error = blocking)
trigger: PostToolUse    # Hook event that triggers this validator
match:                  # Optional: filter which events trigger this
  tools: [Write, Edit]  # Tool names (regex patterns)
  files: ["*.ts"]       # File globs
---

# Instructions for the validator

Describe what the validator should check and how it should respond.
```

## Triggers

- `PreToolUse` - Before a tool runs (can block)
- `PostToolUse` - After a tool succeeds
- `PostToolUseFailure` - After a tool fails
- `Stop` - When Claude finishes responding
- `SessionStart` - When a session begins
- `SessionEnd` - When a session ends
- `UserPromptSubmit` - When user submits a prompt

## Severity Levels

- `info` - Informational, logged but never blocks
- `warn` - Warning, logged but doesn't block (default)
- `error` - Error, blocks the action if validation fails

## More Information

See the AVP documentation for examples.
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_avp_hooks_config_structure() {
        let config = avp_hooks_config();
        let obj = config.as_object().unwrap();

        // Tool-based hooks have matcher
        assert!(obj.contains_key("PreToolUse"));
        let pre_tool = obj.get("PreToolUse").unwrap().as_array().unwrap();
        assert_eq!(pre_tool[0].get("matcher").unwrap(), "*");

        // Simple hooks don't have matcher
        assert!(obj.contains_key("Stop"));
        let stop = obj.get("Stop").unwrap().as_array().unwrap();
        assert!(stop[0].get("matcher").is_none());
    }

    #[test]
    fn test_is_avp_hook() {
        let avp_hook = json!({
            "matcher": "*",
            "hooks": [{ "type": "command", "command": "avp" }]
        });
        assert!(is_avp_hook(&avp_hook));

        let other_hook = json!({
            "matcher": "*",
            "hooks": [{ "type": "command", "command": "other-tool" }]
        });
        assert!(!is_avp_hook(&other_hook));

        let full_path_hook = json!({
            "hooks": [{ "type": "command", "command": "/usr/local/bin/avp" }]
        });
        assert!(is_avp_hook(&full_path_hook));
    }

    #[test]
    fn test_merge_hooks_empty() {
        let mut settings = json!({});
        let avp_hooks = avp_hooks_config();
        merge_hooks(&mut settings, avp_hooks);

        assert!(settings.get("hooks").is_some());
        let hooks = settings.get("hooks").unwrap().as_object().unwrap();
        assert!(hooks.contains_key("PreToolUse"));
        assert!(hooks.contains_key("Stop"));
    }

    #[test]
    fn test_merge_hooks_idempotent() {
        let mut settings = json!({});
        let avp_hooks = avp_hooks_config();

        merge_hooks(&mut settings, avp_hooks.clone());
        let first = settings.clone();

        merge_hooks(&mut settings, avp_hooks);
        assert_eq!(settings, first);
    }

    #[test]
    fn test_merge_hooks_preserves_existing() {
        let mut settings = json!({
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "Bash",
                        "hooks": [{ "type": "command", "command": "my-script.sh" }]
                    }
                ]
            }
        });

        let avp_hooks = avp_hooks_config();
        merge_hooks(&mut settings, avp_hooks);

        let pre_tool = settings
            .get("hooks")
            .unwrap()
            .get("PreToolUse")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(pre_tool.len(), 2);
    }

    #[test]
    fn test_remove_hooks() {
        let mut settings = json!({
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "*",
                        "hooks": [{ "type": "command", "command": "avp" }]
                    },
                    {
                        "matcher": "Bash",
                        "hooks": [{ "type": "command", "command": "my-script.sh" }]
                    }
                ],
                "Stop": [
                    {
                        "hooks": [{ "type": "command", "command": "avp" }]
                    }
                ]
            },
            "other_setting": "value"
        });

        remove_hooks(&mut settings);

        let hooks = settings.get("hooks").unwrap().as_object().unwrap();
        let pre_tool = hooks.get("PreToolUse").unwrap().as_array().unwrap();
        assert_eq!(pre_tool.len(), 1);
        assert!(!is_avp_hook(&pre_tool[0]));
        assert!(!hooks.contains_key("Stop"));
        assert_eq!(settings.get("other_setting").unwrap(), "value");
    }

    #[test]
    fn test_install_creates_hooks_and_avp_dir() {
        let temp = TempDir::new().unwrap();
        install(InitScope::Project, temp.path()).unwrap();

        // Settings file should exist with hooks
        let settings_file = temp.path().join(".claude/settings.json");
        assert!(settings_file.exists());
        let content: Value =
            serde_json::from_str(&fs::read_to_string(&settings_file).unwrap()).unwrap();
        assert!(content.get("hooks").is_some());

        // .avp directory structure should exist
        assert!(temp.path().join(".avp/validators").is_dir());
        assert!(temp.path().join(".avp/README.md").exists());
    }

    #[test]
    fn test_install_idempotent() {
        let temp = TempDir::new().unwrap();
        install(InitScope::Project, temp.path()).unwrap();
        let first = fs::read_to_string(temp.path().join(".claude/settings.json")).unwrap();

        install(InitScope::Project, temp.path()).unwrap();
        let second = fs::read_to_string(temp.path().join(".claude/settings.json")).unwrap();

        assert_eq!(first, second);
    }

    #[test]
    fn test_uninstall_removes_hooks_and_avp_dir() {
        let temp = TempDir::new().unwrap();
        install(InitScope::Project, temp.path()).unwrap();
        uninstall(InitScope::Project, temp.path()).unwrap();

        // Settings file removed (was empty after hook removal)
        assert!(!temp.path().join(".claude/settings.json").exists());
        // .avp directory removed
        assert!(!temp.path().join(".avp").exists());
    }

    #[test]
    fn test_install_uninstall_preserves_other_settings() {
        let temp = TempDir::new().unwrap();
        let settings_file = temp.path().join(".claude/settings.json");
        fs::create_dir_all(settings_file.parent().unwrap()).unwrap();
        fs::write(
            &settings_file,
            serde_json::to_string_pretty(&json!({"other": "value"})).unwrap(),
        )
        .unwrap();

        install(InitScope::Project, temp.path()).unwrap();
        uninstall(InitScope::Project, temp.path()).unwrap();

        // Settings file should still exist with the other setting
        assert!(settings_file.exists());
        let content: Value =
            serde_json::from_str(&fs::read_to_string(&settings_file).unwrap()).unwrap();
        assert_eq!(content.get("other").unwrap(), "value");
        assert!(content.get("hooks").is_none());
    }

    #[test]
    fn test_settings_path_project() {
        assert_eq!(
            settings_path(InitScope::Project).unwrap(),
            PathBuf::from(".claude/settings.json")
        );
    }

    #[test]
    fn test_settings_path_local() {
        assert_eq!(
            settings_path(InitScope::Local).unwrap(),
            PathBuf::from(".claude/settings.local.json")
        );
    }

    #[test]
    fn test_settings_path_user_returns_result() {
        // Should return Ok with a path ending in .claude/settings.json, not panic
        let result = settings_path(InitScope::User);
        assert!(result.is_ok());
        assert!(result.unwrap().ends_with(".claude/settings.json"));
    }

    #[test]
    fn test_install_with_non_object_settings_json() {
        let temp = TempDir::new().unwrap();
        let settings_file = temp.path().join(".claude/settings.json");
        fs::create_dir_all(settings_file.parent().unwrap()).unwrap();
        // Write a JSON array instead of an object
        fs::write(&settings_file, "[]").unwrap();

        let result = install(InitScope::Project, temp.path());
        assert!(
            result.is_err(),
            "install should return Err for non-object settings JSON"
        );
        assert!(result.unwrap_err().contains("not a JSON object"));
    }

    #[test]
    fn test_uninstall_user_scope_does_not_remove_avp_dir() {
        let temp = TempDir::new().unwrap();
        // Create a .avp directory (as if project-level install happened)
        let avp_dir = temp.path().join(".avp");
        fs::create_dir_all(avp_dir.join("validators")).unwrap();

        // Uninstall at User scope should NOT remove the project's .avp directory
        // (User scope only touches ~/.claude/settings.json)
        let _ = uninstall(InitScope::User, temp.path());

        assert!(
            avp_dir.exists(),
            ".avp directory should not be removed for User scope uninstall"
        );
    }

    #[test]
    fn test_install_local_scope() {
        let temp = TempDir::new().unwrap();
        install(InitScope::Local, temp.path()).unwrap();

        // Local scope uses settings.local.json
        let settings_file = temp.path().join(".claude/settings.local.json");
        assert!(settings_file.exists());
        let content: Value =
            serde_json::from_str(&fs::read_to_string(&settings_file).unwrap()).unwrap();
        assert!(content.get("hooks").is_some());

        // .avp directory structure should also be created for Local scope
        assert!(temp.path().join(".avp/validators").is_dir());
        assert!(temp.path().join(".avp/README.md").exists());
    }
}
