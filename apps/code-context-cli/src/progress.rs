//! CLI-side consumer for MCP `notifications/progress` messages.
//!
//! Tools dispatched through `code-context` (and any future MCP-invoking CLI in
//! this workspace) may emit `ProgressNotificationParam` events while they run.
//! This module provides the consumer side: a small renderer abstraction plus
//! a dispatch wrapper that wires a fresh `progressToken`, an in-process
//! notification sink on the `ToolContext`, and a renderer task driven by the
//! incoming notification stream.
//!
//! ## Op-agnostic by design
//!
//! Renderers operate on raw [`ProgressNotificationParam`] only — they do not
//! know whether the producing op is `rebuild index`, a future `reindex
//! workspace`, or anything else. The bar identity (one bar per `progress_token`)
//! comes from the wire payload alone, so any new MCP op that emits progress
//! automatically gets a TUI without changes here.
//!
//! ## Wire path for in-process calls
//!
//! The MCP server-side bridge (`swissarmyhammer-tools/src/mcp/progress.rs`)
//! buffers `ProgressNotificationParam`s on an `mpsc::UnboundedReceiver` and
//! ships them through `Peer::send_notification` over stdio. For an in-process
//! call from this binary we never spin up a stdio server — instead the CLI
//! sets `ToolContext::progress_sink` to its own [`mpsc::UnboundedSender`] and
//! the tool's existing drain task forwards each param straight to us. No
//! special-case code in the tool; the renderer here is a drop-in consumer.

use std::collections::HashMap;

use indicatif::{ProgressBar, ProgressStyle};
use rmcp::model::{NumberOrString, ProgressNotificationParam, ProgressToken};
use tokio::sync::mpsc;

/// Default `indicatif` template used for live progress bars.
///
/// Layout: `<message> [bar] pos/len` — message comes from the tool, the bar
/// and counters reflect the latest `progress`/`total` pair on the wire.
/// `indicatif` automatically degrades the bar glyphs to plain text when stdout
/// is not a TTY, so this template stays usable under `tee`, CI, and piped
/// scripts without a separate code path.
const BAR_TEMPLATE: &str = "{msg} [{bar:40.cyan/blue}] {pos}/{len}";

/// Consumer for [`ProgressNotificationParam`] events emitted by MCP tools.
///
/// Implementations are driven by [`dispatch_with_progress`]: one
/// [`Self::on_notification`] call per inbound notification, then a single
/// [`Self::finish`] call when the source channel closes.
///
/// All methods are synchronous and `&mut self` — renderers are single-owner
/// and live on the consuming task; they never need to be `Sync`.
pub trait ProgressRenderer: Send + 'static {
    /// Apply one wire notification to the renderer state.
    ///
    /// Called exactly once per inbound notification, in the order the tool
    /// emitted them. Implementations key bars / lines off
    /// `n.progress_token` so multiple concurrent ops do not collide.
    fn on_notification(&mut self, n: &ProgressNotificationParam);

    /// Signal end-of-stream; the renderer should clean up its surface.
    ///
    /// Called once after the notification channel closes. For TUI-style
    /// renderers this typically finalises any open progress bars so they
    /// stay visible after the tool returns.
    fn finish(&mut self);
}

/// `indicatif`-backed renderer that draws one bar per `progressToken`.
///
/// Bars are created lazily on the first notification carrying a given token
/// and stored in a `HashMap` keyed off the token. The wire `total` is allowed
/// to grow over the lifetime of a tool call (the MCP spec permits a refining
/// total) and each notification's `total` updates the bar length accordingly.
///
/// `indicatif` auto-degrades to plain-line output on non-TTY stdout, so this
/// renderer is also a reasonable default for piped/CI use; callers who
/// explicitly want zero output should use [`NullRenderer`] instead.
pub struct IndicatifRenderer {
    /// One bar per `progress_token`. Bars are stored as `ProgressBar` (cheap
    /// `Arc`-backed handles) so the map's `Drop` does not finalise them
    /// twice — callers control finalisation via [`ProgressRenderer::finish`].
    bars: HashMap<String, ProgressBar>,
}

impl IndicatifRenderer {
    /// Create a fresh renderer with no bars.
    pub fn new() -> Self {
        Self {
            bars: HashMap::new(),
        }
    }

    /// Stable string key for a `ProgressToken`.
    ///
    /// `ProgressToken` wraps `NumberOrString`; we collapse both variants to a
    /// `String` for `HashMap` use. Stringifying a number is fine because the
    /// spec guarantees tokens are stable for the lifetime of a request, and
    /// we never need to round-trip the key.
    fn key(token: &ProgressToken) -> String {
        match &token.0 {
            NumberOrString::Number(n) => n.to_string(),
            NumberOrString::String(s) => s.to_string(),
        }
    }

    /// Default style used for newly created bars.
    fn default_style() -> ProgressStyle {
        // `template` parses the `BAR_TEMPLATE` constant; unwrap is safe
        // because the template is a compile-time literal we control.
        ProgressStyle::with_template(BAR_TEMPLATE)
            .expect("BAR_TEMPLATE is a valid indicatif template")
            .progress_chars("##-")
    }
}

impl Default for IndicatifRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgressRenderer for IndicatifRenderer {
    fn on_notification(&mut self, n: &ProgressNotificationParam) {
        let key = Self::key(&n.progress_token);
        let bar = self.bars.entry(key).or_insert_with(|| {
            // Initial length: prefer the carried `total`, falling back to
            // `progress` (or 0 if neither is known yet). `indicatif`
            // accepts `set_length` updates as the run learns its plan.
            let initial_len = n.total.map(|t| t as u64).unwrap_or(n.progress as u64);
            let pb = ProgressBar::new(initial_len);
            pb.set_style(Self::default_style());
            pb
        });

        if let Some(t) = n.total {
            bar.set_length(t as u64);
        }
        bar.set_position(n.progress as u64);
        if let Some(m) = &n.message {
            bar.set_message(m.clone());
        }
    }

    fn finish(&mut self) {
        for bar in self.bars.values() {
            // `finish` keeps the bar on screen at its final position. The
            // bar is `Drop`-safe, but we close it explicitly so the
            // terminal cursor moves past the bar surface before the next
            // line of stdout.
            bar.finish();
        }
    }
}

/// Renderer that swallows every notification and produces no output.
///
/// Used when `--no-progress` is passed (CI, piped scripts, anywhere the
/// `indicatif` TTY heuristic might still get the wrong answer). Construction
/// is free and `Drop` is a no-op, so swapping this in costs nothing.
#[derive(Debug, Default)]
pub struct NullRenderer;

impl ProgressRenderer for NullRenderer {
    fn on_notification(&mut self, _n: &ProgressNotificationParam) {}
    fn finish(&mut self) {}
}

/// Create a fresh, unique `ProgressToken` for one outgoing tool call.
///
/// Uses a UUID v4 so tokens are globally unique even when multiple CLI
/// processes run in parallel and (hypothetically) tee their notifications
/// into the same recording sink.
pub fn fresh_progress_token() -> ProgressToken {
    // `NumberOrString::String` wraps `Arc<str>`; `.into()` converts the UUID
    // string without an extra allocation past what `to_string` already did.
    ProgressToken(NumberOrString::String(
        uuid::Uuid::new_v4().to_string().into(),
    ))
}

/// Spawn a tokio task that drives `renderer` from `rx` until the channel
/// closes, then calls [`ProgressRenderer::finish`] and returns.
///
/// Exposed so callers that build their own dispatch loop can plug a renderer
/// in without going through [`dispatch_with_progress`]. The returned handle
/// resolves when the renderer's `finish` has run, so callers can `.await` it
/// to flush terminal output before returning the tool's result.
pub fn spawn_renderer_task<R: ProgressRenderer>(
    mut renderer: R,
    mut rx: mpsc::UnboundedReceiver<ProgressNotificationParam>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(param) = rx.recv().await {
            renderer.on_notification(&param);
        }
        renderer.finish();
    })
}

/// Set of bindings the caller wires into a `ToolContext` to enable progress.
///
/// The dispatch flow is:
///
/// 1. Call [`build_progress_wiring`] before constructing the `ToolContext`.
/// 2. Use [`ProgressWiring::token`] and [`ProgressWiring::sink`] on the
///    `ToolContext` (via `with_progress_token` / `with_progress_sink`).
/// 3. After `tool.execute(...)` returns, drop the sender end of the sink
///    (which lives inside the moved `ToolContext`) — the wiring's
///    [`ProgressWiring::renderer_handle`] then completes and can be awaited
///    so the terminal is fully rendered before the next CLI output line.
pub struct ProgressWiring {
    /// Token to install on the `ToolContext` via `with_progress_token`.
    pub token: ProgressToken,
    /// Sink to install on the `ToolContext` via `with_progress_sink`.
    pub sink: mpsc::UnboundedSender<ProgressNotificationParam>,
    /// Renderer task handle; await after the tool returns and the sink is
    /// dropped so the renderer's `finish` has run.
    pub renderer_handle: tokio::task::JoinHandle<()>,
}

/// Build the token / sink / renderer-task triple for one tool call.
///
/// Spawns the renderer task immediately so the channel has a consumer the
/// moment the tool starts emitting progress. Callers move `token` and `sink`
/// into the `ToolContext` and `.await` `renderer_handle` once the tool has
/// returned and the sink has been dropped.
pub fn build_progress_wiring<R: ProgressRenderer>(renderer: R) -> ProgressWiring {
    let token = fresh_progress_token();
    let (sink, rx) = mpsc::unbounded_channel();
    let renderer_handle = spawn_renderer_task(renderer, rx);
    ProgressWiring {
        token,
        sink,
        renderer_handle,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a string-typed `ProgressToken` for tests.
    fn token(s: &str) -> ProgressToken {
        ProgressToken(NumberOrString::String(s.into()))
    }

    /// Helper: construct a `ProgressNotificationParam` with the supplied
    /// fields.
    fn param(
        tok: &str,
        progress: f64,
        total: Option<f64>,
        message: Option<&str>,
    ) -> ProgressNotificationParam {
        ProgressNotificationParam {
            progress_token: token(tok),
            progress,
            total,
            message: message.map(str::to_string),
        }
    }

    #[test]
    fn indicatif_renderer_creates_one_bar_per_token() {
        let mut r = IndicatifRenderer::new();
        r.on_notification(&param("tok1", 1.0, Some(10.0), Some("a")));
        r.on_notification(&param("tok1", 2.0, Some(10.0), Some("b")));
        r.on_notification(&param("tok2", 3.0, Some(20.0), Some("c")));
        assert_eq!(r.bars.len(), 2, "one bar per distinct progress_token");
    }

    #[test]
    fn indicatif_renderer_updates_position_length_message() {
        let mut r = IndicatifRenderer::new();
        r.on_notification(&param("tok", 5.0, Some(20.0), Some("starting")));
        r.on_notification(&param("tok", 10.0, Some(40.0), Some("halfway")));
        let bar = r.bars.get("tok").expect("bar should exist");
        assert_eq!(
            bar.position(),
            10,
            "position should reflect latest notification"
        );
        assert_eq!(
            bar.length(),
            Some(40),
            "length should grow with refined total"
        );
        assert_eq!(bar.message(), "halfway");
    }

    #[test]
    fn indicatif_renderer_handles_missing_total() {
        let mut r = IndicatifRenderer::new();
        // No total in the first notification — bar should still be created
        // and position should be applied.
        r.on_notification(&param("tok", 7.0, None, None));
        let bar = r.bars.get("tok").expect("bar should exist");
        assert_eq!(bar.position(), 7);
    }

    #[test]
    fn indicatif_renderer_finish_does_not_panic() {
        let mut r = IndicatifRenderer::new();
        r.on_notification(&param("tok", 1.0, Some(2.0), None));
        r.finish();
        // calling finish on an empty renderer should also be fine
        let mut r2 = IndicatifRenderer::new();
        r2.finish();
    }

    #[test]
    fn null_renderer_swallows_everything() {
        let mut r = NullRenderer;
        r.on_notification(&param("tok", 1.0, Some(2.0), Some("noise")));
        r.on_notification(&param("tok", 2.0, Some(2.0), Some("more noise")));
        r.finish();
        // No assertions beyond "this does not panic and compiles" — the
        // renderer is meant to be inert.
    }

    #[test]
    fn fresh_progress_token_is_unique() {
        let a = fresh_progress_token();
        let b = fresh_progress_token();
        assert_ne!(a, b, "successive fresh tokens must be unique");
    }

    #[tokio::test]
    async fn spawn_renderer_task_drains_until_close() {
        let (tx, rx) = mpsc::unbounded_channel();
        let r = IndicatifRenderer::new();
        let handle = spawn_renderer_task(r, rx);
        tx.send(param("tok", 1.0, Some(2.0), Some("a"))).unwrap();
        tx.send(param("tok", 2.0, Some(2.0), Some("b"))).unwrap();
        drop(tx);
        handle.await.expect("renderer task should join cleanly");
    }

    #[tokio::test]
    async fn build_progress_wiring_returns_usable_triple() {
        let wiring = build_progress_wiring(NullRenderer);
        // Token must be a UUID-shaped string token (we control the
        // constructor; the assertion is light — just that it's the String
        // variant).
        assert!(
            matches!(wiring.token.0, NumberOrString::String(_)),
            "fresh token should be a UUID-shaped String"
        );
        // Send one notification and close, then join the renderer.
        wiring
            .sink
            .send(param("tok", 1.0, None, None))
            .expect("sink should accept while renderer task is alive");
        drop(wiring.sink);
        wiring
            .renderer_handle
            .await
            .expect("renderer task should join cleanly");
    }
}
