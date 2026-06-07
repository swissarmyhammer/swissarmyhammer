//! On-demand UI-geometry provider — the kernel's pull seam into the webview.
//!
//! The focus kernel is **stateless with respect to scope geometry**: scope
//! rects are live DOM reads (`getBoundingClientRect`) that only the webview
//! can produce, and they are sampled fresh per decision — never cached, never
//! held between calls (see the crate-level "Stateless with respect to scope
//! geometry" docs and the owner constraint on Card F2).
//!
//! Where [`crate::observer::FocusEventSink`] is the kernel's PUSH seam (the
//! kernel hands a produced event to the adapter), [`UiGeometryProvider`] is
//! the kernel's PULL seam: when a host-driven nav op needs the live geometry,
//! the current scope chain, or the focused FQM for a window, it asks the
//! provider and awaits the answer. The kernel holds no replica of any of it.
//!
//! # Dependency direction
//!
//! This trait lives in the kernel; the app implements it. The kanban app's
//! implementation answers each query by issuing a host→UI request over the
//! `request_from_ui` channel and awaiting the webview's reply — but the
//! kernel knows nothing about Tauri, exactly as it knows nothing about Tauri
//! events behind [`FocusEventSink`]. Tests substitute a fake provider with
//! fixed answers.
//!
//! # Lock discipline (load-bearing)
//!
//! Every provider method is `async` because the production implementation
//! awaits a webview round-trip. The kernel MUST drop its spatial `Mutex`es
//! before awaiting a provider call and re-acquire afterwards — otherwise the
//! reply (which travels back through a Tauri command that may itself contend
//! for those locks) would deadlock. The [`crate::server::FocusServer`] handlers
//! that pull geometry honor this: they `await` the provider first, THEN take
//! the locks to run the kernel logic.
//!
//! [`FocusEventSink`]: crate::observer::FocusEventSink

use async_trait::async_trait;

use crate::snapshot::NavSnapshot;
use crate::types::{FullyQualifiedMoniker, WindowLabel};

/// The kernel's on-demand pull seam into the webview's live UI geometry.
///
/// Implementations are `Send + Sync` so the focus server can hold one behind
/// an `Arc<dyn UiGeometryProvider>` shared across async tasks. Every method is
/// keyed by [`WindowLabel`] because geometry, scope chain, and focus are all
/// per-window — each Tauri window owns its own layout and focus slot.
#[async_trait]
pub trait UiGeometryProvider: Send + Sync {
    /// Pull the live [`NavSnapshot`] for the focused layer in `window`.
    ///
    /// The webview builds it on demand (`getBoundingClientRect` at the
    /// instant of the call) for whatever scope currently holds focus.
    /// Returns `None` when no snapshot is available — the window is closed,
    /// no responder is registered, or the focused scope's registry has torn
    /// down (the transient unmount window). A `None` makes the calling nav op
    /// drop silently, matching the inline `snapshot: None` early-return.
    async fn snapshot(&self, window: &WindowLabel) -> Option<NavSnapshot>;

    /// Pull the current command/focus scope chain for `window`, outermost
    /// first. Empty when the window has no active scope chain.
    async fn scope_chain(&self, window: &WindowLabel) -> Vec<FullyQualifiedMoniker>;

    /// Pull the FQM currently focused in `window`, or `None` when the window
    /// has no focus.
    async fn focus(&self, window: &WindowLabel) -> Option<FullyQualifiedMoniker>;
}

/// A provider that answers every pull with "nothing" — the default when no
/// real provider is injected.
///
/// Mirrors [`crate::observer::NoopSink`]: a server constructed without
/// [`crate::server::FocusServer::with_provider`] gets this, so the
/// geometry-pull query ops degrade to empty answers (and host-driven navs
/// drop silently) rather than panicking. Tests that only exercise the
/// inline-snapshot ops are unaffected.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopProvider;

#[async_trait]
impl UiGeometryProvider for NoopProvider {
    async fn snapshot(&self, _window: &WindowLabel) -> Option<NavSnapshot> {
        None
    }

    async fn scope_chain(&self, _window: &WindowLabel) -> Vec<FullyQualifiedMoniker> {
        Vec::new()
    }

    async fn focus(&self, _window: &WindowLabel) -> Option<FullyQualifiedMoniker> {
        None
    }
}
