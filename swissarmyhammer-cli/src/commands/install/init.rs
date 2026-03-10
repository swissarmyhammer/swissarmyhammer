//! Set up sah for all detected AI coding agents (skills + MCP).
//!
//! Delegates to composable `Initializable` components registered in `super::components`.

use crate::cli::InstallTarget;
use swissarmyhammer_common::lifecycle::{InitRegistry, InitScope, InitStatus};

use super::components;

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

    // Display results and check for errors
    let mut has_errors = false;
    for r in &results {
        match r.status {
            InitStatus::Ok => {}    // component already printed its messages
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
