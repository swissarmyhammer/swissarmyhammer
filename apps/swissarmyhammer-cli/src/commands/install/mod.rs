//! Install and uninstall sah for all detected AI coding agents.
//!
//! The `init` and `deinit` commands install/remove sah's declarative
//! [`Profile`] (the shared SAH MCP server, all builtin skills, all builtin
//! agents, and the statusline) via
//! [`mirdan::install::init_profile`] / [`mirdan::install::deinit_profile`],
//! plus the two non-profile `Initializable` components (the `.sah/` +
//! `.prompts/` project workspace and the `.kanban/` merge drivers) registered
//! by [`crate::commands::registry`].

pub mod components;
pub mod deinit;
pub mod init;

use std::time::Instant;

use crate::cli::InstallTarget;
use swissarmyhammer_common::lifecycle::{InitRegistry, InitResult, InitScope, InitStatus};
use swissarmyhammer_common::reporter::{CliReporter, InitEvent, InitReporter};

/// Which way [`run_lifecycle`] drives sah's install lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Direction {
    /// Install the profile, then run every component's init.
    Install,
    /// Remove the profile, then run every component's deinit.
    Uninstall,
}

impl Direction {
    /// Pick the value that belongs to this direction.
    ///
    /// The one place that branches on the enum, so the per-string accessors
    /// below stay a table of wording rather than three copies of one `match`.
    fn pick<T>(self, install: T, uninstall: T) -> T {
        match self {
            Direction::Install => install,
            Direction::Uninstall => uninstall,
        }
    }

    /// The verb in the header line printed before the run starts.
    fn header_verb(self) -> &'static str {
        self.pick("Installing", "Removing")
    }

    /// The label carried by the closing [`InitEvent::Finished`] event.
    fn finished_label(self) -> &'static str {
        self.pick("sah initialization", "sah removal")
    }

    /// The error returned when at least one component reported an error.
    fn failure_message(self) -> &'static str {
        self.pick(
            "Some components failed to initialize",
            "Some components failed to deinitialize",
        )
    }
}

/// Run sah's install lifecycle in `direction` for `target`'s scope.
///
/// Applies sah's [`Profile`](mirdan::install::Profile) through
/// [`mirdan::install::init_profile`] or [`mirdan::install::deinit_profile`],
/// then runs the non-profile `Initializable` components registered by
/// [`crate::commands::registry::register_all`]. `remove_directory` reaches
/// `ProjectStructure` and only bites on [`Direction::Uninstall`], where it
/// decides whether `.sah/` and `.prompts/` are deleted.
///
/// Install and uninstall differ only in these two dispatch points and in the
/// wording they report, so they share one body rather than two that drift.
pub(crate) fn run_lifecycle(
    direction: Direction,
    target: InstallTarget,
    remove_directory: bool,
) -> Result<(), String> {
    let reporter = CliReporter;
    let start = Instant::now();
    let scope: InitScope = target.into();

    crate::banner::print_banner_stderr();
    reporter.emit(&InitEvent::Header {
        message: format!("{} for {:?} scope", direction.header_verb(), scope),
    });

    let profile = crate::commands::profile::sah_profile();
    let mut results = match direction {
        Direction::Install => mirdan::install::init_profile(&profile, scope, None, &reporter),
        Direction::Uninstall => mirdan::install::deinit_profile(&profile, scope, None, &reporter),
    };

    let mut registry = InitRegistry::new();
    crate::commands::registry::register_all(&mut registry, remove_directory);
    results.extend(match direction {
        Direction::Install => registry.run_all_init(&scope, &reporter),
        Direction::Uninstall => registry.run_all_deinit(&scope, &reporter),
    });

    let has_errors = report_results(&results, &reporter);

    reporter.emit(&InitEvent::Finished {
        message: direction.finished_label().to_string(),
        elapsed_ms: start.elapsed().as_millis() as u64,
    });

    if has_errors {
        Err(direction.failure_message().to_string())
    } else {
        Ok(())
    }
}

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
