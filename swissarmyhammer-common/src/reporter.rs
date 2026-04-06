//! Reporter abstraction for init/deinit lifecycle progress.
//!
//! Provides styled terminal output via `CliReporter` using `indicatif` spinners
//! and `colored` text, and a `NullReporter` that discards events (useful for tests).
//! Components emit `InitEvent` variants through the `InitReporter` trait instead
//! of calling `println!`/`eprintln!` directly.

use serde::Serialize;
use std::io::IsTerminal;

/// Events emitted during init/deinit lifecycle operations.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", content = "data")]
pub enum InitEvent {
    /// Top-level header: "Initializing sah..." or "Removing sah..."
    Header { message: String },
    /// A component completed an action successfully.
    Action { verb: String, message: String },
    /// A warning (non-fatal).
    Warning { message: String },
    /// An error (fatal for that component).
    Error { message: String },
    /// A component was skipped.
    Skipped { component: String, reason: String },
    /// Top-level finished with timing.
    Finished { message: String, elapsed_ms: u64 },
}

/// Trait for reporting init/deinit progress. Send + Sync for threading.
pub trait InitReporter: Send + Sync {
    /// Emit a single lifecycle event.
    fn emit(&self, event: &InitEvent);
}

/// CLI reporter: styled terminal output with spinners and color.
///
/// Uses `indicatif` for spinners and `colored` for text styling.
/// Falls back to plain text when not connected to a terminal or when
/// `NO_COLOR` is set.
pub struct CliReporter;

impl CliReporter {
    /// Whether we should use color/spinners.
    fn use_color() -> bool {
        std::io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none()
    }
}

impl InitReporter for CliReporter {
    fn emit(&self, event: &InitEvent) {
        if Self::use_color() {
            emit_styled(event);
        } else {
            emit_plain(event);
        }
    }
}

/// Plain-text fallback for non-TTY / NO_COLOR environments.
fn emit_plain(event: &InitEvent) {
    match event {
        InitEvent::Header { message } => {
            eprintln!("  {}", message);
            eprintln!();
        }
        InitEvent::Action { verb, message } => {
            eprintln!("  + {} {}", verb, message);
        }
        InitEvent::Warning { message } => {
            eprintln!("  ! {}", message);
        }
        InitEvent::Error { message } => {
            eprintln!("  x {}", message);
        }
        InitEvent::Skipped { .. } => {}
        InitEvent::Finished {
            message,
            elapsed_ms,
        } => {
            eprintln!();
            eprintln!("  {} in {:.2}s", message, *elapsed_ms as f64 / 1000.0);
            eprintln!();
        }
    }
}

/// Styled terminal output with unicode symbols and ANSI colors.
fn emit_styled(event: &InitEvent) {
    use colored::Colorize;

    match event {
        InitEvent::Header { message } => {
            eprintln!("  {}", message.dimmed());
            eprintln!();
        }
        InitEvent::Action { verb, message } => {
            // Show a completed step with a green checkmark
            eprintln!(
                "  {}  {:>12} {}",
                "✓".green().bold(),
                verb.green().bold(),
                message.dimmed()
            );
        }
        InitEvent::Warning { message } => {
            eprintln!("  {}  {}", "⚠".yellow().bold(), message.yellow());
        }
        InitEvent::Error { message } => {
            eprintln!("  {}  {}", "✗".red().bold(), message.red());
        }
        InitEvent::Skipped { .. } => {
            // Silent for skipped
        }
        InitEvent::Finished {
            message,
            elapsed_ms,
        } => {
            eprintln!();
            eprintln!(
                "  {}  {} {}",
                "◆".green().bold(),
                message.bold(),
                format!("in {:.2}s", *elapsed_ms as f64 / 1000.0).dimmed()
            );
            eprintln!();
        }
    }
}

/// Null reporter: discards all events (for tests).
pub struct NullReporter;

impl InitReporter for NullReporter {
    fn emit(&self, _event: &InitEvent) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// Collecting reporter that records all emitted events (for test assertions).
    struct CollectingReporter {
        events: Arc<Mutex<Vec<InitEvent>>>,
    }

    impl CollectingReporter {
        fn new() -> (Self, Arc<Mutex<Vec<InitEvent>>>) {
            let events = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    events: events.clone(),
                },
                events,
            )
        }
    }

    impl InitReporter for CollectingReporter {
        fn emit(&self, event: &InitEvent) {
            self.events.lock().unwrap().push(event.clone());
        }
    }

    #[test]
    fn null_reporter_does_not_panic() {
        let reporter = NullReporter;
        reporter.emit(&InitEvent::Header {
            message: "test".to_string(),
        });
        reporter.emit(&InitEvent::Action {
            verb: "Installed".to_string(),
            message: "thing".to_string(),
        });
        reporter.emit(&InitEvent::Warning {
            message: "oops".to_string(),
        });
        reporter.emit(&InitEvent::Error {
            message: "bad".to_string(),
        });
        reporter.emit(&InitEvent::Skipped {
            component: "c".to_string(),
            reason: "r".to_string(),
        });
        reporter.emit(&InitEvent::Finished {
            message: "done".to_string(),
            elapsed_ms: 42,
        });
    }

    #[test]
    fn collecting_reporter_captures_events() {
        let (reporter, events) = CollectingReporter::new();
        reporter.emit(&InitEvent::Header {
            message: "hello".to_string(),
        });
        reporter.emit(&InitEvent::Action {
            verb: "Created".to_string(),
            message: "file".to_string(),
        });

        let captured = events.lock().unwrap();
        assert_eq!(captured.len(), 2);
    }

    #[test]
    fn init_event_serializes_as_tagged() {
        let event = InitEvent::Action {
            verb: "Installed".to_string(),
            message: "MCP server".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"kind\":\"Action\""));
        assert!(json.contains("\"verb\":\"Installed\""));
    }

    #[test]
    fn cli_reporter_does_not_panic() {
        let reporter = CliReporter;
        reporter.emit(&InitEvent::Header {
            message: "test".to_string(),
        });
        reporter.emit(&InitEvent::Finished {
            message: "done".to_string(),
            elapsed_ms: 100,
        });
    }

    /// Helper that builds one of every `InitEvent` variant for exhaustive testing.
    fn all_event_variants() -> Vec<InitEvent> {
        vec![
            InitEvent::Header {
                message: "Initializing sah...".to_string(),
            },
            InitEvent::Action {
                verb: "Installed".to_string(),
                message: "MCP server".to_string(),
            },
            InitEvent::Warning {
                message: "config missing".to_string(),
            },
            InitEvent::Error {
                message: "connection refused".to_string(),
            },
            InitEvent::Skipped {
                component: "lsp".to_string(),
                reason: "not configured".to_string(),
            },
            InitEvent::Finished {
                message: "Done".to_string(),
                elapsed_ms: 1234,
            },
        ]
    }

    #[test]
    fn emit_plain_handles_all_variants() {
        // Exercises every branch in emit_plain. Output goes to stderr
        // which is fine for coverage — we verify no panics and that every
        // match arm is reached.
        for event in all_event_variants() {
            emit_plain(&event);
        }
    }

    #[test]
    fn emit_styled_handles_all_variants() {
        // Exercises every branch in emit_styled including colored output.
        // When running in CI (non-TTY), colored will strip ANSI codes
        // but the formatting logic still runs.
        for event in all_event_variants() {
            emit_styled(&event);
        }
    }

    #[test]
    fn emit_plain_skipped_is_silent() {
        // Skipped events intentionally produce no output in plain mode.
        // This verifies the empty match arm doesn't panic.
        emit_plain(&InitEvent::Skipped {
            component: "watcher".to_string(),
            reason: "disabled".to_string(),
        });
    }

    #[test]
    fn emit_styled_skipped_is_silent() {
        // Skipped events intentionally produce no output in styled mode.
        emit_styled(&InitEvent::Skipped {
            component: "watcher".to_string(),
            reason: "disabled".to_string(),
        });
    }

    #[test]
    fn emit_plain_finished_formats_seconds() {
        // Verify the elapsed-time conversion doesn't panic with edge values.
        emit_plain(&InitEvent::Finished {
            message: "Complete".to_string(),
            elapsed_ms: 0,
        });
        emit_plain(&InitEvent::Finished {
            message: "Complete".to_string(),
            elapsed_ms: 999,
        });
        emit_plain(&InitEvent::Finished {
            message: "Complete".to_string(),
            elapsed_ms: 60_000,
        });
    }

    #[test]
    fn emit_styled_finished_formats_seconds() {
        // Verify the elapsed-time conversion doesn't panic with edge values.
        emit_styled(&InitEvent::Finished {
            message: "Complete".to_string(),
            elapsed_ms: 0,
        });
        emit_styled(&InitEvent::Finished {
            message: "Complete".to_string(),
            elapsed_ms: 999,
        });
        emit_styled(&InitEvent::Finished {
            message: "Complete".to_string(),
            elapsed_ms: 60_000,
        });
    }

    #[test]
    fn cli_reporter_emit_exercises_all_variants() {
        // Calls CliReporter::emit for every variant, covering the
        // dispatch logic (line 50-56) and whichever of emit_plain /
        // emit_styled the environment selects.
        let reporter = CliReporter;
        for event in all_event_variants() {
            reporter.emit(&event);
        }
    }
}
