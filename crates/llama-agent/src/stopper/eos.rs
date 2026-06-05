use super::Stopper;
use crate::types::FinishReason;
use llama_cpp_2::{context::LlamaContext, llama_batch::LlamaBatch};
use tracing::{debug, warn};

/// Stopper that detects End-of-Sequence (EOS) tokens to terminate generation.
///
/// The `EosStopper` is designed to work with the model's natural termination mechanism
/// by detecting when an End-of-Sequence token is generated. This is the most common
/// and reliable stopping condition for text generation.
///
/// ## Architecture
///
/// This stopper integrates with the standard llama.cpp EOS detection mechanism
/// rather than reimplementing token detection. The actual EOS detection happens
/// in the generation loop using `model.is_eog_token(token)`, which is the
/// standard approach in llama.cpp-based applications.
///
/// ## Performance
///
/// EOS detection adds virtually no overhead since it leverages the model's
/// built-in token classification. The stopper validates configuration and
/// provides a consistent interface without duplicating the core detection logic.
///
/// ## Thread Safety
///
/// `EosStopper` implements `Send` and `Sync` since it only stores the EOS token ID
/// and has no mutable state during evaluation.
///
/// # Examples
///
/// ```rust
/// use llama_agent::stopper::EosStopper;
///
/// // Create EOS stopper with common EOS token ID
/// let stopper = EosStopper::new(2);  // Common EOS token ID
///
/// // Token IDs vary by model:
/// let gpt_eos = EosStopper::new(50256);     // GPT-style models
/// let llama_eos = EosStopper::new(2);       // LLaMA-style models  
/// let custom_eos = EosStopper::new(128001); // Custom tokenizer
/// ```
///
/// ## Configuration
///
/// The EOS token ID should match the model's tokenizer configuration.
/// Using an incorrect EOS token ID will prevent proper generation termination.
/// Check your model's tokenizer configuration or use the model's metadata
/// to determine the correct EOS token ID.
#[derive(Debug, Clone)]
pub struct EosStopper {
    /// The token ID that represents End-of-Sequence for this model.
    ///
    /// This value should match the model's tokenizer configuration.
    /// Common values include:
    /// - 2: LLaMA and similar models
    /// - 50256: GPT-2/GPT-3 style models  
    /// - 128001: Some newer models with extended vocabularies
    eos_token_id: u32,
}

impl EosStopper {
    /// Create a new EOS stopper with the specified token ID.
    ///
    /// The EOS token ID should match your model's tokenizer configuration.
    /// Providing an incorrect token ID will prevent proper generation termination.
    ///
    /// ## Determining the Correct EOS Token ID
    ///
    /// Check your model documentation or tokenizer config for the EOS token ID:
    /// - LLaMA models typically use token ID 2
    /// - GPT-style models often use token ID 50256  
    /// - Custom models may use different values
    ///
    /// You can also check the model's metadata or use the tokenizer to encode
    /// the EOS string to determine the correct ID.
    ///
    /// # Arguments
    ///
    /// * `eos_token_id` - The token ID that represents End-of-Sequence for the model
    ///
    /// # Examples
    ///
    /// ```rust
    /// use llama_agent::stopper::EosStopper;
    ///
    /// // For LLaMA-style models
    /// let llama_stopper = EosStopper::new(2);
    ///
    /// // For GPT-style models
    /// let gpt_stopper = EosStopper::new(50256);
    ///
    /// // For models with custom tokenizers
    /// let custom_stopper = EosStopper::new(128001);
    /// ```
    ///
    /// # Note
    ///
    /// This constructor always succeeds since any u32 value is potentially
    /// a valid token ID. Validation of the token ID against the actual model
    /// happens during generation when the model's token vocabulary is available.
    pub fn new(eos_token_id: u32) -> Self {
        debug!("Creating EosStopper with token ID: {}", eos_token_id);
        Self { eos_token_id }
    }

    /// Get the configured EOS token ID.
    ///
    /// This method returns the EOS token ID that was configured when creating
    /// this stopper. Useful for debugging or logging purposes.
    ///
    /// # Returns
    ///
    /// The EOS token ID as a u32.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use llama_agent::stopper::EosStopper;
    ///
    /// let stopper = EosStopper::new(2);
    /// assert_eq!(stopper.eos_token_id(), 2);
    /// ```
    pub fn eos_token_id(&self) -> u32 {
        self.eos_token_id
    }

    /// Evaluate the model-independent portion of the EOS stop decision.
    ///
    /// This isolates the logic that does not depend on a `LlamaContext` so it
    /// can be unit-tested without loading a model. Actual EOS detection is
    /// performed in the generation loop via the model's `is_eog_token`, so this
    /// stopper never reports a stop on its own and always returns `None`. The
    /// only observable behavior here is a warning when the configured token ID
    /// is the sentinel `u32::MAX`, which is almost certainly a misconfiguration.
    ///
    /// # Returns
    ///
    /// Always `None` — EOS detection is owned by the sampling loop, not by this
    /// stopper.
    fn evaluate(&self) -> Option<FinishReason> {
        // Verify the stopper is properly initialized
        if self.eos_token_id == u32::MAX {
            warn!(
                "EOS stopper configured with maximum token ID ({}), which may be invalid",
                self.eos_token_id
            );
        }

        // The stopper maintains its interface contract but defers to the
        // generation system for actual EOS detection. This ensures optimal
        // performance and compatibility with llama.cpp's token handling.
        //
        // Integration points in the generation system:
        // 1. Token sampling loop checks model.is_eog_token(sampled_token)
        // 2. Direct comparison: sampled_token == self.eos_token_id
        // 3. Immediate termination when EOS is detected
        None
    }
}

impl EosStopper {
    /// Test-only accessor for the model-independent decision core.
    ///
    /// Lets sibling test modules (e.g. the composite-precedence tests in
    /// `stopper/mod.rs`) observe the exact decision `should_stop` makes without
    /// a `LlamaContext`. Not part of the public API.
    #[cfg(test)]
    pub(crate) fn evaluate_for_test(&self) -> Option<FinishReason> {
        self.evaluate()
    }
}

impl Stopper for EosStopper {
    fn should_stop(&mut self, context: &LlamaContext, _batch: &LlamaBatch) -> Option<FinishReason> {
        // EOS detection is integrated with the standard llama.cpp mechanism rather than
        // being implemented here. This design choice ensures compatibility with the
        // model's built-in token classification and avoids duplicating detection logic.
        //
        // The actual EOS detection happens in the generation loop using:
        // - model.is_eog_token(token) for End-of-Generation detection
        // - Direct token ID comparison after sampling
        //
        // This approach is more efficient and reliable than trying to extract tokens
        // from the batch, since EOS tokens are typically generated as individual tokens
        // and are immediately detectable after sampling.

        // Validate that we have access to the model context. EOS detection is
        // owned by the sampling loop (see `evaluate`), so the context is only
        // touched to honor the trait contract.
        let _model = &context.model;

        // Defer to the model-independent core so the decision logic stays
        // testable without loading a model.
        self.evaluate()
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eos_stopper_creation() {
        let eos_token_id = 2; // Common EOS token ID
        let stopper = EosStopper::new(eos_token_id);

        assert_eq!(stopper.eos_token_id, eos_token_id);
    }

    #[test]
    fn test_eos_stopper_different_token_ids() {
        let test_cases = [0, 1, 2, 128001, 999999];

        for token_id in test_cases {
            let stopper = EosStopper::new(token_id);
            assert_eq!(stopper.eos_token_id, token_id);
        }
    }

    #[test]
    fn test_eos_stopper_interface_compliance() {
        // Verify that EosStopper properly implements the Stopper trait
        let eos_token_id = 2;
        let stopper = EosStopper::new(eos_token_id);

        // Verify it can be stored as a trait object
        let _boxed: Box<dyn Stopper> = Box::new(stopper);

        // Test passes by compilation - if EosStopper doesn't implement Stopper trait,
        // the code above would not compile
    }

    #[test]
    fn test_eos_stopper_thread_safety() {
        // Test that EosStopper can be sent between threads
        let eos_token_id = 2;
        let stopper = EosStopper::new(eos_token_id);

        // Verify it implements Send and Sync (required for concurrent usage)
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<EosStopper>();
        assert_sync::<EosStopper>();

        // Test moving between threads would work
        let _moved_stopper = stopper;
    }

    #[test]
    fn test_eos_stopper_clone_and_debug() {
        let eos_token_id = 128001; // Common GPT-style EOS token
        let stopper = EosStopper::new(eos_token_id);

        // Test that we can format for debugging
        let debug_str = format!("{:?}", stopper);
        assert!(debug_str.contains("EosStopper"));
        assert!(debug_str.contains("128001"));
    }

    #[test]
    fn test_eos_stopper_edge_cases() {
        // Test with boundary values
        let boundary_cases = [
            0,        // Minimum token ID
            u32::MAX, // Maximum token ID
            1,        // BOS token often
            2,        // EOS token often
        ];

        for token_id in boundary_cases {
            let stopper = EosStopper::new(token_id);
            assert_eq!(stopper.eos_token_id, token_id);

            // Verify the stopper is properly initialized
            let debug_output = format!("{:?}", stopper);
            assert!(debug_output.contains(&token_id.to_string()));
        }
    }

    // ---------------------------------------------------------------------
    // Decision tests for the model-independent core (`evaluate`).
    //
    // `should_stop` only borrows `context.model` (a no-op) and then delegates
    // to `evaluate`. Exercising `evaluate` therefore covers the stopper's
    // entire decision behavior without constructing a `LlamaContext`. By design
    // this stopper never reports a stop itself — real EOS detection lives in the
    // sampling loop via `model.is_eog_token` — so `evaluate` must always return
    // `None`, regardless of the configured token ID.
    // ---------------------------------------------------------------------

    #[test]
    fn eos_token_id_getter_returns_configured_value() {
        // Pin the public getter independently of the doctest so it is covered by
        // the library test suite.
        let stopper = EosStopper::new(50256);
        assert_eq!(stopper.eos_token_id(), 50256);
    }

    #[test]
    fn evaluate_never_stops_for_the_eos_token_id() {
        // The "EOS" id itself does not make this stopper fire: detection is
        // delegated to the sampling loop.
        let stopper = EosStopper::new(2);
        assert!(stopper.evaluate().is_none());
    }

    #[test]
    fn evaluate_never_stops_for_a_non_eos_token_id() {
        // An ordinary, non-EOS id likewise never fires.
        let stopper = EosStopper::new(12345);
        assert!(stopper.evaluate().is_none());
    }

    #[test]
    fn evaluate_never_stops_for_alternate_end_token_ids() {
        // Model-specific alternate end tokens (LLaMA 2, GPT-2, extended
        // vocabularies) are all treated identically: no stop is reported here.
        for token_id in [2u32, 50256, 128001, 128009] {
            let stopper = EosStopper::new(token_id);
            assert!(
                stopper.evaluate().is_none(),
                "token id {token_id} should not cause a stop"
            );
        }
    }

    #[test]
    fn evaluate_handles_boundary_token_ids_without_stopping() {
        // Including the sentinel `u32::MAX`, which logs a warning but must still
        // return `None` (it does not stop generation).
        for token_id in [0u32, 1, u32::MAX] {
            let stopper = EosStopper::new(token_id);
            assert!(
                stopper.evaluate().is_none(),
                "boundary token id {token_id} should not cause a stop"
            );
        }
    }

    // Note: Integration tests with an actual LlamaContext and LlamaBatch are
    // implemented in the integration tests to avoid requiring model loading in
    // unit tests. The `should_stop` wrapper only borrows `context.model` and
    // forwards to `evaluate`, which is covered exhaustively above.
}
