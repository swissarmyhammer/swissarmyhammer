//! Hot reload: turning filesystem changes into load / reload / unload.
//!
//! [`PluginHost`](crate::PluginHost) subscribes to the
//! `swissarmyhammer-directory` stack-aware [`Watcher`](swissarmyhammer_directory::Watcher)
//! on the `plugins/` subdirectory. Each [`StackedEvent`](swissarmyhammer_directory::StackedEvent)
//! it delivers is translated — by the host — into one lifecycle decision for
//! the affected plugin id, considering which layer's copy is currently active.
//! The translation rules and the reload mechanics live in [`crate::host`]; this
//! module owns the two seams hot reload needs that are not lifecycle code:
//!
//! - [`ReloadPolicy`] — the `provides`-expansion re-approval hook. When a
//!   reloaded plugin's manifest declares more server names than the copy it
//!   replaces, the host pauses and asks the policy whether to proceed. The
//!   actual UI prompt is out of scope; this trait is the seam a host (or a
//!   test) installs a real decision behind.
//! - [`ReloadStatus`] — the per-plugin status the host surfaces so a crashed
//!   or failed-to-reload plugin is observable rather than silent. Crashed
//!   plugins do not auto-restart; this is how a caller (the settings UI, a
//!   test) learns one needs attention.

use std::fmt;

/// Whether a `provides` expansion on reload is allowed to proceed.
///
/// Returned by [`ReloadPolicy::approve_provides_expansion`]. A reload that
/// would let a plugin register server names beyond the set its previous copy
/// declared is a privilege escalation, so it is gated: the host applies the
/// policy's decision rather than silently widening what the plugin may do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProvidesDecision {
    /// The expansion is approved; the reload proceeds with the wider `provides`.
    Approve,
    /// The expansion is denied; the reload is abandoned and the plugin is left
    /// unloaded, exactly as a failed v2 load would leave it.
    Deny,
}

/// A `provides`-expansion re-approval request handed to a [`ReloadPolicy`].
///
/// Carries the plugin's identity and both the previously approved set of
/// server names and the wider set the reloaded copy's manifest declares, so a
/// policy — or a UI built on one — can show the user exactly what new
/// capability is being requested.
#[derive(Debug, Clone)]
pub struct ProvidesExpansion {
    /// The manifest id of the plugin being reloaded.
    pub plugin: String,

    /// The server names the currently active copy was approved to provide.
    pub previous: Vec<String>,

    /// The server names the reloaded copy's manifest declares — a strict
    /// superset of [`previous`](Self::previous).
    pub requested: Vec<String>,
}

impl ProvidesExpansion {
    /// The server names new in this reload — `requested` minus `previous`.
    ///
    /// These are the names whose addition the policy is being asked to
    /// approve; everything in `previous` was already approved when the active
    /// copy loaded.
    pub fn added(&self) -> Vec<String> {
        self.requested
            .iter()
            .filter(|name| !self.previous.contains(name))
            .cloned()
            .collect()
    }
}

/// The re-approval seam for a reload that expands a plugin's `provides`.
///
/// When a reloaded plugin's manifest declares server names beyond what its
/// active copy was approved for, the host pauses and calls
/// [`approve_provides_expansion`](Self::approve_provides_expansion). The
/// platform ships [`ApproveAllReloads`] and [`DenyProvidesExpansion`] as the
/// two sane defaults; an embedder installs its own implementation — backed by
/// a real UI prompt — when it needs the user in the loop.
///
/// A `ReloadPolicy` is consulted from the host's watcher-drain task, so it
/// must be `Send + Sync`.
pub trait ReloadPolicy: Send + Sync {
    /// Decides whether a reload may widen a plugin's `provides` set.
    ///
    /// Called only when the reloaded copy declares strictly more server names
    /// than the active copy; a reload that keeps or narrows `provides` never
    /// reaches the policy. The host applies the returned
    /// [`ProvidesDecision`]: [`Approve`](ProvidesDecision::Approve) lets the
    /// reload proceed, [`Deny`](ProvidesDecision::Deny) abandons it and leaves
    /// the plugin unloaded.
    ///
    /// # Parameters
    ///
    /// - `expansion` — the plugin id and the previous and requested `provides`
    ///   sets, so the implementation can decide with full context.
    fn approve_provides_expansion(&self, expansion: &ProvidesExpansion) -> ProvidesDecision;
}

/// A [`ReloadPolicy`] that approves every `provides` expansion.
///
/// The platform default: hot reload is always available and a plugin author
/// editing their own plugin is trusted to widen its `provides`. An embedder
/// that wants a human in the loop installs its own policy instead.
#[derive(Debug, Clone, Copy, Default)]
pub struct ApproveAllReloads;

impl ReloadPolicy for ApproveAllReloads {
    /// Always returns [`ProvidesDecision::Approve`].
    fn approve_provides_expansion(&self, _expansion: &ProvidesExpansion) -> ProvidesDecision {
        ProvidesDecision::Approve
    }
}

/// A [`ReloadPolicy`] that denies every `provides` expansion.
///
/// The strict policy: a reload may keep or narrow a plugin's `provides`, but
/// any widening is refused until an embedder installs a policy that can ask
/// the user. Useful as a conservative default and as the deny side of a
/// policy-driven test.
#[derive(Debug, Clone, Copy, Default)]
pub struct DenyProvidesExpansion;

impl ReloadPolicy for DenyProvidesExpansion {
    /// Always returns [`ProvidesDecision::Deny`].
    fn approve_provides_expansion(&self, _expansion: &ProvidesExpansion) -> ProvidesDecision {
        ProvidesDecision::Deny
    }
}

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

    /// A reload was abandoned because the [`ReloadPolicy`] denied a `provides`
    /// expansion. The plugin is left unloaded; the carried names are the
    /// server names whose addition was refused.
    ProvidesExpansionDenied {
        /// The server names the denied reload would have added.
        added: Vec<String>,
    },
}

impl fmt::Display for ReloadStatus {
    /// Renders the status as a short human-readable line.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReloadStatus::Healthy => write!(f, "healthy"),
            ReloadStatus::Failed { error } => write!(f, "reload failed: {error}"),
            ReloadStatus::ProvidesExpansionDenied { added } => {
                write!(
                    f,
                    "reload denied: provides expansion to [{}] was refused",
                    added.join(", ")
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `ProvidesExpansion::added` reports exactly the names new to the
    /// requested set.
    #[test]
    fn added_reports_only_the_new_names() {
        let expansion = ProvidesExpansion {
            plugin: "weather".to_string(),
            previous: vec!["forecast".to_string()],
            requested: vec!["forecast".to_string(), "alerts".to_string()],
        };
        assert_eq!(expansion.added(), vec!["alerts".to_string()]);
    }

    /// The approve-all policy approves an expansion.
    #[test]
    fn approve_all_policy_approves() {
        let expansion = ProvidesExpansion {
            plugin: "p".to_string(),
            previous: vec![],
            requested: vec!["s".to_string()],
        };
        assert_eq!(
            ApproveAllReloads.approve_provides_expansion(&expansion),
            ProvidesDecision::Approve
        );
    }

    /// The deny policy denies an expansion.
    #[test]
    fn deny_policy_denies() {
        let expansion = ProvidesExpansion {
            plugin: "p".to_string(),
            previous: vec![],
            requested: vec!["s".to_string()],
        };
        assert_eq!(
            DenyProvidesExpansion.approve_provides_expansion(&expansion),
            ProvidesDecision::Deny
        );
    }

    /// Every `ReloadStatus` variant renders a non-empty line.
    #[test]
    fn reload_status_displays_non_empty() {
        assert!(!ReloadStatus::Healthy.to_string().is_empty());
        assert!(!ReloadStatus::Failed {
            error: "boom".to_string(),
        }
        .to_string()
        .is_empty());
        assert!(!ReloadStatus::ProvidesExpansionDenied {
            added: vec!["s".to_string()],
        }
        .to_string()
        .is_empty());
    }
}
