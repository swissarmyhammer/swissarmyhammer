//! Install and uninstall AVP hooks in Claude Code settings.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

// Re-export InstallTarget from the self-contained cli module.
pub use crate::cli::InstallTarget;

/// Get the settings file path for the given target.
pub fn settings_path(target: InstallTarget) -> PathBuf {
    match target {
        InstallTarget::Project => PathBuf::from(".claude/settings.json"),
        InstallTarget::Local => PathBuf::from(".claude/settings.local.json"),
        InstallTarget::User => dirs::home_dir()
            .expect("Could not find home directory")
            .join(".claude/settings.json"),
    }
}

/// Generate the AVP hooks configuration for all Claude Code hook events.
fn avp_hooks_config() -> Value {
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

    // Add tool-based hooks with matcher
    for event in tool_hooks {
        hooks.insert(
            event.to_string(),
            json!([{
                "matcher": "*",
                "hooks": [{ "type": "command", "command": "avp" }]
            }]),
        );
    }

    // Add simple hooks without matcher
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
fn is_avp_hook(hook: &Value) -> bool {
    // Check if any hook in the hooks array has command "avp"
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
fn merge_hooks(settings: &mut Value, avp_hooks: Value) {
    // Ensure settings["hooks"] exists
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
            // Check if AVP hook already exists
            if let Some(arr) = existing_event_hooks.as_array_mut() {
                let already_installed = arr.iter().any(is_avp_hook);
                if !already_installed {
                    arr.push(avp_hook_entry.clone());
                }
            }
        } else {
            // No existing hooks for this event, add the AVP hook
            settings_hooks.insert(event_name.clone(), avp_event_hooks.clone());
        }
    }
}

/// Remove AVP hooks from settings.
fn remove_hooks(settings: &mut Value) {
    if let Some(hooks) = settings.get_mut("hooks").and_then(|h| h.as_object_mut()) {
        let event_names: Vec<String> = hooks.keys().cloned().collect();

        for event_name in event_names {
            if let Some(event_hooks) = hooks.get_mut(&event_name).and_then(|h| h.as_array_mut()) {
                // Remove AVP hooks
                event_hooks.retain(|hook| !is_avp_hook(hook));

                // If empty, we'll remove the event below
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

/// Install AVP hooks to the specified target.
pub fn install(target: InstallTarget) -> Result<(), String> {
    let path = settings_path(target);

    // Read existing settings or create empty object
    let mut settings: Value = if path.exists() {
        let content = fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))?
    } else {
        json!({})
    };

    // Create parent directory if needed
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory {}: {}", parent.display(), e))?;
    }

    // Merge AVP hooks into settings
    let avp_hooks = avp_hooks_config();
    merge_hooks(&mut settings, avp_hooks);

    // Write back with pretty formatting
    let content = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;
    fs::write(&path, content).map_err(|e| format!("Failed to write {}: {}", path.display(), e))?;

    println!("AVP hooks installed to {}", path.display());

    // For project installs, also create the .avp directory structure
    if matches!(target, InstallTarget::Project | InstallTarget::Local) {
        create_avp_project_structure()?;
    }

    Ok(())
}

/// Create the .avp project directory with README and sample validators.
fn create_avp_project_structure() -> Result<(), String> {
    let avp_dir = PathBuf::from(".avp");
    let validators_dir = avp_dir.join("validators");

    // Create directories
    fs::create_dir_all(&validators_dir)
        .map_err(|e| format!("Failed to create .avp/validators: {}", e))?;

    // Create README.md
    let readme_path = avp_dir.join("README.md");
    if !readme_path.exists() {
        fs::write(&readme_path, AVP_README)
            .map_err(|e| format!("Failed to write .avp/README.md: {}", e))?;
        println!("Created {}", readme_path.display());
    }

    // Create sample validators
    create_sample_validator(
        &validators_dir,
        "file-changes.md",
        SAMPLE_FILE_CHANGES_VALIDATOR,
    )?;
    create_sample_validator(
        &validators_dir,
        "session-summary.md",
        SAMPLE_SESSION_SUMMARY_VALIDATOR,
    )?;

    Ok(())
}

/// Create a sample validator file if it doesn't exist.
fn create_sample_validator(dir: &Path, filename: &str, content: &str) -> Result<(), String> {
    let path = dir.join(filename);
    if !path.exists() {
        fs::write(&path, content)
            .map_err(|e| format!("Failed to write {}: {}", path.display(), e))?;
        println!("Created {}", path.display());
    }
    Ok(())
}

/// README content for the .avp directory.
const AVP_README: &str = r#"# AVP - Agent Validator Protocol

This directory contains validators for Claude Code hooks. Validators are markdown
files with YAML frontmatter that define validation rules.

## Directory Structure

```
.avp/
├── README.md           # This file
├── avp.log             # Hook event log (auto-generated, gitignored)
└── validators/         # Your validator files
    ├── file-changes.md
    └── session-summary.md
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

See the sample validators in this directory for examples.
"#;

/// Sample validator for file changes (PostToolUse on Write/Edit).
const SAMPLE_FILE_CHANGES_VALIDATOR: &str = r#"---
name: file-changes
description: Review file modifications for common issues
severity: warn
trigger: PostToolUse
match:
  tools: [Write, Edit]
---

# File Changes Validator

Review the file changes made by this tool call.

Check for:
1. Syntax errors or obvious bugs
2. Removed code that might still be needed
3. Debug statements or console.log left in
4. TODO comments that should be addressed

If you find issues, explain what you found. Otherwise, confirm the changes look good.
"#;

/// Sample validator for session summary (Stop hook).
const SAMPLE_SESSION_SUMMARY_VALIDATOR: &str = r#"---
name: session-summary
description: Summarize what was accomplished in this response
severity: info
trigger: Stop
---

# Session Summary Validator

Briefly summarize what Claude accomplished in this response.

Note:
- What files were modified
- What the main changes were
- Any pending work mentioned

This is informational only - it helps maintain context across sessions.
"#;

/// Uninstall AVP hooks from the specified target.
pub fn uninstall(target: InstallTarget) -> Result<(), String> {
    let path = settings_path(target);

    if !path.exists() {
        println!(
            "No settings file at {}, nothing to uninstall",
            path.display()
        );
    } else {
        // Read existing settings
        let content = fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
        let mut settings: Value = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))?;

        // Remove AVP hooks
        remove_hooks(&mut settings);

        // If hooks object is now empty, remove it
        if let Some(hooks) = settings.get("hooks").and_then(|h| h.as_object()) {
            if hooks.is_empty() {
                settings.as_object_mut().unwrap().remove("hooks");
            }
        }

        // Write back (or delete if settings is just {})
        if settings.as_object().map(|o| o.is_empty()).unwrap_or(false) {
            fs::remove_file(&path)
                .map_err(|e| format!("Failed to remove {}: {}", path.display(), e))?;
            println!("AVP hooks uninstalled, removed empty {}", path.display());
        } else {
            let content = serde_json::to_string_pretty(&settings)
                .map_err(|e| format!("Failed to serialize settings: {}", e))?;
            fs::write(&path, content)
                .map_err(|e| format!("Failed to write {}: {}", path.display(), e))?;
            println!("AVP hooks uninstalled from {}", path.display());
        }
    }

    // Remove .avp directory if it exists
    let avp_dir = std::env::current_dir()
        .map_err(|e| format!("Failed to get current directory: {}", e))?
        .join(".avp");

    if avp_dir.exists() {
        fs::remove_dir_all(&avp_dir)
            .map_err(|e| format!("Failed to remove {}: {}", avp_dir.display(), e))?;
        println!("Removed {}", avp_dir.display());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_avp_hooks_config_structure() {
        let config = avp_hooks_config();
        let obj = config.as_object().unwrap();

        // Check tool-based hooks have matcher
        assert!(obj.contains_key("PreToolUse"));
        let pre_tool = obj.get("PreToolUse").unwrap().as_array().unwrap();
        assert_eq!(pre_tool[0].get("matcher").unwrap(), "*");

        // Check simple hooks don't have matcher
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

        // Should be identical after second merge
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

        // Should have both the original hook and the AVP hook
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

        // PreToolUse should still exist with just the non-AVP hook
        let pre_tool = hooks.get("PreToolUse").unwrap().as_array().unwrap();
        assert_eq!(pre_tool.len(), 1);
        assert!(!is_avp_hook(&pre_tool[0]));

        // Stop should be removed (was only AVP hook)
        assert!(!hooks.contains_key("Stop"));

        // Other settings preserved
        assert_eq!(settings.get("other_setting").unwrap(), "value");
    }

    #[test]
    fn test_install_uninstall_roundtrip() {
        let temp = TempDir::new().unwrap();
        let settings_file = temp.path().join(".claude/settings.json");

        // Manually set up the path for testing
        std::fs::create_dir_all(settings_file.parent().unwrap()).unwrap();

        // Simulate install
        let mut settings = json!({});
        let avp_hooks = avp_hooks_config();
        merge_hooks(&mut settings, avp_hooks);
        let content = serde_json::to_string_pretty(&settings).unwrap();
        std::fs::write(&settings_file, &content).unwrap();

        // Verify installed
        let installed: Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_file).unwrap()).unwrap();
        assert!(installed.get("hooks").is_some());

        // Simulate uninstall
        let mut settings: Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_file).unwrap()).unwrap();
        remove_hooks(&mut settings);

        // Hooks should be empty (only had AVP hooks)
        let hooks = settings.get("hooks").unwrap().as_object().unwrap();
        assert!(hooks.is_empty());
    }
}
