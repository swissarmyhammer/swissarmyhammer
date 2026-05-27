//! Caller-lifecycle seam for the command service.
//!
//! The service treats every successful `register command` as evidence that
//! the registering caller now owns service-side state — its entry on the
//! override-stack registry, plus any future entries it adds. On caller
//! unload (a plugin teardown, primarily) those entries must be purged so
//! the next-most-recent registration re-emerges as active.
//!
//! The service does not know how callers unload — that is a platform
//! concern. [`CallerLifecycle`] is the seam: on every successful register,
//! the service hands the platform a one-shot dispose hook that purges the
//! caller. The platform-integration layer's implementation installs the
//! hook on the caller's per-plugin ledger (see
//! [`swissarmyhammer_plugin::PluginHost::record_unload_hook`]), so the
//! host's authoritative unload drain runs the hook as part of normal
//! cleanup.
//!
//! Service tests substitute a no-op implementation
//! ([`NoopCallerLifecycle`]) — the unload path is not under test there.

use std::sync::Arc;

use swissarmyhammer_plugin::CallerId;

/// One-shot dispose hook the service hands to the platform on register.
///
/// The hook is boxed and `Send` so the platform can store it across
/// threads, and `FnOnce` because each register installs exactly one hook
/// targeted at the calling caller — purge is idempotent, so the hook only
/// has to run once.
pub type UnloadHook = Box<dyn FnOnce() + Send>;

/// Seam the [`crate::CommandService`] uses to install per-caller unload
/// hooks.
///
/// The platform-integration layer's implementation wires `hook` onto the
/// caller's per-plugin ledger entry; the host's unload drain runs it as
/// part of normal disposal. The service-level default
/// ([`NoopCallerLifecycle`]) drops the hook on the floor — used by
/// service tests that exercise the verb dispatch path without a real
/// host.
///
/// Implementations must be cheap to call: the service installs one hook
/// per successful register, so every burst of plugin registrations also
/// triggers a burst of `install_unload_hook` calls.
pub trait CallerLifecycle: Send + Sync + std::fmt::Debug {
    /// Install `hook` so it runs when `caller` is unloaded.
    ///
    /// Implementations target a specific caller's unload boundary: for a
    /// [`CallerId::Plugin`] this is the host's unload of that plugin; for
    /// other caller kinds (the host itself, an external client) the
    /// platform may have no unload boundary to attach to, in which case
    /// the hook is conventionally dropped on the floor.
    ///
    /// # Parameters
    ///
    /// - `caller` — the caller whose unload should trigger `hook`.
    /// - `hook` — the dispose function to run; called at most once.
    fn install_unload_hook(&self, caller: &CallerId, hook: UnloadHook);
}

/// Type alias for the shared lifecycle handle stored on the service.
pub type SharedCallerLifecycle = Arc<dyn CallerLifecycle>;

/// A lifecycle implementation that drops every hook on the floor.
///
/// Used as the default by [`crate::CommandService::new`] so callers that
/// never wire a real platform (service-level verb tests, in particular)
/// can keep working without a [`swissarmyhammer_plugin::PluginHost`]
/// dependency. The real production wiring substitutes a host-aware
/// implementation in the platform-integration layer.
#[derive(Debug, Default)]
pub struct NoopCallerLifecycle;

impl CallerLifecycle for NoopCallerLifecycle {
    fn install_unload_hook(&self, _caller: &CallerId, _hook: UnloadHook) {
        // Intentionally a no-op: the unload path is not under test at the
        // service level, and the hook is one-shot so dropping it leaks
        // nothing.
    }
}
