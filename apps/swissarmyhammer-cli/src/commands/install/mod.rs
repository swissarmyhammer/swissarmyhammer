//! Install and uninstall sah for all detected AI coding agents.
//!
//! The `init` and `deinit` commands install/remove sah's declarative
//! [`Profile`] (the shared SAH MCP server, all builtin skills, all builtin
//! agents, the statusline, and the CLAUDE.md preamble) via
//! [`mirdan::install::init_profile`] / [`mirdan::install::deinit_profile`],
//! plus the two non-profile `Initializable` components (the `.sah/` +
//! `.prompts/` project workspace and the `.kanban/` merge drivers) registered
//! by [`crate::commands::registry`].

pub mod components;
pub mod deinit;
pub mod init;

use swissarmyhammer_common::lifecycle::{InitResult, InitStatus};
use swissarmyhammer_common::reporter::{InitEvent, InitReporter};

/// Emit warnings/errors from `results` through `reporter` and report whether any
/// component errored.
///
/// `Ok` and `Skipped` results are silent — components emit their own progress
/// messages as they run. Returns `true` if any result has
/// [`InitStatus::Error`]. Shared by both [`init::install`] and
/// [`deinit::uninstall`] so the profile and registry result sets are surfaced
/// identically.
pub(crate) fn report_results(results: &[InitResult], reporter: &dyn InitReporter) -> bool {
    let mut has_errors = false;
    for r in results {
        match r.status {
            InitStatus::Ok => {}
            InitStatus::Warning => reporter.emit(&InitEvent::Warning {
                message: r.message.clone(),
            }),
            InitStatus::Error => {
                reporter.emit(&InitEvent::Error {
                    message: r.message.clone(),
                });
                has_errors = true;
            }
            InitStatus::Skipped => {}
        }
    }
    has_errors
}
