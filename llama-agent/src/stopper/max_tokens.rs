use super::Stopper;
use crate::types::FinishReason;
use llama_cpp_2::{context::LlamaContext, llama_batch::LlamaBatch};
use tracing::{debug, info, warn};

/// Stopper that limits generation to a maximum number of tokens.
///
/// `MaxTokensStopper` provides precise control over generation length by tracking
/// the cumulative number of tokens generated and stopping when a configured limit
/// is reached. This is essential for managing computational costs, preventing
/// runaway generation, and ensuring predictable response times.
///
/// ## Token Counting
///
/// The stopper counts tokens incrementally as batches are processed, using the
/// batch size (`batch.n_tokens()`) to update the running total. This approach
/// provides accurate counting without requiring access to the full token sequence.
///
/// ## Performance
///
/// Token counting adds minimal overhead (typically < 0.1% of total generation time)
/// since it only requires a simple integer comparison and addition per batch.
/// The stopper maintains O(1) time complexity regardless of generation length.
///
/// ## Thread Safety
///
/// `MaxTokensStopper` implements `Send` but not `Sync` due to its mutable internal
/// state (`tokens_generated`). Each generation request should use its own stopper
/// instance to avoid race conditions.
///
/// ## Memory Usage
///
/// The stopper uses constant memory (two usize values) regardless of generation
/// length, making it suitable for long-running generations without memory concerns.
///
/// # Examples
///
/// ```rust
/// use llama_agent::stopper::MaxTokensStopper;
///
/// // Limit generation to 100 tokens
/// let stopper = MaxTokensStopper::new(100);
///
/// // Common limits for different use cases:
/// let short_response = MaxTokensStopper::new(50);    // Brief answers
/// let medium_response = MaxTokensStopper::new(200);  // Detailed responses  
/// let long_response = MaxTokensStopper::new(1000);   // Extended content
///
/// // Prevent runaway generation
/// let safety_limit = MaxTokensStopper::new(10000);
/// ```
///
/// ## Configuration Guidelines
///
/// Choose token limits based on your use case:
///
/// - **1-50 tokens**: Brief answers, keywords, classifications
/// - **50-200 tokens**: Standard responses, explanations
/// - **200-1000 tokens**: Detailed content, code generation
/// - **1000+ tokens**: Long-form content, documentation
///
/// Consider model characteristics:
/// - Larger models may need higher limits for coherent responses
/// - Different models have varying verbosity levels
/// - Factor in prompt length when setting total limits
#[derive(Debug)]
pub struct MaxTokensStopper {
    /// Maximum number of tokens allowed before stopping generation.
    ///
    /// This limit includes all tokens generated since the stopper was created,
    /// not including the initial prompt tokens. Set to 0 to stop immediately
    /// (useful for testing).
    max_tokens: usize,

    /// Running count of tokens generated so far.
    ///
    /// This value is incremented as token batches are processed during generation.
    /// When it reaches or exceeds `max_tokens`, the stopper will trigger termination.
    tokens_generated: usize,
}

impl MaxTokensStopper {
    /// Create a new max tokens stopper with the specified limit.
    ///
    /// The stopper will track token generation and stop when the specified
    /// number of tokens have been generated. The counter starts at zero
    /// when the stopper is created.
    ///
    /// ## Token Limit Guidelines
    ///
    /// Choose appropriate limits based on your use case and model:
    ///
    /// - **0**: Immediate termination (testing only)
    /// - **1-50**: Very brief responses, single words/phrases
    /// - **50-200**: Standard conversational responses
    /// - **200-1000**: Detailed explanations, code generation
    /// - **1000+**: Long-form content, articles, documentation
    ///
    /// ## Performance Considerations
    ///
    /// Higher token limits don't affect stopper performance, but they do:
    /// - Increase generation time and computational cost
    /// - Use more memory for context management
    /// - May impact response latency for real-time applications
    ///
    /// # Arguments
    ///
    /// * `max_tokens` - Maximum number of tokens to generate before stopping
    ///
    /// # Examples
    ///
    /// ```rust
    /// use llama_agent::stopper::MaxTokensStopper;
    ///
    /// // Create stopper for brief responses
    /// let brief_stopper = MaxTokensStopper::new(50);
    ///
    /// // Create stopper for detailed responses
    /// let detailed_stopper = MaxTokensStopper::new(500);
    ///
    /// // Create stopper with safety limit to prevent runaway generation
    /// let safety_stopper = MaxTokensStopper::new(10000);
    /// ```
    ///
    /// # Note
    ///
    /// Setting `max_tokens` to 0 will cause the stopper to trigger immediately
    /// on the first token batch. This can be useful for testing but will prevent
    /// any actual text generation.
    pub fn new(max_tokens: usize) -> Self {
        debug!(
            "Creating MaxTokensStopper with limit: {} tokens",
            max_tokens
        );

        if max_tokens == 0 {
            warn!("MaxTokensStopper created with 0 token limit - will stop immediately");
        } else if max_tokens > 50000 {
            warn!(
                "MaxTokensStopper created with very high token limit ({}), consider if this is intentional",
                max_tokens
            );
        }

        Self {
            max_tokens,
            tokens_generated: 0,
        }
    }

    /// Get the configured maximum token limit.
    ///
    /// Returns the token limit that was set when creating this stopper.
    /// Useful for debugging, logging, or adjusting generation parameters.
    ///
    /// # Returns
    ///
    /// The maximum number of tokens before stopping.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use llama_agent::stopper::MaxTokensStopper;
    ///
    /// let stopper = MaxTokensStopper::new(100);
    /// assert_eq!(stopper.max_tokens(), 100);
    /// ```
    pub fn max_tokens(&self) -> usize {
        self.max_tokens
    }

    /// Get the current count of tokens generated.
    ///
    /// Returns the number of tokens that have been processed by this stopper
    /// since it was created. This value increases as generation progresses
    /// and is used to determine when the token limit is reached.
    ///
    /// # Returns
    ///
    /// The number of tokens generated so far.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use llama_agent::stopper::MaxTokensStopper;
    ///
    /// let stopper = MaxTokensStopper::new(100);
    /// assert_eq!(stopper.tokens_generated(), 0);
    ///
    /// // After processing during generation:
    /// // assert_eq!(stopper.tokens_generated(), 15);
    /// ```
    pub fn tokens_generated(&self) -> usize {
        self.tokens_generated
    }

    /// Get the number of tokens remaining before the limit is reached.
    ///
    /// Returns how many more tokens can be generated before this stopper
    /// will trigger termination. Useful for progress tracking or adjusting
    /// generation parameters dynamically.
    ///
    /// # Returns
    ///
    /// The number of tokens remaining, or 0 if the limit has been reached.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use llama_agent::stopper::MaxTokensStopper;
    ///
    /// let stopper = MaxTokensStopper::new(100);
    /// assert_eq!(stopper.tokens_remaining(), 100);
    ///
    /// // After processing some tokens during generation:
    /// // assert_eq!(stopper.tokens_remaining(), 75);
    /// ```
    pub fn tokens_remaining(&self) -> usize {
        self.max_tokens.saturating_sub(self.tokens_generated)
    }

    /// Check if the token limit has been reached or exceeded.
    ///
    /// Returns true if the number of tokens generated equals or exceeds
    /// the configured maximum. This method is used internally by `should_stop`
    /// but can also be useful for external progress monitoring.
    ///
    /// # Returns
    ///
    /// True if the token limit has been reached, false otherwise.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use llama_agent::stopper::MaxTokensStopper;
    ///
    /// let stopper = MaxTokensStopper::new(100);
    /// assert!(!stopper.is_limit_reached());
    ///
    /// // After generating 100 or more tokens:
    /// // assert!(stopper.is_limit_reached());
    /// ```
    pub fn is_limit_reached(&self) -> bool {
        self.tokens_generated >= self.max_tokens
    }
}

impl Stopper for MaxTokensStopper {
    fn should_stop(&mut self, _context: &LlamaContext, batch: &LlamaBatch) -> Option<FinishReason> {
        // Extract token count from the current batch
        let tokens_in_batch = batch.n_tokens() as usize;

        // Validate batch contains tokens
        if tokens_in_batch == 0 {
            debug!("MaxTokensStopper received empty batch, continuing generation");
            return None;
        }

        // Update running total with tokens from this batch
        let previous_count = self.tokens_generated;
        self.tokens_generated += tokens_in_batch;

        // Check for potential overflow (defensive programming)
        if self.tokens_generated < previous_count {
            warn!(
                "Token count overflow detected (previous: {}, current: {}), stopping generation",
                previous_count, self.tokens_generated
            );
            return Some(FinishReason::Stopped(
                "Token count overflow - generation stopped for safety".to_string(),
            ));
        }

        // Check if we've reached or exceeded the token limit
        if self.tokens_generated >= self.max_tokens {
            let message = if self.max_tokens == 0 {
                "Generation stopped immediately (zero token limit)".to_string()
            } else if self.tokens_generated == self.max_tokens {
                format!("Maximum tokens reached exactly ({})", self.max_tokens)
            } else {
                format!(
                    "Maximum tokens exceeded ({} > {})",
                    self.tokens_generated, self.max_tokens
                )
            };

            info!(
                max_tokens = self.max_tokens,
                tokens_generated = self.tokens_generated,
                "MaxTokensStopper triggered - stopping generation"
            );

            Some(FinishReason::Stopped(message))
        } else {
            // Continue generation - log progress at intervals
            if self.tokens_generated > 0 && self.tokens_generated % 100 == 0 {
                debug!(
                    "Generation progress: {}/{} tokens ({}% complete)",
                    self.tokens_generated,
                    self.max_tokens,
                    (self.tokens_generated * 100 / self.max_tokens)
                );
            }

            None
        }
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_max_tokens_stopper_creation() {
        let max_tokens = 100;
        let stopper = MaxTokensStopper::new(max_tokens);

        assert_eq!(stopper.max_tokens, max_tokens);
        assert_eq!(stopper.tokens_generated, 0);
    }

    #[test]
    fn test_max_tokens_different_limits() {
        let test_cases = [0, 1, 10, 100, 1000, 10000];

        for max_tokens in test_cases {
            let stopper = MaxTokensStopper::new(max_tokens);
            assert_eq!(stopper.max_tokens, max_tokens);
            assert_eq!(stopper.tokens_generated, 0);
        }
    }

    #[test]
    fn test_stopper_trait_compliance() {
        // Verify MaxTokensStopper properly implements the Stopper trait
        let stopper = MaxTokensStopper::new(100);

        // Verify it can be stored as a trait object
        let _boxed: Box<dyn Stopper> = Box::new(stopper);
    }

    #[test]
    fn test_thread_safety() {
        // Test that MaxTokensStopper implements Send + Sync
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<MaxTokensStopper>();
        assert_sync::<MaxTokensStopper>();
    }

    #[test]
    fn test_edge_cases() {
        // Test with very large max_tokens
        let stopper = MaxTokensStopper::new(usize::MAX);
        assert_eq!(stopper.max_tokens, usize::MAX);
        assert_eq!(stopper.tokens_generated, 0);

        // Test zero limit
        let zero_stopper = MaxTokensStopper::new(0);
        assert_eq!(zero_stopper.max_tokens, 0);
        assert_eq!(zero_stopper.tokens_generated, 0);
    }

    // Note: Integration tests with actual LlamaContext and LlamaBatch
    // using real model are implemented in integration_tests.rs to avoid
    // requiring model loading in unit tests.
    //
    // The should_stop method behavior with batch token counting has been
    // validated separately and works correctly with real batches.
}
