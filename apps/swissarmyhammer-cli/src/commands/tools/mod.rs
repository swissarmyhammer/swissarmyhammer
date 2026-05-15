//! Tools management commands — enable/disable MCP tools.

use swissarmyhammer_tools::mcp::tool_config::{
    global_config_path, load_merged_tool_config, load_tool_config_from_path, project_config_path,
    save_tool_config, ToolConfig, ToolEntry, KNOWN_TOOL_NAMES,
};

/// Handle the `sah tools` command.
///
/// Dispatches to the appropriate handler based on the optional subcommand:
/// - `None` → list all tools with their current enable/disable status
/// - `Some(Enable { names })` → enable the named tools (or all if empty)
/// - `Some(Disable { names })` → disable the named tools (or all if empty)
///
/// Returns an exit code: `0` on success, `1` on error.
pub fn handle_command(global: bool, subcommand: Option<crate::cli::ToolsSubcommand>) -> i32 {
    match subcommand {
        None => handle_list(),
        Some(crate::cli::ToolsSubcommand::Enable { names }) => handle_enable(names, global),
        Some(crate::cli::ToolsSubcommand::Disable { names }) => handle_disable(names, global),
    }
}

/// List all known tools with their current enabled/disabled status.
///
/// Reads the merged config (global + project layers) and prints a two-column
/// table showing each tool name and its status.
fn handle_list() -> i32 {
    let config = load_merged_tool_config();

    println!("{:<20} STATUS", "TOOL");
    for name in KNOWN_TOOL_NAMES {
        let enabled = config
            .entries()
            .get(*name)
            .map(|e| e.is_enabled())
            .unwrap_or(true); // default: enabled
        let status = if enabled { "enabled" } else { "disabled" };
        println!("{:<20} {}", name, status);
    }
    0
}

/// Enable the given tools (or all tools if `names` is empty).
///
/// Writes the updated config to the resolved path (global or project).
/// Returns `1` if any name is unknown or the config cannot be saved.
fn handle_enable(names: Vec<String>, global: bool) -> i32 {
    if let Err(e) = validate_tool_names(&names) {
        eprintln!("{}", e);
        return 1;
    }

    let Some(config_path) = resolve_config_path(global) else {
        eprintln!("Could not determine config path");
        return 1;
    };

    let mut config = load_tool_config_for_edit(&config_path);

    if names.is_empty() {
        // Enable all — clearing the map lets every tool revert to its default
        // (enabled), which avoids leaving stale "enabled: true" entries.
        config.entries_mut().clear();
        println!("All tools enabled.");
    } else {
        for name in &names {
            config
                .entries_mut()
                .insert(name.clone(), ToolEntry::new(true));
            println!("{}: enabled", name);
        }
    }

    if let Err(e) = save_tool_config(&config, &config_path) {
        eprintln!("Failed to save config: {}", e);
        return 1;
    }
    0
}

/// Disable the given tools (or all tools if `names` is empty).
///
/// Writes the updated config to the resolved path (global or project).
/// Returns `1` if any name is unknown or the config cannot be saved.
fn handle_disable(names: Vec<String>, global: bool) -> i32 {
    if let Err(e) = validate_tool_names(&names) {
        eprintln!("{}", e);
        return 1;
    }

    let Some(config_path) = resolve_config_path(global) else {
        eprintln!("Could not determine config path");
        return 1;
    };

    let mut config = load_tool_config_for_edit(&config_path);

    if names.is_empty() {
        // Disable all known tools.
        for name in KNOWN_TOOL_NAMES {
            config
                .entries_mut()
                .insert(name.to_string(), ToolEntry::new(false));
        }
        println!("All tools disabled.");
    } else {
        for name in &names {
            config
                .entries_mut()
                .insert(name.clone(), ToolEntry::new(false));
            println!("{}: disabled", name);
        }
    }

    if let Err(e) = save_tool_config(&config, &config_path) {
        eprintln!("Failed to save config: {}", e);
        return 1;
    }
    0
}

/// Validate that all supplied names are in [`KNOWN_TOOL_NAMES`].
///
/// Collects all unrecognised names and returns them in a single error message
/// so the caller can fix multiple mistakes in one pass.
fn validate_tool_names(names: &[String]) -> Result<(), String> {
    let unknown: Vec<_> = names
        .iter()
        .filter(|n| !KNOWN_TOOL_NAMES.contains(&n.as_str()))
        .collect();
    if unknown.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Unknown tool(s): {}. Valid tools: {}",
            unknown
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            KNOWN_TOOL_NAMES.join(", ")
        ))
    }
}

/// Resolve the config file path based on the `--global` flag.
///
/// When `global` is `true`, returns the global path (`~/.sah/tools.yaml`).
/// Otherwise returns the project path (`.sah/tools.yaml` at git root), falling
/// back to the global path when no git root is detected.
fn resolve_config_path(global: bool) -> Option<std::path::PathBuf> {
    if global {
        global_config_path()
    } else {
        project_config_path().or_else(global_config_path)
    }
}

/// Load the tool config from `path` for editing.
///
/// Returns the existing config if the file is present and parseable, or a
/// fresh default config if the file does not exist or cannot be read.
fn load_tool_config_for_edit(path: &std::path::Path) -> ToolConfig {
    load_tool_config_from_path(path).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Helper: build a [`ToolConfig`] from name/enabled pairs.
    fn make_config(entries: &[(&str, bool)]) -> ToolConfig {
        let mut config = ToolConfig::default();
        for (name, enabled) in entries {
            config
                .entries_mut()
                .insert(name.to_string(), ToolEntry::new(*enabled));
        }
        config
    }

    #[test]
    fn test_validate_tool_names_valid() {
        assert!(validate_tool_names(&["shell".to_string(), "git".to_string()]).is_ok());
    }

    #[test]
    fn test_validate_tool_names_empty() {
        assert!(validate_tool_names(&[]).is_ok());
    }

    #[test]
    fn test_validate_tool_names_unknown() {
        let err = validate_tool_names(&["unknown_tool".to_string()]).unwrap_err();
        assert!(err.contains("unknown_tool"));
        assert!(err.contains("Valid tools"));
    }

    #[test]
    fn test_validate_tool_names_multiple_unknowns() {
        // All unknown names should be reported in one error message.
        let err = validate_tool_names(&["bad1".to_string(), "bad2".to_string()]).unwrap_err();
        assert!(err.contains("bad1"));
        assert!(err.contains("bad2"));
        assert!(err.contains("Valid tools"));
    }

    #[test]
    fn test_load_tool_config_for_edit_missing_file() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("nonexistent.yaml");
        let config = load_tool_config_for_edit(&path);
        // Missing file should produce an empty (default) config.
        assert!(config.entries().is_empty());
    }

    #[test]
    fn test_load_tool_config_for_edit_existing_file() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("tools.yaml");
        let original = make_config(&[("shell", false)]);
        save_tool_config(&original, &path).expect("save");

        let loaded = load_tool_config_for_edit(&path);
        assert_eq!(loaded, original);
    }

    #[test]
    fn test_enable_all_clears_map() {
        // Simulate "enable all": the resulting config should be empty so that
        // all tools revert to their default (enabled) state.
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("tools.yaml");

        // Pre-populate with some disabled tools.
        let initial = make_config(&[("shell", false), ("kanban", false)]);
        save_tool_config(&initial, &path).expect("save");

        let mut config = load_tool_config_for_edit(&path);
        config.entries_mut().clear(); // This is what handle_enable does for "enable all"
        save_tool_config(&config, &path).expect("save");

        let reloaded = load_tool_config_for_edit(&path);
        assert!(reloaded.entries().is_empty());
    }

    #[test]
    fn test_disable_all_sets_all_known_tools() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("tools.yaml");

        let mut config = ToolConfig::default();
        for name in KNOWN_TOOL_NAMES {
            config
                .entries_mut()
                .insert(name.to_string(), ToolEntry::new(false));
        }
        save_tool_config(&config, &path).expect("save");

        let reloaded = load_tool_config_for_edit(&path);
        for name in KNOWN_TOOL_NAMES {
            assert!(
                !reloaded.entries()[*name].is_enabled(),
                "Expected {} to be disabled",
                name
            );
        }
    }

    #[test]
    fn test_enable_specific_tools() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("tools.yaml");

        // Start with shell disabled.
        let initial = make_config(&[("shell", false)]);
        save_tool_config(&initial, &path).expect("save");

        // Re-enable shell.
        let mut config = load_tool_config_for_edit(&path);
        config
            .entries_mut()
            .insert("shell".to_string(), ToolEntry::new(true));
        save_tool_config(&config, &path).expect("save");

        let reloaded = load_tool_config_for_edit(&path);
        assert!(reloaded.entries()["shell"].is_enabled());
    }
}
