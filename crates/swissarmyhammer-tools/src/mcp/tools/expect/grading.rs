//! The tool-layer grading seam for the `expect` ledger ops.
//!
//! The engine's tiered compare ([`compare_tiered`](swissarmyhammer_expect::compare_tiered))
//! consults a **synchronous** [`TextEmbedder`] for the Tier-2 semantic band and the
//! Tier-3 anchor-similarity gate. The platform embedder is **asynchronous** (it is the
//! same `model_embedding::TextEmbedder` `review` resolves via
//! [`default_embedder_factory`](crate::mcp::tools::review::review_op::default_embedder_factory)
//! and a shared process-global cache). This module bridges the two: [`ExpectEmbedder`]
//! adapts the async platform embedder to the engine's sync trait.
//!
//! Wiring mirrors `review` — the embedder is resolved through the *same* injected
//! [`EmbedderFactory`](crate::mcp::tools::review::review_op::EmbedderFactory) seam, so
//! the production server shares one loaded platform model across both tools and tests
//! inject a deterministic mock (or none, exercising the loud-and-safe uncheckable
//! guard). Where `review`'s engine simply `await`s the embedder, `expect` bridges,
//! because its grading trait is sync.

use std::cell::Cell;
use std::sync::Arc;

use tokio::runtime::Handle;

use swissarmyhammer_expect::TextEmbedder;

/// Adapts the platform async embedder (`model_embedding::TextEmbedder`, the model
/// `review` loads via its [`EmbedderFactory`](crate::mcp::tools::review::review_op::EmbedderFactory))
/// to the engine's sync [`TextEmbedder`] that the tiered compare consults.
///
/// [`compare_tiered`](swissarmyhammer_expect::compare_tiered) embeds synchronously,
/// but the platform model is async. Each [`embed`](TextEmbedder::embed) drives the
/// async `embed_text` on `handle` via [`Handle::block_on`], which is sound **only**
/// off a runtime worker — every caller runs the grading inside
/// [`spawn_blocking`](tokio::task::spawn_blocking), so this blocks a blocking-pool
/// thread, never a reactor thread.
pub(super) struct ExpectEmbedder {
    /// The loaded platform embedder, pinned to the repo's configured embedding model
    /// (the value a golden freezes into [`GradingPins`](swissarmyhammer_expect::GradingPins)).
    inner: Arc<dyn model_embedding::TextEmbedder>,
    /// The runtime handle the sync `embed` drives the async `embed_text` on.
    handle: Handle,
    /// Set when any `embed_text` call failed during the pass.
    ///
    /// A failed embed degrades to an empty vector to avoid panicking the grading pass,
    /// but an empty vector is **not** safe to grade on: it scores cosine 0 on BOTH the
    /// golden and received sides, so a genuine divergence reads as pass==pass ⇒ no drift
    /// ⇒ silently Approved — the very footgun this tool closes. So the caller must
    /// [`take_failed`](Self::take_failed) after grading a golden and, when set, escalate
    /// that golden to the NON-approved `uncheckable` status instead of trusting the
    /// (degraded) verdict. A `Cell` suffices because grading runs single-threaded inside
    /// one `spawn_blocking` closure.
    failed: Cell<bool>,
}

impl ExpectEmbedder {
    /// Wrap the loaded platform `embedder`, driving its async embeds on `handle`.
    pub(super) fn new(embedder: Arc<dyn model_embedding::TextEmbedder>, handle: Handle) -> Self {
        Self {
            inner: embedder,
            handle,
            failed: Cell::new(false),
        }
    }

    /// Return whether any embed failed since the last call, clearing the flag.
    ///
    /// The caller checks this after grading each golden: a `true` means the golden was
    /// graded on a degraded (empty-vector) embedding and must be escalated to
    /// `uncheckable` rather than read off the resulting compare.
    pub(super) fn take_failed(&self) -> bool {
        self.failed.replace(false)
    }
}

impl TextEmbedder for ExpectEmbedder {
    /// Embed `text` by driving the platform model's async `embed_text` to completion on
    /// the captured runtime handle.
    ///
    /// A model failure records the failure (for the caller's [`take_failed`](Self::take_failed)
    /// escalation) and degrades to an empty vector so the pass does not panic. The empty
    /// vector is **never** trusted as a real grade — the caller turns a recorded failure
    /// into a NON-approved `uncheckable` outcome.
    fn embed(&self, text: &str) -> Vec<f32> {
        match self.handle.block_on(self.inner.embed_text(text)) {
            Ok(result) => result.embedding().to_vec(),
            Err(err) => {
                tracing::warn!(
                    "expect: embedding failed; the golden will be escalated as uncheckable: {err}"
                );
                self.failed.set(true);
                Vec::new()
            }
        }
    }
}
