//! The MTP draft→verify→accept orchestration loop, ported from the tested
//! reference in the `llama-cpp-rs` fork (`examples/mtp/src/session.rs`).
//!
//! Single sequence, CPU top-k sampling. Mirrors
//! `common_speculative_impl_draft_mtp` in `llama.cpp/common/speculative.cpp`.
//! The fork's `mtp-orchestration.md` is the design spec. Kept structurally
//! identical to the reference so the fork's gated correctness test remains the
//! authority on the algorithm.

use anyhow::{Context, Result};

use llama_cpp_2::context::LlamaContext;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::mtp_batch::LlamaMtpBatch;
use llama_cpp_2::sampling::LlamaSampler;
use llama_cpp_2::token::data_array::LlamaTokenDataArray;
use llama_cpp_2::token::LlamaToken;

use super::helpers::{accept_h_index, draft_should_stop, shift_h_mapping, verify_acceptance};

/// The single sequence id this session drafts on.
const DRAFT_SEQ_ID: i32 = 0;

/// Tuning parameters for the MTP draft→verify→accept loop.
///
/// Defaults mirror the reference: a small per-step draft budget, greedy drafting
/// (`p_min = 0.0`, so the accepted stream matches plain greedy generation), and
/// a top-k 10 draft sampler. `#[serde(default)]` lets a model YAML supply a
/// partial `mtp:` block (any omitted field falls back to these defaults).
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct MtpParams {
    /// Maximum draft tokens proposed per step.
    pub n_max: usize,
    /// Minimum draft length; shorter drafts are discarded whole.
    pub n_min: usize,
    /// Minimum top-1 probability required to keep drafting.
    pub p_min: f32,
    /// Top-k cutoff for the draft sampler.
    pub top_k: i32,
}

impl Default for MtpParams {
    fn default() -> Self {
        Self {
            n_max: 4,
            n_min: 1,
            p_min: 0.0,
            top_k: 10,
        }
    }
}

/// The result of verifying a draft against the target. The full emitted stream
/// for the step is `accepted` followed by `next_token`.
#[derive(Debug, Clone)]
pub struct VerifyOutcome {
    /// Number of leading draft tokens the target agreed with (`0..=drafts.len()`).
    pub n_accepted: usize,
    /// The accepted draft prefix (length `n_accepted`).
    pub accepted: Vec<LlamaToken>,
    /// The target's own next token at the position after the last accepted
    /// draft — always present (the speculative-decoding +1).
    pub next_token: LlamaToken,
    /// The position `next_token` occupies: `n_past + n_accepted + 1`.
    pub next_pos: i32,
}

/// Per-sequence state carried across MTP generation steps.
///
/// `pending_h` is the pre-norm hidden row that pairs with the *next* token fed
/// to the MTP head; `verify_h`/`n_rows` hold the target pre-norm rows captured
/// during the most recent verification decode.
#[derive(Debug)]
pub struct MtpSession {
    n_embd: usize,
    pending_h: Vec<f32>,
    verify_h: Vec<f32>,
    n_rows: usize,
    params: MtpParams,
}

impl MtpSession {
    /// Create an empty session for a model with embedding dimension `n_embd`.
    #[must_use]
    pub fn new(n_embd: usize, params: MtpParams) -> Self {
        Self {
            n_embd,
            pending_h: vec![0.0; n_embd],
            verify_h: Vec::new(),
            n_rows: 0,
            params,
        }
    }

    /// Capture the target's pre-norm rows and mirror the accepted tokens onto
    /// the draft context (reference `process()`).
    ///
    /// Call after a target `decode` of `n` sequential positions. Captures each
    /// position's pre-norm row into `verify_h`, carries the last row as
    /// `pending_h`, and replays the tokens on the draft with the
    /// correctness-critical right-shift h-pairing (slot 0 ← pre-call carry, slot
    /// `k>=1` ← `verify_h` row `k-1`; see [`shift_h_mapping`]).
    ///
    /// # Panics
    ///
    /// Panics if `batch_tokens` and `batch_positions` differ in length.
    pub fn sync_capture(
        &mut self,
        target: &LlamaContext,
        draft: &mut LlamaContext,
        batch_tokens: &[LlamaToken],
        batch_positions: &[i32],
        seq_id: i32,
    ) {
        assert_eq!(
            batch_tokens.len(),
            batch_positions.len(),
            "batch_tokens and batch_positions must describe the same positions",
        );
        let n = batch_tokens.len();
        if n == 0 {
            return;
        }

        // Snapshot the carry that pairs with batch_tokens[0] before step 1
        // overwrites pending_h with the freshly captured last row.
        let carry = self.pending_h.clone();

        // Step 1: capture the target's pre-norm rows into verify_h.
        self.n_rows = n;
        self.verify_h.resize(n * self.n_embd, 0.0);
        for i in 0..n {
            let row = target
                .get_embeddings_pre_norm_ith(i32::try_from(i).expect("row index fits into i32"))
                .expect("target produced no pre-norm row for a verified position");
            self.verify_h[i * self.n_embd..(i + 1) * self.n_embd].copy_from_slice(row);
        }
        // The last captured row pairs with the next token (cross-call carryover).
        self.pending_h
            .copy_from_slice(&self.verify_h[(n - 1) * self.n_embd..]);

        // Step 2: mirror onto the draft with the right-shift h-pairing.
        //
        // FIRST drop any draft KV at positions >= batch_positions[0]. The
        // preceding `draft()` advances the draft auto-regressively (seed +
        // proposed drafts), so its KV ends ahead of the canonical accepted
        // frontier. Without this clear the mirror's decode at those same
        // positions trips M-RoPE's `KV.max_pos < batch.start_pos` invariant.
        // Partial `seq_rm` works once `with_n_rs_seq(>0)` is set on the
        // context (see `LlamaModelManager::create_context_with_type`); for
        // the chunked-prefill case batch_positions[0] equals the previous
        // chunk's max + 1 so this is a no-op there. (An earlier port held a
        // snapshot+restore here for hybrid-recurrent draft contexts; with
        // n_rs_seq enabled it is no longer needed and was wrong for chunked
        // prefill — repeated restores between chunks dropped every chunk but
        // the last.)
        if let (Some(&start_pos), Ok(seq)) = (batch_positions.first(), u32::try_from(seq_id)) {
            if let Ok(pos) = u32::try_from(start_pos) {
                if let Err(err) = draft.clear_kv_cache_seq(Some(seq), Some(pos), None) {
                    tracing::warn!(
                        "mtp sync_capture: failed to clear draft KV at >= {pos}: {err:?}"
                    );
                }
            }
        }

        let mut mirror = LlamaMtpBatch::new(n, self.n_embd);
        for (k, row_index) in shift_h_mapping(n).into_iter().enumerate() {
            let embd = match row_index {
                None => &carry[..],
                Some(r) => &self.verify_h[r * self.n_embd..(r + 1) * self.n_embd],
            };
            mirror
                .add(batch_tokens[k], embd, batch_positions[k], seq_id, false)
                .expect("mirror batch sized for n positions");
        }
        if let Err(err) = draft.decode_mtp(&mut mirror) {
            tracing::warn!("mtp sync_capture: draft mirror decode failed: {err}");
        }
    }

    /// Produce up to `params.n_max` draft tokens on the draft context (reference
    /// `draft()`). Greedy CPU drafting on a single sequence: the seed pairs
    /// `id_last` with the carried `pending_h`, and every subsequent step pairs
    /// the just-sampled token with the pre-norm row the draft produced for it.
    ///
    /// Returns the drafted tokens (`0..=params.n_max`), or empty when shorter
    /// than `params.n_min` or a decode fails.
    ///
    /// # Panics
    ///
    /// Panics if the draft length does not fit into an [`i32`], or if a logits
    /// position yields no candidates/pre-norm row (a backend fault).
    #[must_use]
    pub fn draft(
        &mut self,
        draft: &mut LlamaContext,
        id_last: LlamaToken,
        n_past: i32,
    ) -> Vec<LlamaToken> {
        // Seed: pair id_last with the carried pending_h and request logits.
        let mut batch = LlamaMtpBatch::new(1, self.n_embd);
        batch
            .add(id_last, &self.pending_h, n_past, DRAFT_SEQ_ID, true)
            .expect("seed batch has capacity for one position");
        if let Err(err) = draft.decode_mtp(&mut batch) {
            tracing::warn!("mtp draft: seed decode failed: {err}");
            return Vec::new();
        }

        let mut result: Vec<LlamaToken> = Vec::new();
        // Each single-token batch is read at index 0.
        let i_batch = 0_i32;
        loop {
            let (top1, top1_prob) = self.top1(draft, i_batch);

            if draft_should_stop(top1_prob, self.params.p_min, result.len(), self.params.n_max) {
                break;
            }

            // h_k for the token we are about to keep, read before the next decode
            // invalidates the draft's pre-norm buffer.
            let h_row = draft
                .get_embeddings_pre_norm_ith(i_batch)
                .expect("draft produced no pre-norm row for a logits position")
                .to_vec();

            result.push(top1);
            if result.len() >= self.params.n_max {
                break;
            }

            // Advance the draft one step: pair the kept token with its own h_k.
            let pos = n_past + i32::try_from(result.len()).expect("draft length fits into i32");
            let mut next = LlamaMtpBatch::new(1, self.n_embd);
            next.add(top1, &h_row, pos, DRAFT_SEQ_ID, true)
                .expect("step batch has capacity for one position");
            if let Err(err) = draft.decode_mtp(&mut next) {
                tracing::warn!("mtp draft: step decode failed: {err}");
                break;
            }
        }

        if result.len() < self.params.n_min {
            return Vec::new();
        }
        result
    }

    /// Verify a draft against the target and produce the step's outcome.
    ///
    /// Decodes `[id_last, draft_0, …, draft_{n-1}]` at `n_past ..= n_past + n` on
    /// the target in one batch, every position requesting logits; accepts the
    /// longest prefix where the target's greedy choice matches the draft. The
    /// frontier choice (`target_chosen[n_accepted]`) is always emitted (the +1).
    ///
    /// # Contract
    /// The caller must NOT have decoded `id_last` into the target's KV at
    /// `n_past` — `verify` owns that decode. On return the KV is populated
    /// through `next_pos`.
    ///
    /// # Panics
    ///
    /// Panics if `drafts` is empty, or a verified position yields no candidates.
    pub fn verify(
        &self,
        target: &mut LlamaContext,
        id_last: LlamaToken,
        drafts: &[LlamaToken],
        n_past: i32,
        seq_id: i32,
    ) -> Result<VerifyOutcome> {
        assert!(!drafts.is_empty(), "verify requires at least one draft token");
        let n = drafts.len();

        let mut batch = LlamaBatch::new(n + 1, 1);
        for (i, &token) in std::iter::once(&id_last).chain(drafts).enumerate() {
            let offset = i32::try_from(i).context("verify batch position does not fit into i32")?;
            batch
                .add(token, n_past + offset, &[seq_id], true)
                .context("verify batch sized for id_last plus the draft positions")?;
        }
        target.decode(&mut batch).context("target verify decode failed")?;

        // The target's greedy choice (argmax logit) at each batch index 0..=n.
        let target_chosen: Vec<LlamaToken> = (0..=n)
            .map(|i| {
                let i_batch = i32::try_from(i).expect("batch index fits into i32");
                target
                    .candidates_ith(i_batch)
                    .max_by(|a, b| a.logit().total_cmp(&b.logit()))
                    .map(|data| data.id())
                    .expect("target produced no candidates for a verified position")
            })
            .collect();

        let (n_accepted, next_token) = verify_acceptance(&target_chosen, drafts);
        let next_pos = n_past
            + i32::try_from(n_accepted + 1).context("next position does not fit into i32")?;

        Ok(VerifyOutcome {
            n_accepted,
            accepted: drafts[..n_accepted].to_vec(),
            next_token,
            next_pos,
        })
    }

    /// Accept the verified prefix: roll the target KV back to the accepted
    /// frontier and carry the matching pre-norm row forward (reference
    /// `accept()`).
    ///
    /// `accepted_pos` is the first *rejected* position — [`VerifyOutcome::next_pos`]
    /// from the same step. The KV range `[accepted_pos, end)` is removed
    /// (inclusive), keeping `id_last`, the accepted drafts, and the guaranteed
    /// `next_token`. No draft rollback is needed (the next `sync_capture`
    /// re-mirrors the canonical sequence).
    ///
    /// # Errors
    /// Returns an error if `seq_id`/`accepted_pos` is negative or the KV rollback
    /// fails.
    pub fn accept(
        &mut self,
        target: &mut LlamaContext,
        n_accepted: usize,
        accepted_pos: i32,
        seq_id: i32,
    ) -> Result<()> {
        let seq = u32::try_from(seq_id).context("seq_id must be non-negative")?;
        let pos = u32::try_from(accepted_pos).context("accepted_pos must be non-negative")?;
        target
            .clear_kv_cache_seq(Some(seq), Some(pos), None)
            .context("target KV rollback to accepted frontier failed")?;

        let row = accept_h_index(n_accepted, self.n_rows);
        self.pending_h
            .copy_from_slice(&self.verify_h[row * self.n_embd..(row + 1) * self.n_embd]);

        Ok(())
    }

    /// The top-1 candidate and its probability at draft batch index `i_batch`
    /// (top-k `params.top_k` cut + softmax; greedy pick is the argmax).
    ///
    /// # Panics
    ///
    /// Panics if the draft produced no candidates at `i_batch`.
    fn top1(&self, draft: &LlamaContext, i_batch: i32) -> (LlamaToken, f32) {
        let mut candidates = LlamaTokenDataArray::from_iter(draft.candidates_ith(i_batch), false);
        candidates.apply_sampler(&LlamaSampler::top_k(self.params.top_k));
        candidates.apply_sampler(&LlamaSampler::dist(0));
        let top = candidates
            .data
            .first()
            .expect("draft produced no candidates");
        (top.id(), top.p())
    }
}
