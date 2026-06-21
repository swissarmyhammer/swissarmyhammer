//! Set up sah for all detected AI coding agents (skills + agents + MCP +
//! statusline + preamble).
//!
//! The MCP server, builtin skills, builtin agents, statusline, and CLAUDE.md
//! preamble are all installed through sah's declarative [`Profile`] via
//! [`mirdan::install::init_profile`] — sah is "just a bigger profile," not a
//! special case. The two install concerns that are not expressible as profile
//! data — the `.sah/` + `.prompts/` workspace structure and the `.kanban/`
//! merge drivers — run as the `Initializable` components registered by
//! [`crate::commands::registry::register_all`].

use std::time::Instant;

use crate::cli::InstallTarget;
use swissarmyhammer_common::lifecycle::{InitRegistry, InitScope};
use swissarmyhammer_common::reporter::{CliReporter, InitEvent, InitReporter};

/// Install sah for all detected AI coding agents.
///
/// Runs sah's [`Profile`] through [`mirdan::install::init_profile`] (MCP,
/// skills, agents, statusline, preamble) and then the non-profile
/// `Initializable` components (project workspace, kanban merge drivers) in
/// priority order. Components that are not applicable to the given scope are
/// automatically skipped.
pub fn install(target: InstallTarget) -> Result<(), String> {
    let reporter = CliReporter;
    let start = Instant::now();
    let scope: InitScope = target.into();

    crate::banner::print_banner_stderr();
    reporter.emit(&InitEvent::Header {
        message: format!("Installing for {:?} scope", scope),
    });

    let mut results = mirdan::install::init_profile(
        &crate::commands::profile::sah_profile(),
        scope,
        None,
        &reporter,
    );

    let mut registry = InitRegistry::new();
    crate::commands::registry::register_all(&mut registry, false);
    results.extend(registry.run_all_init(&scope, &reporter));

    let has_errors = super::report_results(&results, &reporter);

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
