//! Generation configuration types that match existing parameter usage patterns.

use serde::{Deserialize, Serialize};

/// Configuration for text generation operations.
///
/// This struct consolidates the generation parameters used across the codebase,
/// maintaining compatibility with existing parameter validation and usage patterns.
/// The parameters match those found in GenerationRequest and the manual generation
/// implementations in queue.rs and agent.rs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenerationConfig {
    /// Maximum number of tokens to generate.
    pub max_tokens: u32,

    /// Temperature for sampling randomness (0.0 = deterministic, higher = more random).
    pub temperature: f32,

    /// Top-p (nucleus) sampling threshold.
    pub top_p: f32,

    /// Stop tokens that will terminate generation early.
    pub stop_tokens: Vec<String>,

    /// Seed for deterministic generation (used in sampling chain).
    /// Defaults to 1234 to match existing compaction patterns.
    pub seed: u32,

    /// Whether to use greedy sampling (used in agent compaction).
    /// When true, creates sampler chain with greedy() after dist().
    pub use_greedy: bool,
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            max_tokens: 4096, // Matches model context size
            temperature: 0.7, // Reasonable default
            top_p: 0.9,       // Reasonable default
            stop_tokens: Vec::new(),
            seed: 1234,       // Matches existing fixed seed in compaction
            use_greedy: true, // Matches existing compaction behavior
        }
    }
}

impl GenerationConfig {
    /// Create a new configuration for batch generation.
    ///
    /// Uses parameters typical for general text generation with configurable
    /// sampling behavior rather than the fixed greedy approach used in compaction.
    pub fn for_batch_generation() -> Self {
        Self {
            max_tokens: 4096, // Matches model context size
            temperature: 0.7,
            top_p: 0.9,
            stop_tokens: Vec::new(),
            seed: 1234,
            use_greedy: false, // Allow more flexible sampling for general use
        }
    }

    /// Create a new configuration for streaming generation.
    ///
    /// Optimized for streaming with reasonable token limits and responsive sampling.
    pub fn for_streaming() -> Self {
        Self {
            max_tokens: 4096, // Matches model context size
            temperature: 0.7,
            top_p: 0.9,
            stop_tokens: Vec::new(),
            seed: 1234,
            use_greedy: false,
        }
    }

    /// Create a new configuration for compaction operations.
    ///
    /// Matches the existing compaction patterns in agent.rs with deterministic
    /// greedy sampling and reasonable token limits for summaries.
    pub fn for_compaction() -> Self {
        Self {
            max_tokens: 512,  // Matches existing "Reasonable limit for summaries"
            temperature: 0.0, // Deterministic for consistent compaction
            top_p: 1.0,       // Not used with greedy sampling
            stop_tokens: Vec::new(),
            seed: 1234,       // Matches existing "fixed seed for deterministic behavior"
            use_greedy: true, // Matches existing compaction behavior
        }
    }

    /// Validate the configuration parameters.
    ///
    /// Provides the same validation logic currently implemented in the parameter
    /// validator, ensuring early error detection with clear error messages.
    pub fn validate(&self) -> Result<(), String> {
        if self.max_tokens == 0 {
            return Err("max_tokens must be greater than 0".to_string());
        }

        if self.max_tokens > 100_000 {
            return Err("max_tokens cannot exceed 100,000".to_string());
        }

        if !(0.0..=2.0).contains(&self.temperature) {
            return Err("temperature must be between 0.0 and 2.0".to_string());
        }

        if !(0.0..=1.0).contains(&self.top_p) {
            return Err("top_p must be between 0.0 and 1.0".to_string());
        }

        if self.stop_tokens.len() > 10 {
            return Err("Cannot specify more than 10 stop tokens".to_string());
        }

        for token in &self.stop_tokens {
            if token.is_empty() {
                return Err("Stop tokens cannot be empty".to_string());
            }
            if token.len() > 50 {
                return Err("Stop tokens cannot exceed 50 characters".to_string());
            }
        }

        Ok(())
    }
}
