//! Pure, model-free helpers for the MTP draft→verify→accept orchestration.
//!
//! These functions encode the decision rules of the MTP speculative-decoding
//! loop without touching a live context, so they can be exercised by fast unit
//! tests independent of any model. They are a verbatim port of the reference
//! `examples/mtp/src/helpers.rs` in the `llama-cpp-rs` fork (see that fork's
//! `mtp-orchestration.md`); keeping them identical lets the fork's reference and
//! this consumer share one definition of the loop's index arithmetic.

use llama_cpp_2::token::LlamaToken;

/// Length of the longest prefix where the target and draft tokens agree.
///
/// This is the verify accept rule: walk both slices in lockstep and count
/// matching tokens until the first divergence (or until the shorter slice ends).
#[must_use]
pub fn longest_accepted_prefix(target: &[LlamaToken], draft: &[LlamaToken]) -> usize {
    target
        .iter()
        .zip(draft.iter())
        .take_while(|(t, d)| t == d)
        .count()
}

/// The `verify_h` row index to carry forward as `pending_h` after acceptance.
///
/// Mirrors the reference `accept()` row pick: the pre-norm row for the last
/// accepted token, clamped to the last available row so it never indexes past
/// the captured rows. Returns `min(n_accepted, n_rows.saturating_sub(1))`.
#[must_use]
pub fn accept_h_index(n_accepted: usize, n_rows: usize) -> usize {
    n_accepted.min(n_rows.saturating_sub(1))
}

/// The right-shifted `verify_h` row mapping for the draft mirror batch.
///
/// When the target accepts `n` sequential tokens, the draft context must be fed
/// the same `n` tokens, each paired with the hidden row that *precedes* it (the
/// MTP head predicts token `k+1` from `(token_k, h_k)`). Slot 0 pairs with the
/// carry `pending_h` (returned as `None`); slot `k >= 1` pairs with `verify_h`
/// row `k - 1`. An off-by-one here silently collapses acceptance to ~0, so it is
/// isolated and unit-tested.
///
/// Returns a vector of length `n`: `[None, Some(0), …, Some(n - 2)]`.
#[must_use]
pub fn shift_h_mapping(n: usize) -> Vec<Option<usize>> {
    (0..n).map(|k| k.checked_sub(1)).collect()
}

/// Resolve a verified step's `n_accepted` and guaranteed next token from the
/// target's per-batch-index greedy choices.
///
/// The target decodes the batch `[id_last, draft_0, …, draft_{n-1}]`, so its
/// greedy choice at batch index `i` predicts the token that should *follow*
/// batch entry `i`: `target_chosen[0]` should match `draft_0`, `target_chosen[i]`
/// should match `draft_i`. The accepted prefix is the longest run where
/// `target_chosen[i] == draft_i`; the guaranteed next token (the +1) is the
/// target's choice at the frontier, `target_chosen[n_accepted]` — well-defined
/// for every `n_accepted` in `0..=n` because `target_chosen` has `n + 1` entries.
///
/// # Panics
///
/// Panics unless `target_chosen.len() == drafts.len() + 1`.
#[must_use]
pub fn verify_acceptance(
    target_chosen: &[LlamaToken],
    drafts: &[LlamaToken],
) -> (usize, LlamaToken) {
    assert_eq!(
        target_chosen.len(),
        drafts.len() + 1,
        "verify expects one target choice per logit position (drafts.len() + 1)",
    );
    let n_accepted = longest_accepted_prefix(&target_chosen[..drafts.len()], drafts);
    (n_accepted, target_chosen[n_accepted])
}

/// Compose a verified step's emitted tokens: the accepted draft prefix plus the
/// target's guaranteed next token.
///
/// The data-level shape of speculative decoding's "+1" invariant: a verified
/// step always emits the `n_accepted` accepted drafts followed by the target's
/// own next token. Even with `n_accepted == 0` exactly one token (`next_token`)
/// is emitted, so the loop always makes forward progress.
#[must_use]
pub fn compose_emitted(accepted: &[LlamaToken], next_token: LlamaToken) -> Vec<LlamaToken> {
    let mut emitted = accepted.to_vec();
    emitted.push(next_token);
    emitted
}

/// The `count` sequential decode positions starting at `start`:
/// `[start, start + 1, …, start + count - 1]`.
///
/// # Panics
///
/// Panics if any position does not fit into an [`i32`].
#[must_use]
pub fn sequential_positions(start: i32, count: usize) -> Vec<i32> {
    (0..count)
        .map(|k| {
            start
                .checked_add(i32::try_from(k).expect("position offset fits into i32"))
                .expect("decode position fits into i32")
        })
        .collect()
}

/// Whether the draft loop should stop after the most recent draft token.
///
/// Stops when the model is no longer confident enough (top-1 probability below
/// `p_min`) or the per-step budget is exhausted (`drafted_len >= n_max`).
#[must_use]
pub fn draft_should_stop(top1_prob: f32, p_min: f32, drafted_len: usize, n_max: usize) -> bool {
    top1_prob < p_min || drafted_len >= n_max
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(ids: &[i32]) -> Vec<LlamaToken> {
        ids.iter().copied().map(LlamaToken).collect()
    }

    #[test]
    fn longest_accepted_prefix_empty_inputs() {
        assert_eq!(longest_accepted_prefix(&[], &[]), 0);
        assert_eq!(longest_accepted_prefix(&toks(&[1, 2]), &[]), 0);
        assert_eq!(longest_accepted_prefix(&[], &toks(&[1, 2])), 0);
    }

    #[test]
    fn longest_accepted_prefix_full_match() {
        assert_eq!(
            longest_accepted_prefix(&toks(&[1, 2, 3]), &toks(&[1, 2, 3])),
            3
        );
    }

    #[test]
    fn longest_accepted_prefix_partial_match() {
        assert_eq!(
            longest_accepted_prefix(&toks(&[1, 2, 9]), &toks(&[1, 2, 3])),
            2
        );
    }

    #[test]
    fn longest_accepted_prefix_divergent_at_zero() {
        assert_eq!(
            longest_accepted_prefix(&toks(&[9, 2, 3]), &toks(&[1, 2, 3])),
            0
        );
    }

    #[test]
    fn longest_accepted_prefix_stops_at_shorter_slice() {
        assert_eq!(
            longest_accepted_prefix(&toks(&[1, 2]), &toks(&[1, 2, 3])),
            2
        );
    }

    #[test]
    fn accept_h_index_accepted_below_rows() {
        assert_eq!(accept_h_index(1, 4), 1);
    }

    #[test]
    fn accept_h_index_accepted_equals_rows_clamps() {
        assert_eq!(accept_h_index(4, 4), 3);
    }

    #[test]
    fn accept_h_index_no_rows() {
        assert_eq!(accept_h_index(0, 0), 0);
        assert_eq!(accept_h_index(3, 0), 0);
    }

    #[test]
    fn accept_h_index_single_row() {
        assert_eq!(accept_h_index(0, 1), 0);
        assert_eq!(accept_h_index(5, 1), 0);
    }

    #[test]
    fn shift_h_mapping_typical() {
        assert_eq!(shift_h_mapping(4), vec![None, Some(0), Some(1), Some(2)]);
    }

    #[test]
    fn shift_h_mapping_single_row() {
        assert_eq!(shift_h_mapping(1), vec![None]);
    }

    #[test]
    fn shift_h_mapping_empty() {
        assert_eq!(shift_h_mapping(0), Vec::<Option<usize>>::new());
    }

    #[test]
    fn shift_h_mapping_indices_are_one_behind_slot() {
        let map = shift_h_mapping(8);
        assert_eq!(map[0], None);
        for (slot, entry) in map.iter().enumerate().skip(1) {
            assert_eq!(*entry, Some(slot - 1));
        }
    }

    #[test]
    fn verify_acceptance_full_match_takes_frontier_next() {
        let target_chosen = toks(&[1, 2, 3, 4]);
        let drafts = toks(&[1, 2, 3]);
        assert_eq!(
            verify_acceptance(&target_chosen, &drafts),
            (3, LlamaToken(4))
        );
    }

    #[test]
    fn verify_acceptance_partial_match_next_is_frontier_choice() {
        let target_chosen = toks(&[1, 2, 9, 99]);
        let drafts = toks(&[1, 2, 3]);
        assert_eq!(
            verify_acceptance(&target_chosen, &drafts),
            (2, LlamaToken(9))
        );
    }

    #[test]
    fn verify_acceptance_zero_match_next_is_id_last_prediction() {
        let target_chosen = toks(&[5, 6, 7]);
        let drafts = toks(&[1, 2]);
        assert_eq!(
            verify_acceptance(&target_chosen, &drafts),
            (0, LlamaToken(5))
        );
    }

    #[test]
    fn verify_acceptance_single_draft_accepted() {
        let target_chosen = toks(&[1, 2]);
        let drafts = toks(&[1]);
        assert_eq!(
            verify_acceptance(&target_chosen, &drafts),
            (1, LlamaToken(2))
        );
    }

    #[test]
    fn verify_acceptance_single_draft_rejected() {
        let target_chosen = toks(&[9, 8]);
        let drafts = toks(&[1]);
        assert_eq!(
            verify_acceptance(&target_chosen, &drafts),
            (0, LlamaToken(9))
        );
    }

    #[test]
    #[should_panic(expected = "one target choice per logit position")]
    fn verify_acceptance_rejects_mismatched_lengths() {
        let _ = verify_acceptance(&toks(&[1, 2]), &toks(&[1, 2]));
    }

    #[test]
    fn compose_emitted_appends_next_after_full_prefix() {
        assert_eq!(
            compose_emitted(&toks(&[1, 2, 3]), LlamaToken(4)),
            toks(&[1, 2, 3, 4]),
        );
    }

    #[test]
    fn compose_emitted_partial_prefix() {
        assert_eq!(
            compose_emitted(&toks(&[1, 2]), LlamaToken(9)),
            toks(&[1, 2, 9])
        );
    }

    #[test]
    fn compose_emitted_zero_accepted_still_emits_next() {
        assert_eq!(compose_emitted(&[], LlamaToken(7)), toks(&[7]));
    }

    #[test]
    fn sequential_positions_typical() {
        assert_eq!(sequential_positions(5, 3), vec![5, 6, 7]);
    }

    #[test]
    fn sequential_positions_single() {
        assert_eq!(sequential_positions(0, 1), vec![0]);
    }

    #[test]
    fn sequential_positions_empty() {
        assert_eq!(sequential_positions(9, 0), Vec::<i32>::new());
    }

    #[test]
    fn draft_should_stop_below_p_min() {
        assert!(draft_should_stop(0.4, 0.5, 0, 4));
    }

    #[test]
    fn draft_should_stop_at_p_min_continues() {
        assert!(!draft_should_stop(0.5, 0.5, 0, 4));
    }

    #[test]
    fn draft_should_stop_at_n_max() {
        assert!(draft_should_stop(0.99, 0.0, 4, 4));
    }

    #[test]
    fn draft_should_stop_below_n_max_continues() {
        assert!(!draft_should_stop(0.99, 0.0, 3, 4));
    }

    #[test]
    fn draft_should_stop_past_n_max() {
        assert!(draft_should_stop(0.99, 0.0, 5, 4));
    }
}
