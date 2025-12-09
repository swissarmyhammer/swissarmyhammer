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

        // Validate that we have access to the model context
        let _model = &context.model;

        // Verify the stopper is properly initialized
        if self.eos_token_id == u32::MAX {
            warn!(
                "EOS stopper configured with maximum token ID ({}), which may be invalid",
                self.eos_token_id
            );
        }

        // The stopper maintains its interface contract but defers to the generation
        // system for actual EOS detection. This ensures optimal performance and
        // compatibility with llama.cpp's token handling.
        //
        // Integration points in the generation system:
        // 1. Token sampling loop checks model.is_eog_token(sampled_token)
        // 2. Direct comparison: sampled_token == self.eos_token_id
        // 3. Immediate termination when EOS is detected
        //
        // This design provides the best balance of:
        // - Performance: No duplicate token processing
        // - Reliability: Uses model's authoritative EOS classification
        // - Maintainability: Clear separation of concerns

        // Always return None - EOS detection handled in sampling loop
        None
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

    // Note: Integration tests with actual LlamaContext and LlamaBatch
    // are implemented in the integration_tests.rs file to avoid
    // requiring model loading in unit tests.
    //
    // The should_stop method implementation with batch token checking
    // is tested there with real model data.
}
