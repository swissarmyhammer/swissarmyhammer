//! MTP streaming generation: drive the [`MtpSession`] draft→verify→accept loop
//! on a target+draft context pair, emitting `StreamChunk`s.
//!
//! This is the consumer-side glue that takes the ported orchestration
//! ([`super::session`]) and the existing queue/streaming chunk plumbing and
//! turns them into a drop-in replacement for
//! `GenerationHelper::generate_stream_with_borrowed_model_and_template_offset`
//! whenever the loaded model carries an MTP/NextN head (auto-detect via
//! `model.has_mtp()`).
//!
//! Termination, cancellation, max-tokens and context-window guards mirror the
//! standard streaming generator. Correctness is preserved by the speculative
//! verify (the target always picks the truth); the speedup comes from emitting
//! up to `n_max` accepted draft tokens per target forward pass.

use llama_cpp_2::context::LlamaContext;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::{AddBos, LlamaModel, Special};
use llama_cpp_2::token::LlamaToken;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use crate::generation::send_with_backpressure;

use super::session::{MtpParams, MtpSession};
use crate::generation::budget;
use crate::generation::error::GenerationError;
use crate::types::{FinishReason, GenerationRequest, QueueError, StreamChunk};

/// The seq_id this loop drafts/verifies on (single sequence per session).
const SEQ_ID: i32 = 0;

/// Drive the MTP draft→verify→accept loop on `target` (default-type context)
/// and `draft` (MTP-type context), emitting `StreamChunk`s as tokens are
/// accepted.
///
/// Contract matches the standard streaming generator:
/// - `template_token_count` is the number of leading prompt tokens already in
///   the target's KV (from the streaming KV-reuse path); only the suffix is
///   prefilled here.
/// - The function enables pre-norm output on both contexts itself.
/// - `on_prefill_complete` fires exactly once, right after the prompt is
///   fully prefilled into BOTH the target and the draft (via the per-chunk
///   `sync_capture`), but BEFORE the first generation pass. The worker uses
///   this hook to snapshot target + draft state at the prompt boundary so
///   the next turn's LCP trim has zero rollback distance on the common
///   path — see the matching note on the standard streaming generator.
/// - Termination: EOG token, `max_tokens` budget, context window, cancellation,
///   or disconnected receiver. A terminal chunk (`is_complete = true`,
///   `token_count = 0`, `finish_reason = Some(..)`) is emitted on every normal
///   exit; cancellation/disconnect short-circuit without a terminal chunk
///   (matching the non-MTP path).
#[allow(clippy::too_many_arguments)]
pub fn generate_stream_mtp<F>(
    model: &LlamaModel,
    target: &mut LlamaContext,
    draft: &mut LlamaContext,
    prompt: &str,
    request: &GenerationRequest,
    stream_sender: &mpsc::Sender<Result<StreamChunk, QueueError>>,
    cancellation_token: &CancellationToken,
    batch_size: usize,
    template_token_count: Option<usize>,
    mtp_params: MtpParams,
    on_prefill_complete: F,
) -> Result<(), GenerationError>
where
    F: FnOnce(&LlamaContext, &LlamaContext),
{
    // Pre-norm output is required: the target rows feed the draft mirror;
    // the draft rows seed the AR draft step. Reference uses masked=false on the
    // target (emit rows for every position) and masked=true on the draft (only
    // logits-requesting positions).
    target.set_embeddings_pre_norm(true, /* masked */ false);
    draft.set_embeddings_pre_norm(true, /* masked */ true);

    let n_embd = usize::try_from(model.n_embd()).expect("n_embd > 0 fits into usize");
    let mut session = MtpSession::new(n_embd, mtp_params);

    // Tokenize the full prompt; honor the target-side KV-reuse offset so only
    // the new tokens are prefilled here.
    let prompt_tokens = model
        .str_to_token(prompt, AddBos::Always)
        .map_err(GenerationError::tokenization)?;
    let total_token_count = prompt_tokens.len();
    let template_offset = template_token_count.unwrap_or(0);

    if budget::template_offset_exhausted(template_offset, total_token_count) {
        warn!(
            "MTP streaming: template_offset ({}) >= total ({}); no new tokens to process",
            template_offset, total_token_count
        );
        return finish(stream_sender, "No new tokens to process");
    }
    if total_token_count == 0 {
        return finish(stream_sender, "Empty prompt");
    }

    // Prefill the new prompt tokens on the target, requesting logits/pre-norm
    // for every position. We MUST `sync_capture` onto the draft *per chunk*
    // (not after the whole prefill): `get_embeddings_pre_norm_ith` only holds
    // rows for the most recent decode batch, so a single end-of-prefill mirror
    // would read garbage for everything before the last chunk. Per-chunk
    // mirroring also keeps `argmax_at` reading from the LAST batch's index
    // space (`0..last_chunk_len`), not the cumulative prompt length — that
    // off-by-batch bug emits 0 tokens silently on prompts wider than
    // `batch_size`.
    let mut batch = LlamaBatch::new(batch_size, 1);
    let new_tokens: &[LlamaToken] = &prompt_tokens[template_offset..];
    let mut absolute_position = template_offset;
    let mut last_chunk_len: usize = 0;
    for chunk in new_tokens.chunks(batch_size) {
        if check_cancelled(cancellation_token, stream_sender) {
            return Ok(());
        }
        batch.clear();
        let chunk_positions: Vec<i32> = (0..chunk.len())
            .map(|i| {
                i32::try_from(absolute_position + i).expect("prefill position fits into i32")
            })
            .collect();
        for (i, token) in chunk.iter().enumerate() {
            batch
                .add(*token, chunk_positions[i], &[SEQ_ID], true)
                .map_err(GenerationError::batch)?;
        }
        target
            .decode(&mut batch)
            .map_err(GenerationError::decoding)?;
        // Mirror just-decoded chunk onto the draft: target's pre-norm buffer
        // still holds these rows; the next decode overwrites them.
        session.sync_capture(target, draft, chunk, &chunk_positions, SEQ_ID);
        absolute_position += chunk.len();
        last_chunk_len = chunk.len();
    }

    // Post-prefill checkpoint: target and draft KVs both end exactly at the
    // prompt boundary. Fire the worker's snapshot hook now so the next turn
    // restores a clean prompt-boundary state and the n_rs_seq window never
    // has to roll back over the upcoming MTP-accepted generated tokens.
    on_prefill_complete(target, draft);

    // Sample the first id_last from the last prefilled position's logits —
    // indexed within the LAST batch (size = last_chunk_len), not the full
    // prompt length.
    let last_batch_idx = i32::try_from(last_chunk_len - 1).expect("last chunk len fits into i32");
    let mut id_last = argmax_at(target, last_batch_idx)?;
    let mut n_past = i32::try_from(total_token_count).expect("n_past fits into i32");

    let max_tokens = budget::generation_budget(request.max_tokens);
    let context_size = target.n_ctx() as usize;
    let mut tokens_generated: usize = 0;
    // Acceptance telemetry — counted by phase so we can tell whether MTP is
    // actually winning: ideal is `accepted_total > 0` and `target_passes <
    // tokens_generated` (a real speedup). Logged once per turn at the end of
    // the loop alongside the elapsed time.
    let mut accepted_total: usize = 0;
    let mut target_passes: usize = 0;
    let mut empty_drafts: usize = 0;
    let turn_start = std::time::Instant::now();

    let log_stats = |reason: &str,
                     tokens: usize,
                     passes: usize,
                     accepted: usize,
                     empty: usize| {
        let elapsed = turn_start.elapsed();
        let tps = if elapsed.as_secs_f64() > 0.0 {
            (tokens as f64) / elapsed.as_secs_f64()
        } else {
            0.0
        };
        let acc_rate = if passes > 0 {
            (accepted as f64) / (passes as f64)
        } else {
            0.0
        };
        let speedup = if passes > 0 {
            (tokens as f64) / (passes as f64)
        } else {
            0.0
        };
        tracing::info!(
            "MTP turn done: reason={reason} tokens={tokens} target_passes={passes} \
             accepted_total={accepted} empty_drafts={empty} acc_per_pass={acc_rate:.2} \
             tokens_per_pass={speedup:.2} elapsed={elapsed:.2?} throughput={tps:.1}tok/s"
        );
    };

    // If the very first token would be EOG, end immediately (matching the
    // non-MTP path's "stop before emitting EOG" behavior).
    if model.is_eog_token(id_last) {
        log_stats("EndOfSequence(first)", 0, 0, 0, 0);
        return finish(stream_sender, "EndOfSequence");
    }

    // Emit the first generated token (sampled from prefill) before the
    // speculative loop; subsequent iterations emit n_accepted + 1 tokens each.
    if let Some(()) = emit_token(model, id_last, stream_sender)? {
        // disconnected
        return Ok(());
    }
    tokens_generated += 1;

    loop {
        if check_cancelled(cancellation_token, stream_sender) {
            log_stats("Cancelled", tokens_generated, target_passes, accepted_total, empty_drafts);
            return Ok(());
        }
        if tokens_generated >= max_tokens {
            log_stats("MaxTokens", tokens_generated, target_passes, accepted_total, empty_drafts);
            return finish(stream_sender, "MaxTokens");
        }
        if budget::reached_context_limit(total_token_count, tokens_generated, context_size) {
            log_stats("ContextWindowFull", tokens_generated, target_passes, accepted_total, empty_drafts);
            return finish(stream_sender, "ContextWindowFull");
        }

        // Propose drafts via the MTP head.
        let drafts = session.draft(draft, id_last, n_past);

        target_passes += 1;
        let (accepted, next_token, n_accepted) = if drafts.is_empty() {
            empty_drafts += 1;
            // Non-speculative fallback: decode id_last alone on the target,
            // sample next, and mirror id_last onto the draft. Keeps the loop
            // making progress when the draft has nothing confident to propose.
            batch.clear();
            batch
                .add(id_last, n_past, &[SEQ_ID], true)
                .map_err(GenerationError::batch)?;
            target
                .decode(&mut batch)
                .map_err(GenerationError::decoding)?;
            let next = argmax_at(target, 0)?;
            session.sync_capture(target, draft, &[id_last], &[n_past], SEQ_ID);
            (Vec::<LlamaToken>::new(), next, 0usize)
        } else {
            // Speculative verify + accept. `verify` owns the id_last decode.
            let outcome = session
                .verify(target, id_last, &drafts, n_past, SEQ_ID)
                .map_err(|e| GenerationError::GenerationFailed(format!("MTP verify: {e}")))?;
            session
                .accept(target, outcome.n_accepted, outcome.next_pos, SEQ_ID)
                .map_err(|e| GenerationError::GenerationFailed(format!("MTP accept: {e}")))?;

            // Mirror id_last + accepted_drafts (n_accepted+1 tokens) onto the
            // draft — what the target's KV actually retained after accept.
            let mut mirror_tokens = Vec::with_capacity(outcome.n_accepted + 1);
            mirror_tokens.push(id_last);
            mirror_tokens.extend_from_slice(&outcome.accepted);
            let mirror_positions: Vec<i32> = (0..mirror_tokens.len())
                .map(|k| n_past + i32::try_from(k).expect("mirror position fits into i32"))
                .collect();
            session.sync_capture(target, draft, &mirror_tokens, &mirror_positions, SEQ_ID);

            accepted_total += outcome.n_accepted;
            (outcome.accepted, outcome.next_token, outcome.n_accepted)
        };

        // Emit accepted drafts + next_token, stopping early on EOG or
        // max_tokens.
        for token in accepted.iter().chain(std::iter::once(&next_token)) {
            if check_cancelled(cancellation_token, stream_sender) {
                log_stats("Cancelled", tokens_generated, target_passes, accepted_total, empty_drafts);
                return Ok(());
            }
            if model.is_eog_token(*token) {
                log_stats("EndOfSequence", tokens_generated, target_passes, accepted_total, empty_drafts);
                return finish(stream_sender, "EndOfSequence");
            }
            if let Some(()) = emit_token(model, *token, stream_sender)? {
                log_stats("Disconnected", tokens_generated, target_passes, accepted_total, empty_drafts);
                return Ok(());
            }
            tokens_generated += 1;
            if tokens_generated >= max_tokens {
                log_stats("MaxTokens", tokens_generated, target_passes, accepted_total, empty_drafts);
                return finish(stream_sender, "MaxTokens");
            }
        }

        // Advance: id_last = target's frontier choice; n_past = first un-decoded
        // position (== n_past + n_accepted + 1, the verified next_pos).
        id_last = next_token;
        n_past += i32::try_from(n_accepted + 1).expect("n_past delta fits into i32");
    }
}

/// The argmax token at `i_batch` in the context's logits, or an error if no
/// candidates exist (a backend fault).
fn argmax_at(ctx: &LlamaContext, i_batch: i32) -> Result<LlamaToken, GenerationError> {
    ctx.candidates_ith(i_batch)
        .max_by(|a, b| a.logit().total_cmp(&b.logit()))
        .map(|d| d.id())
        .ok_or_else(|| {
            GenerationError::GenerationFailed(format!(
                "MTP: no candidates at logits batch index {i_batch}"
            ))
        })
}

/// Emit a single non-EOG token as a `StreamChunk`. Returns `Ok(None)` on
/// success and `Ok(Some(()))` when the receiver has truly disconnected (caller
/// should stop). Tokens that fail to decode to a string are silently dropped
/// (matching the non-MTP streaming generator's behaviour for partial UTF-8
/// sequences).
fn emit_token(
    model: &LlamaModel,
    token: LlamaToken,
    stream_sender: &mpsc::Sender<Result<StreamChunk, QueueError>>,
) -> Result<Option<()>, GenerationError> {
    match model.token_to_bytes(token, Special::Tokenize) {
        Ok(bytes) => {
            let text = String::from_utf8_lossy(&bytes).into_owned();
            let chunk = StreamChunk {
                text,
                is_complete: false,
                token_count: 1,
                finish_reason: None,
            };
            if send_with_backpressure(stream_sender, Ok(chunk)).is_err() {
                debug!("MTP streaming: receiver disconnected");
                return Ok(Some(()));
            }
            Ok(None)
        }
        Err(_e) => {
            // Failed token-to-bytes (rare); still count progress like the
            // non-MTP path. Drop the text but keep going.
            Ok(None)
        }
    }
}

/// Send the terminal completion chunk (`is_complete=true`, `token_count=0`,
/// `finish_reason=Some(Stopped(reason))`).
fn finish(
    stream_sender: &mpsc::Sender<Result<StreamChunk, QueueError>>,
    reason: &str,
) -> Result<(), GenerationError> {
    let final_chunk = StreamChunk {
        text: String::new(),
        is_complete: true,
        token_count: 0,
        finish_reason: Some(FinishReason::Stopped(reason.to_string())),
    };
    let _ = send_with_backpressure(stream_sender, Ok(final_chunk));
    Ok(())
}


/// Check the cancellation token; on cancellation, push the standard queue
/// "Request cancelled" error and return `true` so the caller can short-circuit.
fn check_cancelled(
    cancellation_token: &CancellationToken,
    stream_sender: &mpsc::Sender<Result<StreamChunk, QueueError>>,
) -> bool {
    if cancellation_token.is_cancelled() {
        let _ = send_with_backpressure(
            stream_sender,
            Err(QueueError::WorkerError("Request cancelled".to_string())),
        );
        true
    } else {
        false
    }
}
