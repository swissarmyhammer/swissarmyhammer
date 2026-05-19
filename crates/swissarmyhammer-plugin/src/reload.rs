//! Hot reload: turning filesystem changes into load / reload / unload.
//!
//! [`PluginHost`](crate::PluginHost) subscribes to the
//! `swissarmyhammer-directory` stack-aware [`Watcher`](swissarmyhammer_directory::Watcher)
//! on the `plugins/` subdirectory. Each [`StackedEvent`](swissarmyhammer_directory::StackedEvent)
//! it delivers is translated — by the host — into one lifecycle decision for
//! the affected plugin id, considering which layer's copy is currently active.
//! The translation rules and the reload mechanics live in [`crate::host`]; this
//! module owns the per-plugin reload status the host surfaces:
//!
//! - [`ReloadStatus`] — the per-plugin status the host surfaces so a crashed
//!   or failed-to-reload plugin is observable rather than silent. Crashed
//!   plugins do not auto-restart; this is how a caller (the settings UI, a
//!   test) learns one needs attention.

use std::fmt;

/// The outcome the host records for the most recent reload of a plugin id.
///
/// Exposed through [`PluginHost::reload_status`](crate::PluginHost::reload_status)
/// so a caller — the settings UI, a test — can tell a healthy plugin apart
/// from one that needs attention. Crashed and failed-to-reload plugins do not
/// auto-restart; this status is how that fact surfaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReloadStatus {
    /// The plugin is loaded and serving — the most recent lifecycle action on
    /// its id succeeded.
    Healthy,

    /// A reload (or layer-fallback load) of the plugin failed. The plugin is
    /// left unloaded — there is no fallback to the previous copy, because that
    /// copy's isolate was already torn down — and the carried message is the
    /// surfaced error. A manual reload is required.
    Failed {
        /// The error that the failed load surfaced, rendered for display.
        error: String,
    },
}

impl fmt::Display for ReloadStatus {
    /// Renders the status as a short human-readable line.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReloadStatus::Healthy => write!(f, "healthy"),
            ReloadStatus::Failed { error } => write!(f, "reload failed: {error}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every `ReloadStatus` variant renders a non-empty line.
    #[test]
    fn reload_status_displays_non_empty() {
        assert!(!ReloadStatus::Healthy.to_string().is_empty());
        assert!(!ReloadStatus::Failed {
            error: "boom".to_string(),
        }
        .to_string()
        .is_empty());
    }
}
