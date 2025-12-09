//! Text generation request and response types.
//!
//! This module contains types for configuring text generation requests
//! and handling responses from language models.

// serde traits will be used when Serialize/Deserialize are needed for GenerationRequest
use std::time::Duration;

use crate::types::ids::SessionId;

/// Configuration for controlling when text generation should stop.
#[derive(Debug, Clone)]
pub struct StoppingConfig {
    pub max_tokens: Option<usize>,
    pub eos_detection: bool,
}

impl Default for StoppingConfig {
    fn default() -> Self {
        Self {
            max_tokens: None,
            eos_detection: true,
        }
    }
}

impl StoppingConfig {
    /// Validate the stopping configuration for reasonable limits
    pub fn validate(&self) -> Result<(), String> {
        // Validate max_tokens
        if let Some(max_tokens) = self.max_tokens {
            if max_tokens == 0 {
                return Err("max_tokens must be greater than 0".to_string());
            }
            if max_tokens > 100_000 {
                return Err("max_tokens cannot exceed 100,000 for safety".to_string());
            }
        }

        Ok(())
    }

    /// Create a validated StoppingConfig
    pub fn new_validated(max_tokens: Option<usize>, eos_detection: bool) -> Result<Self, String> {
        let config = Self {
            max_tokens,
            eos_detection,
        };
        config.validate()?;
        Ok(config)
    }
}

/// Request for text generation from a language model.
#[derive(Debug, Clone)]
pub struct GenerationRequest {
    pub session_id: SessionId,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub stop_tokens: Vec<String>,
    pub stopping_config: Option<StoppingConfig>,
}

impl GenerationRequest {
    /// Create a new GenerationRequest with default stopping configuration
    pub fn new(session_id: SessionId) -> Self {
        Self {
            session_id,
            max_tokens: None,
            temperature: None,
            top_p: None,
            stop_tokens: Vec::new(),
            stopping_config: None,
        }
    }

    /// Create a GenerationRequest with default stopping config if none is provided
    pub fn with_default_stopping(mut self) -> Self {
        if self.stopping_config.is_none() {
            self.stopping_config = Some(StoppingConfig::default());
        }
        self
    }

    /// Create a GenerationRequest with custom stopping configuration
    pub fn with_stopping_config(mut self, config: StoppingConfig) -> Self {
        self.stopping_config = Some(config);
        self
    }

    /// Create a GenerationRequest with validated stopping configuration
    pub fn with_validated_stopping_config(
        mut self,
        config: StoppingConfig,
    ) -> Result<Self, String> {
        config.validate()?;
        self.stopping_config = Some(config);
        Ok(self)
    }

    /// Set max_tokens using builder pattern
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Set temperature using builder pattern
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Set top_p using builder pattern
    pub fn with_top_p(mut self, top_p: f32) -> Self {
        self.top_p = Some(top_p);
        self
    }

    /// Set stop_tokens using builder pattern
    pub fn with_stop_tokens(mut self, stop_tokens: Vec<String>) -> Self {
        self.stop_tokens = stop_tokens;
        self
    }

    /// Get the effective max_tokens considering both the direct field and stopping_config
    pub fn effective_max_tokens(&self) -> Option<u32> {
        // Priority: direct max_tokens field, then stopping_config max_tokens, then None
        self.max_tokens.or_else(|| {
            self.stopping_config
                .as_ref()
                .and_then(|config| config.max_tokens.map(|val| val as u32))
        })
    }

    /// Migrate max_tokens to stopping_config for consistency
    pub fn migrate_max_tokens_to_stopping_config(mut self) -> Self {
        if let Some(max_tokens) = self.max_tokens {
            let max_tokens_usize = max_tokens as usize;

            match &mut self.stopping_config {
                Some(config) => {
                    // If stopping_config exists but no max_tokens is set, use the direct field
                    if config.max_tokens.is_none() {
                        config.max_tokens = Some(max_tokens_usize);
                    }
                    // Clear the direct field since we've moved it to stopping_config
                    self.max_tokens = None;
                }
                None => {
                    // Create new stopping config with the max_tokens
                    self.stopping_config = Some(StoppingConfig {
                        max_tokens: Some(max_tokens_usize),
                        ..StoppingConfig::default()
                    });
                    self.max_tokens = None;
                }
            }
        }
        self
    }
}

/// Response from text generation operation.
#[derive(Debug)]
pub struct GenerationResponse {
    pub generated_text: String,
    pub tokens_generated: u32,
    pub generation_time: Duration,
    pub finish_reason: FinishReason,
    /// Complete token sequence including prompt + generated tokens.
    /// Used for session KV cache persistence to avoid reprocessing conversation history.
    pub complete_token_sequence: Option<Vec<i32>>,
}

/// Reason why text generation stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinishReason {
    Stopped(String),
}
