//! Pure, model-free arithmetic for the generation loops.
//!
//! These functions isolate the token-budget and context-window arithmetic that
//! governs every generation loop in [`super::GenerationHelper`]. They bind no
//! llama.cpp state, so they are exhaustively unit-testable without loading a
//! model — which matters because the "0 tokens generated" production bug
//! (`01KSNJ7CBK9333J0T9G4TCA7DH`) lived precisely in this arithmetic. Driving the
//! real decode loop needs a model and is covered by the small-model integration
//! tests in `tests/integration/streaming_generation.rs`; the *decisions* that
//! loop makes are pinned here.

/// Default new-token budget used when a request leaves `max_tokens` unset.
///
/// Matches the historical `unwrap_or(512)` fallback at every call site.
pub(crate) const DEFAULT_MAX_TOKENS: usize = 512;

/// Resolve the new-token generation budget from a request's `max_tokens`.
///
/// The budget is the number of NEW tokens to produce. It is deliberately **not**
/// reduced by the prompt length: the caller (the ACP agentic loop) already
/// derives the value from the remaining context window
/// (`context_size - current_tokens`). Subtracting the prompt length a second
/// time here was the double-subtraction that collapsed the budget to zero (or
/// underflowed `usize`) for large prompts — the "0 tokens generated" bug.
pub(crate) fn generation_budget(max_tokens: Option<u32>) -> usize {
    max_tokens.map(|t| t as usize).unwrap_or(DEFAULT_MAX_TOKENS)
}

/// Whether generation has reached the context-window limit and must stop.
///
/// Mirrors llama.cpp's own guard: once `prompt_tokens + tokens_generated`
/// reaches `context_size - 1`, the next `decode` would run past the window and
/// fail. Uses saturating arithmetic so a degenerate `context_size` of 0 yields a
/// limit of 0 (always stop) instead of panicking on `context_size - 1`.
pub(crate) fn reached_context_limit(
    prompt_tokens: usize,
    tokens_generated: usize,
    context_size: usize,
) -> bool {
    prompt_tokens + tokens_generated >= context_size.saturating_sub(1)
}

/// Whether a template offset leaves no new tokens to process.
///
/// When the cached template prefix covers the whole prompt there is nothing new
/// to decode, so generation completes immediately with an empty response rather
/// than entering the decode loop.
pub(crate) fn template_offset_exhausted(template_offset: usize, total_tokens: usize) -> bool {
    template_offset >= total_tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_defaults_when_unset() {
        assert_eq!(generation_budget(None), DEFAULT_MAX_TOKENS);
        assert_eq!(generation_budget(None), 512);
    }

    #[test]
    fn budget_uses_request_value_verbatim() {
        // The budget is NOT reduced by anything — it is the caller's value as-is.
        assert_eq!(generation_budget(Some(1)), 1);
        assert_eq!(generation_budget(Some(64)), 64);
        assert_eq!(generation_budget(Some(256)), 256);
        assert_eq!(generation_budget(Some(512)), 512);
        assert_eq!(generation_budget(Some(4096)), 4096);
    }

    #[test]
    fn budget_zero_is_preserved_not_defaulted() {
        // Some(0) is a real (if degenerate) caller choice and must NOT silently
        // become the 512 default; only `None` defaults.
        assert_eq!(generation_budget(Some(0)), 0);
    }

    #[test]
    fn budget_handles_max_u32_without_overflow() {
        assert_eq!(generation_budget(Some(u32::MAX)), u32::MAX as usize);
    }

    #[test]
    fn context_limit_not_reached_when_room_remains() {
        // prompt + generated well under context_size - 1.
        assert!(!reached_context_limit(10, 0, 4096));
        assert!(!reached_context_limit(10, 100, 4096));
    }

    #[test]
    fn context_limit_boundary_one_under_at_over() {
        // context_size = 100 => limit threshold is 99 (context_size - 1).
        // one under the threshold: 10 + 88 = 98 < 99 -> keep going.
        assert!(!reached_context_limit(10, 88, 100));
        // exactly at the threshold: 10 + 89 = 99 >= 99 -> stop.
        assert!(reached_context_limit(10, 89, 100));
        // one over the threshold: 10 + 90 = 100 >= 99 -> stop.
        assert!(reached_context_limit(10, 90, 100));
    }

    #[test]
    fn context_limit_saturates_on_degenerate_sizes() {
        // context_size = 0 must not panic (would be `0 - 1`); the saturating
        // limit is 0, so any position is "at or past" it -> stop immediately.
        assert!(reached_context_limit(0, 0, 0));
        // context_size = 1 => threshold 0 => stop immediately.
        assert!(reached_context_limit(0, 0, 1));
    }

    #[test]
    fn context_limit_large_prompt_stops_immediately() {
        // A prompt that already fills the window leaves no room to generate.
        assert!(reached_context_limit(4095, 0, 4096));
        assert!(reached_context_limit(5000, 0, 4096));
    }

    #[test]
    fn template_offset_not_exhausted_when_new_tokens_remain() {
        assert!(!template_offset_exhausted(100, 150));
        assert!(!template_offset_exhausted(0, 1));
    }

    #[test]
    fn template_offset_exhausted_at_and_past_total() {
        // Equal: the template covers the whole prompt -> nothing new.
        assert!(template_offset_exhausted(100, 100));
        // Past: offset claims more tokens than exist -> nothing new (and the
        // production code must not underflow on the subsequent `skip`).
        assert!(template_offset_exhausted(150, 100));
    }

    #[test]
    fn template_offset_exhausted_on_empty_prompt() {
        // total = 0 means there are no tokens at all; any offset exhausts it.
        assert!(template_offset_exhausted(0, 0));
    }
}
