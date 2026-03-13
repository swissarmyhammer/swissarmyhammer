//! Set up sah for all detected AI coding agents (skills + MCP).
//!
//! Delegates to composable `Initializable` components registered in `super::components`.

use std::time::Instant;

use crate::cli::InstallTarget;
use swissarmyhammer_common::lifecycle::{InitRegistry, InitScope, InitStatus};
use swissarmyhammer_common::reporter::{CliReporter, InitEvent, InitReporter};

use super::components;
use super::settings;

/// Install sah for all detected AI coding agents.
///
/// Creates an `InitRegistry`, registers all components, and runs `init` in
/// priority order. Components that are not applicable to the given scope
/// are automatically skipped.
pub fn install(target: InstallTarget) -> Result<(), String> {
    let reporter = CliReporter;
    let start = Instant::now();
    let scope: InitScope = target.into();
    let global = matches!(target, InstallTarget::User);

    crate::banner::print_banner_stderr();
    reporter.emit(&InitEvent::Header {
        message: format!("Installing for {:?} scope", scope),
    });

    let mut registry = InitRegistry::new();
    components::register_all(&mut registry, global, false);

    let results = registry.run_all_init(&scope, &reporter);

    // Deny built-in Bash tool in Claude Code settings (project-level)
    if matches!(target, InstallTarget::Project | InstallTarget::Local) {
        install_deny_bash(&reporter)?;
    }

    // Install statusline in Claude Code settings
    if matches!(target, InstallTarget::Project | InstallTarget::Local) {
        install_statusline(&reporter)?;
    }

    // Display results and check for errors
    let mut has_errors = false;
    for r in &results {
        match r.status {
            InitStatus::Ok => {} // component already emitted its messages
            InitStatus::Warning => reporter.emit(&InitEvent::Warning {
                message: r.message.clone(),
            }),
            InitStatus::Error => {
                reporter.emit(&InitEvent::Error {
                    message: r.message.clone(),
                });
                has_errors = true;
            }
            InitStatus::Skipped => {} // silent
        }
    }

    reporter.emit(&InitEvent::Finished {
        message: "sah initialization".to_string(),
        elapsed_ms: start.elapsed().as_millis() as u64,
    });

    if has_errors {
        Err("Some components failed to initialize".to_string())
    } else {
        Ok(())
    }
}

/// Install statusline configuration in .claude/settings.json.
fn install_statusline(reporter: &dyn InitReporter) -> Result<(), String> {
    let path = settings::claude_settings_path();
    let mut claude_settings = settings::read_settings(&path)?;
    let changed = settings::merge_statusline(&mut claude_settings);
    settings::write_settings(&path, &claude_settings)?;

    if changed {
        reporter.emit(&InitEvent::Action {
            verb: "Installed".to_string(),
            message: format!("statusline in {}", path.display()),
        });
    }
    Ok(())
}

/// Add "Bash" to permissions.deny in .claude/settings.json.
/// This ensures the agent uses our shell tool instead of the built-in Bash tool.
fn install_deny_bash(reporter: &dyn InitReporter) -> Result<(), String> {
    let path = settings::claude_settings_path();
    let mut claude_settings = settings::read_settings(&path)?;
    let changed = settings::merge_deny_bash(&mut claude_settings);
    settings::write_settings(&path, &claude_settings)?;

    if changed {
        reporter.emit(&InitEvent::Action {
            verb: "Configured".to_string(),
            message: format!(
                "Bash tool denied in {} (use shell tool instead)",
                path.display()
            ),
        });
    }
    Ok(())
}
