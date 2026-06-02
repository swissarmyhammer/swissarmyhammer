//! # Generation Stoppers Module
//!
//! This module provides a flexible system for controlling when text generation should stop,
//! implementing various stopping conditions that can be applied during model inference.
//!
//! ## Overview
//!
//! The stopping system is designed to provide precise control over generation termination
//! while maintaining high performance (< 5% throughput impact as specified). It supports
//! multiple stopping criteria that can work independently or in combination:
//!
//! - **End-of-Sequence (EOS) Detection**: Stops when the model generates an EOS token
//! - **Maximum Token Limiting**: Stops after generating a specified number of tokens
//!
//! ## Architecture
//!
//! The system is built around the [`Stopper`] trait, which provides a uniform interface
//! for all stopping conditions. Each stopper implementation evaluates the current generation
//! state and returns a [`FinishReason`] when stopping conditions are met.
//!
//! ```rust
//! use llama_agent::stopper::*;
//! use llama_agent::types::StoppingConfig;
//!
//! // Create a stopping configuration
//! let config = StoppingConfig {
//!     max_tokens: Some(100),
//!     eos_detection: true,
//! };
//!
//! // Stoppers are created from the configuration during generation
//! let max_tokens_stopper = MaxTokensStopper::new(100);
//! let eos_stopper = EosStopper::new(2); // EOS token ID
//! ```
//!
//! ## Performance Characteristics
//!
//! - **Low Overhead**: Stoppers add < 5% throughput degradation
//! - **Memory Efficient**: Stoppers use minimal memory overhead
//! - **Thread Safe**: All stoppers implement Send for concurrent usage
//! - **Incremental**: Stoppers process only new tokens, not entire sequences
//!
//! ## Integration
//!
//! Stoppers are integrated into the generation pipeline through the queue system,
//! where they're evaluated after each token batch is processed. The first stopper
//! to return a finish reason terminates generation with that reason.
//!
//! ## Error Handling
//!
//! All stoppers are designed to handle errors gracefully without panicking.
//! Invalid configurations are caught during validation, and runtime errors
//! are logged using the tracing system for debugging.

use llama_cpp_2::{context::LlamaContext, llama_batch::LlamaBatch};

use crate::types::FinishReason;

// Stopper implementations
pub mod eos;
pub mod max_tokens;

// Re-export stopper implementations
pub use eos::EosStopper;
pub use max_tokens::MaxTokensStopper;

/// Trait for determining when to stop text generation.
///
/// The `Stopper` trait provides a uniform interface for implementing various stopping
/// conditions during text generation. Each implementation evaluates the current generation
/// state and returns a [`FinishReason`] when its stopping criteria are met.
///
/// ## Design Principles
///
/// - **Performance**: Implementations must be efficient, adding < 5% overhead
/// - **Composability**: Multiple stoppers can work together seamlessly
/// - **Reliability**: No panics - all errors handled gracefully
/// - **Observability**: Debug logging for troubleshooting generation issues
///
/// ## Implementation Requirements
///
/// Implementing types must:
/// 1. Be thread-safe (implement `Send`) for concurrent usage
/// 2. Handle all edge cases without panicking
/// 3. Log significant events using the `tracing` crate
/// 4. Maintain bounded memory usage for long-running generations
///
/// ## Thread Safety
///
/// All stoppers must implement `Send` to support concurrent generation requests.
/// Stoppers maintain mutable state and are not `Sync`, which is expected since
/// each generation request uses its own stopper instances.
///
/// ## Error Handling
///
/// Stoppers should never panic. Invalid states or unexpected conditions should
/// be logged as warnings and handled gracefully, typically by returning `None`
/// to allow generation to continue.
///
/// # Examples
///
/// ```rust
/// use llama_agent::stopper::*;
/// use llama_agent::types::FinishReason;
/// use llama_cpp_2::{context::LlamaContext, llama_batch::LlamaBatch};
///
/// // Create a max tokens stopper
/// let mut stopper = MaxTokensStopper::new(100);
///
/// // In the generation loop (simplified):
/// // let should_stop = stopper.should_stop(&context, &batch);
/// // match should_stop {
/// //     Some(reason) => break, // Stop generation
/// //     None => continue,      // Continue generation
/// // }
/// ```
pub trait Stopper {
    /// Evaluate whether generation should stop based on current state.
    ///
    /// This method is called after each batch of tokens is processed during generation.
    /// Implementations should efficiently evaluate their stopping criteria and return
    /// a [`FinishReason`] if generation should terminate.
    ///
    /// ## Performance Requirements
    ///
    /// This method is called frequently during generation and must be efficient:
    /// - Target execution time: < 1ms per call for most implementations
    /// - Memory allocation should be minimal or avoided
    /// - Complex operations should be incremental, not full re-computation
    ///
    /// ## Error Handling
    ///
    /// Implementations must never panic. Any errors or invalid states should be:
    /// 1. Logged using `tracing::warn!` or `tracing::error!`
    /// 2. Handled gracefully by returning `None`
    /// 3. Optionally reported through metrics if available
    ///
    /// # Arguments
    ///
    /// * `context` - The LLAMA context containing model state and metadata
    /// * `batch` - The current batch being processed, containing token information
    ///
    /// # Returns
    ///
    /// * `Some(FinishReason)` - Generation should stop with the specified reason
    /// * `None` - Generation should continue
    ///
    /// # Examples
    ///
    /// ```rust
    /// use llama_agent::stopper::*;
    /// use llama_agent::types::FinishReason;
    ///
    /// let mut stopper = MaxTokensStopper::new(10);
    ///
    /// // This would be called by the generation system
    /// // let result = stopper.should_stop(&context, &batch);
    /// // match result {
    /// //     Some(FinishReason::Stopped(msg)) => println!("Stopped: {}", msg),
    /// //     None => println!("Continue generation"),
    /// // }
    /// ```
    fn should_stop(&mut self, context: &LlamaContext, batch: &LlamaBatch) -> Option<FinishReason>;

    /// Downcast to `Any` for specialized handling and configuration access.
    ///
    /// This method enables type-specific access to stopper implementations,
    /// allowing the generation system to perform specialized operations such as:
    ///
    /// - Configuring stopper-specific parameters
    /// - Accessing internal state for debugging
    /// - Performing type-specific optimizations
    ///
    /// ## Usage
    ///
    /// This method is primarily used internally by the generation system.
    /// Most users should interact with stoppers through the [`Stopper`] trait methods.
    ///
    /// # Returns
    ///
    /// A mutable reference to the stopper as `Any`, enabling downcasting
    /// to the concrete type.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use llama_agent::stopper::*;
    /// use std::any::Any;
    ///
    /// let mut stopper: Box<dyn Stopper> = Box::new(MaxTokensStopper::new(100));
    ///
    /// // Downcast to specific type for specialized access
    /// if let Some(max_tokens_stopper) = stopper.as_any_mut().downcast_mut::<MaxTokensStopper>() {
    ///     // Access MaxTokensStopper-specific methods or state
    /// }
    /// ```
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stopper::{EosStopper, MaxTokensStopper};

    /// Select the winning finish reason from an ordered list of per-stopper
    /// decisions, mirroring the production dispatch.
    ///
    /// The live generation loop (`generation/generator.rs`) iterates its
    /// `Vec<Box<dyn Stopper>>` in order and breaks on the first stopper that
    /// returns `Some(FinishReason)`. This helper pins that precedence rule —
    /// "first to fire wins" — over the same `Option<FinishReason>` values the
    /// stoppers produce, without requiring a `LlamaContext` (which cannot be
    /// constructed without loading a model).
    ///
    /// # Arguments
    ///
    /// * `decisions` - Per-stopper decisions in dispatch order.
    ///
    /// # Returns
    ///
    /// The first `Some(FinishReason)` in order, or `None` if every stopper
    /// declined to stop.
    fn first_to_fire<I>(decisions: I) -> Option<FinishReason>
    where
        I: IntoIterator<Item = Option<FinishReason>>,
    {
        decisions.into_iter().flatten().next()
    }

    #[test]
    fn no_stopper_fires_continues_generation() {
        // When every stopper declines, dispatch yields no finish reason.
        let decisions = [None, None];
        assert!(first_to_fire(decisions).is_none());
    }

    #[test]
    fn first_firing_stopper_wins() {
        // The earlier stopper's reason is selected even though a later stopper
        // would also have fired.
        let decisions = [
            Some(FinishReason::Stopped("first".to_string())),
            Some(FinishReason::Stopped("second".to_string())),
        ];
        assert_eq!(
            first_to_fire(decisions),
            Some(FinishReason::Stopped("first".to_string()))
        );
    }

    #[test]
    fn later_stopper_wins_when_earlier_declines() {
        // A leading non-firing stopper does not mask a later one.
        let decisions = [None, Some(FinishReason::Stopped("second".to_string()))];
        assert_eq!(
            first_to_fire(decisions),
            Some(FinishReason::Stopped("second".to_string()))
        );
    }

    #[test]
    fn precedence_uses_real_stopper_cores() {
        // Reproduce the production ordering: MaxTokensStopper precedes
        // EosStopper. Drive both stoppers' model-independent cores (the exact
        // values their `should_stop` returns) and confirm the max-tokens reason
        // wins when it fires while EOS — which never fires on its own — declines.
        let mut max_tokens = MaxTokensStopper::new(3);
        let eos = EosStopper::new(2);

        // First decode step: 2 tokens, neither stopper fires yet.
        let max_decision = max_tokens_record(&mut max_tokens, 2);
        assert!(first_to_fire([max_decision, eos.eval()]).is_none());

        // Second decode step: 2 more tokens reaches the limit (total 4 > 3);
        // the max-tokens reason must be the selected outcome.
        let max_decision = max_tokens_record(&mut max_tokens, 2);
        let winner = first_to_fire([max_decision, eos.eval()]);
        assert_eq!(
            winner,
            Some(FinishReason::Stopped(
                "Maximum tokens exceeded (4 > 3)".to_string()
            ))
        );
    }

    /// Test shim: invoke `MaxTokensStopper`'s model-independent decision core.
    fn max_tokens_record(stopper: &mut MaxTokensStopper, tokens: usize) -> Option<FinishReason> {
        stopper.record_tokens_for_test(tokens)
    }

    /// Test extension exposing the EOS stopper's model-independent decision.
    trait EosEvalForTest {
        fn eval(&self) -> Option<FinishReason>;
    }

    impl EosEvalForTest for EosStopper {
        fn eval(&self) -> Option<FinishReason> {
            self.evaluate_for_test()
        }
    }
}
