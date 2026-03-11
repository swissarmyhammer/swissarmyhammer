//! Set up sah for all detected AI coding agents (skills + MCP).
//!
//! Delegates to composable `Initializable` components registered in `super::components`.

use crate::cli::InstallTarget;
use swissarmyhammer_common::lifecycle::{InitRegistry, InitScope, InitStatus};

use super::components;
use super::settings;

/// Install sah for all detected AI coding agents.
///
/// Creates an `InitRegistry`, registers all components, and runs `init` in
/// priority order. Components that are not applicable to the given scope
/// are automatically skipped.
pub fn install(target: InstallTarget) -> Result<(), String> {
    let scope: InitScope = target.into();
    let global = matches!(target, InstallTarget::User);

    let mut registry = InitRegistry::new();
    components::register_all(&mut registry, global, false);

    let results = registry.run_all_init(&scope);

    // Deny built-in Bash tool in Claude Code settings (project-level)
    if matches!(target, InstallTarget::Project | InstallTarget::Local) {
        install_deny_bash()?;
    }

    // Install statusline in Claude Code settings
    if matches!(target, InstallTarget::Project | InstallTarget::Local) {
        install_statusline()?;
    }

    // Display results and check for errors
    let mut has_errors = false;
    for r in &results {
        match r.status {
            InitStatus::Ok => {} // component already printed its messages
            InitStatus::Warning => eprintln!("Warning: {}", r.message),
            InitStatus::Error => {
                eprintln!("Error: {}", r.message);
                has_errors = true;
            }
            InitStatus::Skipped => {} // silent
        }
    }

    if has_errors {
        Err("Some components failed to initialize".to_string())
    } else {
        Ok(())
    }
}

/// Install statusline configuration in .claude/settings.json.
fn install_statusline() -> Result<(), String> {
    let path = settings::claude_settings_path();
    let mut claude_settings = settings::read_settings(&path)?;
    let changed = settings::merge_statusline(&mut claude_settings);
    settings::write_settings(&path, &claude_settings)?;

    if changed {
        println!("Statusline installed in {}", path.display());
    }
    Ok(())
}

/// Add "Bash" to permissions.deny in .claude/settings.json.
/// This ensures the agent uses our shell tool instead of the built-in Bash tool.
fn install_deny_bash() -> Result<(), String> {
    let path = settings::claude_settings_path();
    let mut claude_settings = settings::read_settings(&path)?;
    let changed = settings::merge_deny_bash(&mut claude_settings);
    settings::write_settings(&path, &claude_settings)?;

    if changed {
        println!(
            "Bash tool denied in {} (use shell tool instead)",
            path.display()
        );
    }
    Ok(())
}
