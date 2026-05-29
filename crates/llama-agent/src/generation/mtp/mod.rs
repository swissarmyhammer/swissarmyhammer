//! MTP (Multi-Token Prediction) speculative decoding for the `llama-agent`
//! generation path.
//!
//! This is the consumer-side port of the `draft-mtp` orchestration whose
//! bindings and reference loop live in the `llama-cpp-rs` fork (see that fork's
//! `mtp-orchestration.md` and `examples/mtp/`). The fork exposes the primitives
//! (`LlamaContextType::Mtp`, pre-norm embeddings, `LlamaMtpBatch` / `decode_mtp`,
//! seq-state); the draft→verify→accept *loop* is reimplemented here because we
//! drive our own decode loop rather than running `llama-server`.
//!
//! - [`helpers`] — pure, model-free decision rules (verbatim port of the fork's
//!   reference helpers), unit-tested without a model.

pub mod helpers;
