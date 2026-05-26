//! Set up sah for all detected AI coding agents (skills + MCP).
//!
//! Delegates to composable `Initializable` components registered via the
//! top-level `commands::registry`. The `.sah/` + `.prompts/` workspace
//! structure is created by the root-explicit
//! [`swissarmyhammer_workspace_init`] crate (consumed via the
//! `ProjectStructure` component), so the workspace-setup logic is shared with
//! the kanban-app's in-process board init rather than forked.

use std::time::Instant;

use crate::cli::InstallTarget;
use swissarmyhammer_common::lifecycle::{InitRegistry, InitScope, InitStatus};
use swissarmyhammer_common::reporter::{CliReporter, InitEvent, InitReporter};

/// Install sah for all detected AI coding agents.
///
/// Creates an `InitRegistry`, registers all components via
/// [`crate::commands::registry::register_all`], and runs `init` in
/// priority order. Components that are not applicable to the given scope
/// are automatically skipped.
pub fn install(target: InstallTarget) -> Result<(), String> {
    let reporter = CliReporter;
    let start = Instant::now();
    let scope: InitScope = target.into();

    crate::banner::print_banner_stderr();
    reporter.emit(&InitEvent::Header {
        message: format!("Installing for {:?} scope", scope),
    });

    let mut registry = InitRegistry::new();
    crate::commands::registry::register_all(&mut registry, false);

    let results = registry.run_all_init(&scope, &reporter);

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
