//! Remove sah from all detected AI coding agents (skills + MCP).
//!
//! Delegates to composable `Initializable` components registered via the
//! top-level `commands::registry`.

use std::time::Instant;

use crate::cli::InstallTarget;
use swissarmyhammer_common::lifecycle::{InitRegistry, InitScope, InitStatus};
use swissarmyhammer_common::reporter::{CliReporter, InitEvent, InitReporter};

/// Uninstall sah from all detected AI coding agents.
///
/// Creates an `InitRegistry`, registers all components via
/// [`crate::commands::registry::register_all`], and runs `deinit` in
/// reverse priority order. The `remove_directory` flag controls whether
/// `ProjectStructure` removes `.sah/` and `.prompts/`.
pub fn uninstall(target: InstallTarget, remove_directory: bool) -> Result<(), String> {
    let reporter = CliReporter;
    let start = Instant::now();
    let scope: InitScope = target.into();

    crate::banner::print_banner_stderr();
    reporter.emit(&InitEvent::Header {
        message: format!("Removing for {:?} scope", scope),
    });

    let mut registry = InitRegistry::new();
    crate::commands::registry::register_all(&mut registry, remove_directory);

    let results = registry.run_all_deinit(&scope, &reporter);

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
        message: "sah removal".to_string(),
        elapsed_ms: start.elapsed().as_millis() as u64,
    });

    if has_errors {
        Err("Some components failed to deinitialize".to_string())
    } else {
        Ok(())
    }
}

// Unit tests for the store-cleanup helpers (`remove_if_symlink`,
// `remove_store_entries`) live in `mirdan::store`'s test module — these
// helpers were moved out of swissarmyhammer-cli when the path-safety and
// store-cleanup code consolidated into mirdan.
