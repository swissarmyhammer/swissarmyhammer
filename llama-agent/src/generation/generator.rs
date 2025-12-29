//! LlamaCppGenerator implementation that consolidates generation logic.

use super::{GenerationConfig, GenerationError, TextGenerator};

use crate::stopper::{EosStopper, MaxTokensStopper, Stopper};
use crate::types::{FinishReason, GenerationRequest, GenerationResponse, StreamChunk};
// Note: Not using async_trait due to Send requirements with LlamaContext raw pointers
use llama_cpp_2::{
    context::LlamaContext,
    llama_batch::LlamaBatch,
    model::{AddBos, LlamaModel, Special},
    sampling::LlamaSampler,
    token::LlamaToken,
};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::STRING_CAPACITY_MULTIPLIER;
use tracing::{debug, error, trace, warn};
use ulid::Ulid;

/// Concrete implementation of TextGenerator using llama-cpp-2.
///
/// This implementation consolidates the identical generation patterns from:
/// - queue.rs:605-676 (batch generation)
/// - queue.rs:881-979 (streaming generation)  
/// - agent.rs:1065-1068 (compaction)
/// - agent.rs:1305-1308 (auto-compaction)
///
/// The implementation maintains the same low-level llama-cpp-2 usage patterns
/// that are already optimal for the library's design, while eliminating code duplication.
pub struct LlamaCppGenerator<'a> {
    model: Arc<LlamaModel>,
    context: Arc<Mutex<LlamaContext<'a>>>,
    batch_size: usize,
}

impl<'a> LlamaCppGenerator<'a> {
    /// Create a new LlamaCppGenerator.
    ///
    /// # Arguments
    /// * `model` - The loaded LlamaModel instance
    /// * `context` - The LlamaContext for processing
    /// * `batch_size` - Maximum batch size for token processing
    pub fn new(
        model: Arc<LlamaModel>,
        context: Arc<Mutex<LlamaContext<'a>>>,
        batch_size: usize,
    ) -> Self {
        Self {
            model,
            context,
            batch_size,
        }
    }

    /// Create a sampler chain based on the configuration.
    ///
    /// This matches the existing sampling patterns:
    /// - For compaction: LlamaSampler::chain_simple([dist(seed), greedy()])
    /// - For general use: More flexible sampling based on temperature/top_p
    fn create_sampler(&self, config: &GenerationConfig) -> LlamaSampler {
        if config.use_greedy {
            // Match existing compaction pattern
            LlamaSampler::chain_simple([LlamaSampler::dist(config.seed), LlamaSampler::greedy()])
        } else {
            // Create more flexible sampling chain for general use
            let mut samplers = vec![LlamaSampler::dist(config.seed)];

            if config.temperature > 0.0 {
                samplers.push(LlamaSampler::temp(config.temperature));
            }

            if config.top_p < 1.0 {
                samplers.push(LlamaSampler::top_p(config.top_p, 1));
            }

            if samplers.len() == 1 {
                // If only distribution sampler, add greedy as fallback
                samplers.push(LlamaSampler::greedy());
            }

            LlamaSampler::chain_simple(samplers)
        }
    }

    /// Create stoppers based on the configuration.
    ///
    /// This matches the existing stopper patterns used in queue.rs.
    fn create_stoppers(&self, config: &GenerationConfig) -> Vec<Box<dyn Stopper + Send>> {
        vec![
            Box::new(MaxTokensStopper::new(config.max_tokens as usize)),
            Box::new(EosStopper::new(2)), // Common EOS token ID, actual detection via model.is_eog_token()
        ]
    }

    /// Process the prompt into tokens with batch chunking.
    ///
    /// This consolidates the tokenization and batch processing logic that's
    /// identical across all generation implementations.
    fn process_prompt(
        &self,
        prompt: &str,
        batch: &mut LlamaBatch,
        cancellation_token: &CancellationToken,
    ) -> Result<usize, GenerationError> {
        // Tokenize the prompt (matches queue.rs:524)
        let tokens_list = self
            .model
            .str_to_token(prompt, AddBos::Always)
            .map_err(GenerationError::tokenization)?;

        debug!("Tokenized prompt to {} tokens", tokens_list.len());

        // Process tokens in chunks to fit batch size (matches queue.rs:540-581)
        if tokens_list.len() > self.batch_size {
            debug!(
                "Prompt token count ({}) exceeds batch size ({}). Processing in chunks.",
                tokens_list.len(),
                self.batch_size
            );
        }

        let mut absolute_position = 0;
        for chunk in tokens_list.chunks(self.batch_size) {
            // Check for cancellation before each batch
            if cancellation_token.is_cancelled() {
                return Err(GenerationError::Cancelled);
            }

            batch.clear();

            // Add tokens from this chunk to batch with correct absolute positions
            for (i, token) in chunk.iter().enumerate() {
                let current_pos = absolute_position + i;
                let is_last_in_entire_sequence = current_pos == tokens_list.len() - 1;

                if let Err(e) =
                    batch.add(*token, current_pos as i32, &[0], is_last_in_entire_sequence)
                {
                    return Err(GenerationError::batch(e));
                }
            }

            // Decode this batch
            let mut context = self
                .context
                .lock()
                .map_err(|_| GenerationError::ContextLock)?;
            if let Err(e) = context.decode(batch) {
                return Err(GenerationError::decoding(e));
            }

            absolute_position += chunk.len();
        }

        Ok(tokens_list.len())
    }

    /// Check if generation should stop due to stop tokens.
    ///
    /// This matches the stop token detection logic from queue.rs.
    fn should_stop(&self, generated_text: &str, stop_tokens: &[String]) -> bool {
        for stop_token in stop_tokens {
            if generated_text.contains(stop_token) {
                return true;
            }
        }
        false
    }

    /// Find the common prefix between old and new token sequences.
    ///
    /// This method compares token sequences to determine how many tokens
    /// at the beginning are identical, allowing incremental processing
    /// to skip reprocessing tokens that are already in the context.
    ///
    /// Returns (common_prefix_length, tokens_to_process)
    fn diff_tokens(&self, new_tokens: &[i32], old_tokens: &[i32]) -> (usize, Vec<i32>) {
        let common_len = new_tokens
            .iter()
            .zip(old_tokens.iter())
            .take_while(|(a, b)| a == b)
            .count();

        let tokens_to_process = new_tokens[common_len..].to_vec();
        (common_len, tokens_to_process)
    }

    /// Process only new tokens that aren't already in the context.
    ///
    /// This method implements incremental prompt processing by comparing the new prompt
    /// against previously processed tokens and only sending new tokens to the model.
    /// This dramatically reduces processing overhead for continued conversations.
    ///
    /// Returns the total number of tokens in the full prompt (both cached and new).
    fn process_prompt_incremental(
        &self,
        prompt: &str,
        batch: &mut LlamaBatch,
        context_state: Option<&mut crate::types::sessions::ContextState>,
        cancellation_token: &CancellationToken,
    ) -> Result<usize, GenerationError> {
        // Tokenize the new prompt
        let new_tokens: Vec<i32> = self
            .model
            .str_to_token(prompt, AddBos::Always)
            .map_err(GenerationError::tokenization)?
            .into_iter()
            .map(|token| token.0)
            .collect();

        debug!("Tokenized prompt to {} tokens", new_tokens.len());

        // Determine what tokens need processing
        let (common_prefix_len, tokens_to_process) = if let Some(state) = context_state.as_ref() {
            if state.matches_prompt(prompt) {
                // Exact match - no processing needed
                debug!("Prompt unchanged, skipping processing");
                return Ok(new_tokens.len());
            }

            // Find common prefix with previously processed tokens
            let (prefix_len, new_tokens_only) =
                self.diff_tokens(&new_tokens, &state.processed_tokens);
            debug!(
                "Found {} common tokens, {} new tokens to process",
                prefix_len,
                new_tokens_only.len()
            );
            (prefix_len, new_tokens_only)
        } else {
            // No context state - process all tokens
            debug!("No context state, processing all tokens");
            (0, new_tokens.clone())
        };

        // If no new tokens to process, we're done
        if tokens_to_process.is_empty() {
            debug!("No new tokens to process");
            return Ok(new_tokens.len());
        }

        // Process the new tokens starting from the correct position
        self.process_new_tokens(
            tokens_to_process.clone(),
            common_prefix_len,
            batch,
            cancellation_token,
        )?;

        // Update context state with the new token sequence
        if let Some(state) = context_state {
            state.update(new_tokens.clone(), prompt);
            debug!(
                "Updated context state: {} total tokens, position {}",
                state.processed_tokens.len(),
                state.current_position
            );
        }

        Ok(new_tokens.len())
    }

    /// Process new tokens starting from the given position.
    ///
    /// This method handles the actual token processing, similar to the original
    /// process_prompt method but starting from a specific position in the sequence.
    fn process_new_tokens(
        &self,
        tokens: Vec<i32>,
        start_position: usize,
        batch: &mut LlamaBatch,
        cancellation_token: &CancellationToken,
    ) -> Result<(), GenerationError> {
        if tokens.is_empty() {
            return Ok(());
        }

        debug!(
            "Processing {} new tokens starting from position {}",
            tokens.len(),
            start_position
        );

        // Convert i32 tokens back to LlamaToken
        let tokens_list: Vec<LlamaToken> = tokens.into_iter().map(LlamaToken).collect();

        // Process tokens in chunks to fit batch size (similar to original process_prompt)
        if tokens_list.len() > self.batch_size {
            debug!(
                "New token count ({}) exceeds batch size ({}). Processing in chunks.",
                tokens_list.len(),
                self.batch_size
            );
        }

        let mut absolute_position = start_position;
        for chunk in tokens_list.chunks(self.batch_size) {
            // Check for cancellation before each batch
            if cancellation_token.is_cancelled() {
                return Err(GenerationError::Cancelled);
            }

            batch.clear();

            // Add tokens from this chunk to batch with correct absolute positions
            for (i, token) in chunk.iter().enumerate() {
                let current_pos = absolute_position + i;
                let is_last_in_entire_sequence =
                    current_pos == start_position + tokens_list.len() - 1;

                if let Err(e) =
                    batch.add(*token, current_pos as i32, &[0], is_last_in_entire_sequence)
                {
                    return Err(GenerationError::batch(e));
                }
            }

            // Decode this batch
            let mut context = self
                .context
                .lock()
                .map_err(|_| GenerationError::ContextLock)?;
            if let Err(e) = context.decode(batch) {
                return Err(GenerationError::decoding(e));
            }

            absolute_position += chunk.len();
        }

        Ok(())
    }

    /// Process tokens starting from a specific offset position.
    ///
    /// This is used when a template has been pre-loaded into the KV cache,
    /// and new tokens need to be processed starting after the template.
    ///
    /// # Arguments
    ///
    /// * `tokens` - The tokens to process (as i32 values)
    /// * `start_position` - The position offset to start from (e.g., template token count)
    /// * `batch` - The batch to use for processing
    /// * `cancellation_token` - Token for cancellation
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Template is at positions 0..100
    /// // Process new message tokens starting at position 100
    /// generator.process_tokens_with_offset(message_tokens, 100, &mut batch, &token)?;
    /// ```
    pub fn process_tokens_with_offset(
        &self,
        tokens: Vec<i32>,
        start_position: usize,
        batch: &mut LlamaBatch,
        cancellation_token: &CancellationToken,
    ) -> Result<(), GenerationError> {
        // This is a public wrapper around process_new_tokens that explicitly
        // documents the template offset use case
        self.process_new_tokens(tokens, start_position, batch, cancellation_token)
    }

    /// Process prompt with optional template offset.
    ///
    /// If template_token_count is provided, the prompt will be tokenized and only
    /// tokens after position template_token_count will be processed. This assumes
    /// the template is already in the KV cache at positions 0..template_token_count.
    ///
    /// # Arguments
    ///
    /// * `prompt` - The full prompt including template and messages
    /// * `batch` - The batch to use for processing
    /// * `template_token_count` - Optional number of template tokens already in KV cache
    /// * `cancellation_token` - Token for cancellation
    ///
    /// # Returns
    ///
    /// Returns the total number of tokens in the prompt (both template and message tokens)
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Template is already in KV cache (100 tokens)
    /// // Full prompt = template + messages (150 tokens total)
    /// // Only processes tokens 100..150
    /// let total_tokens = generator.process_prompt_with_template_offset(
    ///     full_prompt,
    ///     &mut batch,
    ///     Some(100),
    ///     &token
    /// )?;
    /// assert_eq!(total_tokens, 150);
    /// ```
    pub fn process_prompt_with_template_offset(
        &self,
        prompt: &str,
        batch: &mut LlamaBatch,
        template_token_count: Option<usize>,
        cancellation_token: &CancellationToken,
    ) -> Result<usize, GenerationError> {
        // Tokenize the full prompt
        let tokens_list = self
            .model
            .str_to_token(prompt, AddBos::Always)
            .map_err(GenerationError::tokenization)?;

        let total_token_count = tokens_list.len();
        let start_position = template_token_count.unwrap_or(0);

        debug!(
            "Tokenized prompt to {} tokens, template offset: {}",
            total_token_count, start_position
        );

        // If no offset, process all tokens normally
        if start_position == 0 {
            return self.process_prompt(prompt, batch, cancellation_token);
        }

        // Validate that offset doesn't exceed token count
        if start_position >= total_token_count {
            debug!(
                "Template offset ({}) >= total tokens ({}), no new tokens to process",
                start_position, total_token_count
            );
            return Ok(total_token_count);
        }

        // Skip template tokens, only process message tokens
        let tokens_to_process: Vec<i32> = tokens_list
            .iter()
            .skip(start_position)
            .map(|t| t.0)
            .collect();

        debug!(
            "Skipping {} template tokens, processing {} message tokens starting at position {}",
            start_position,
            tokens_to_process.len(),
            start_position
        );

        // Process the message tokens starting from the offset position
        self.process_new_tokens(tokens_to_process, start_position, batch, cancellation_token)?;

        Ok(total_token_count)
    }
}

impl<'a> TextGenerator for LlamaCppGenerator<'a> {
    /// Generate text synchronously and return the complete response.
    ///
    /// This method consolidates the batch generation logic from queue.rs:605-676
    /// and the compaction patterns from agent.rs:1065-1068 and agent.rs:1305-1308.
    fn generate_text(
        &mut self,
        prompt: &str,
        request: GenerationRequest,
        cancellation_token: CancellationToken,
    ) -> Result<GenerationResponse, GenerationError> {
        self.generate_text_with_context(prompt, request, cancellation_token, None)
    }

    /// Generate text with optional context state for incremental processing.
    ///
    /// When context_state is provided, this method will use incremental processing
    /// to avoid reprocessing tokens that are already in the context, significantly
    /// improving performance for continued conversations.
    fn generate_text_with_context(
        &mut self,
        prompt: &str,
        request: GenerationRequest,
        cancellation_token: CancellationToken,
        context_state: Option<&mut crate::types::sessions::ContextState>,
    ) -> Result<GenerationResponse, GenerationError> {
        let start_time = Instant::now();
        let _request_id = Ulid::new();

        // Extract configuration from request using appropriate config for batch generation
        let mut config = GenerationConfig::for_batch_generation();

        // Override with request-specific values
        if let Some(max_tokens) = request.max_tokens {
            config.max_tokens = max_tokens;
        }
        if let Some(temperature) = request.temperature {
            config.temperature = temperature;
        }
        if let Some(top_p) = request.top_p {
            config.top_p = top_p;
        }
        config.stop_tokens = request.stop_tokens.clone();

        // Validate configuration
        config.validate()?;

        // Tokenize prompt for token sequence tracking
        let prompt_tokens: Vec<i32> = self
            .model
            .str_to_token(prompt, AddBos::Always)
            .map_err(GenerationError::tokenization)?
            .into_iter()
            .map(|t| t.0)
            .collect();

        // Create batch and process prompt (using incremental processing if context state available)
        let mut batch = LlamaBatch::new(self.batch_size, 1);
        let n_tokens = if context_state.is_some() {
            self.process_prompt_incremental(prompt, &mut batch, context_state, &cancellation_token)?
        } else {
            self.process_prompt(prompt, &mut batch, &cancellation_token)?
        };

        // Create sampler and stoppers
        let mut sampler = self.create_sampler(&config);
        let mut stoppers = self.create_stoppers(&config);

        // Generation state
        let mut generated_text = String::new();
        let mut finish_reason = FinishReason::Stopped("Maximum tokens reached".to_string());
        let mut tokens_generated = 0u32;
        let mut n_cur = n_tokens;
        let mut generated_token_ids = Vec::new(); // Track generated tokens for KV cache

        // Main generation loop (matches queue.rs:605-676)
        while tokens_generated < config.max_tokens {
            // Check for cancellation before each token
            if cancellation_token.is_cancelled() {
                finish_reason = FinishReason::Stopped("Error: Request cancelled".to_string());
                break;
            }

            // Sample next token
            let context = self
                .context
                .lock()
                .map_err(|_| GenerationError::ContextLock)?;
            let token = sampler.sample(&context, batch.n_tokens() - 1);
            drop(context); // Release the lock early

            // Track the generated token ID for KV cache
            generated_token_ids.push(token.0);

            // Check for end of sequence token
            if self.model.is_eog_token(token) {
                finish_reason = FinishReason::Stopped("End of sequence token detected".to_string());
                break;
            }

            // Convert token to string with buffer reuse
            let token_str = match self.model.token_to_str(token, Special::Tokenize) {
                Ok(s) => s,
                Err(e) => {
                    trace!("Failed to convert token to string: {}", e);
                    // Continue generation - this may happen with some tokens and shouldn't be fatal
                    continue;
                }
            };

            // Efficient string concatenation (matches queue.rs pattern)
            if generated_text.capacity() - generated_text.len() < token_str.len() {
                generated_text.reserve(token_str.len() * STRING_CAPACITY_MULTIPLIER);
            }
            generated_text.push_str(&token_str);
            tokens_generated += 1;

            // Check stoppers for early termination
            {
                let context = self
                    .context
                    .lock()
                    .map_err(|_| GenerationError::ContextLock)?;
                for stopper in &mut stoppers {
                    if let Some(FinishReason::Stopped(reason)) =
                        stopper.should_stop(&context, &batch)
                    {
                        finish_reason = FinishReason::Stopped(reason);
                        break;
                    }
                }
            }

            // If a stopper triggered, break out of the generation loop
            if !matches!(finish_reason, FinishReason::Stopped(ref r) if r == "Maximum tokens reached")
            {
                break;
            }

            // Check for stop tokens in the generated text
            if self.should_stop(&generated_text, &config.stop_tokens) {
                finish_reason = FinishReason::Stopped("Stop token detected".to_string());
                break;
            }

            // Prepare next batch for continued generation
            batch.clear();
            if let Err(e) = batch.add(token, n_cur as i32, &[0], true) {
                error!("Failed to add continuation token: {}", e);
                finish_reason = FinishReason::Stopped(
                    "Error: Failed to prepare continuation batch".to_string(),
                );
                break;
            }

            // Decode the new token
            {
                let mut context = self
                    .context
                    .lock()
                    .map_err(|_| GenerationError::ContextLock)?;
                if let Err(e) = context.decode(&mut batch) {
                    error!("Failed to decode continuation batch: {}", e);
                    finish_reason = FinishReason::Stopped(
                        "Error: Failed to decode continuation batch".to_string(),
                    );
                    break;
                }
            }

            n_cur += 1;
        }

        let generation_time = start_time.elapsed();

        // Build complete token sequence: prompt + generated tokens
        let prompt_token_count = prompt_tokens.len();
        let mut complete_tokens = prompt_tokens;
        complete_tokens.extend(generated_token_ids);

        debug!(
            "Complete token sequence: {} tokens ({} prompt + {} generated)",
            complete_tokens.len(),
            prompt_token_count,
            tokens_generated
        );

        Ok(GenerationResponse {
            generated_text,
            tokens_generated,
            generation_time,
            finish_reason,
            complete_token_sequence: Some(complete_tokens),
        })
    }

    /// Generate text with streaming output.
    ///
    /// This method consolidates the streaming generation logic from queue.rs:881-979,
    /// sending individual tokens as they're generated through the provided channel.
    fn generate_stream(
        &mut self,
        prompt: &str,
        request: GenerationRequest,
        stream_sender: mpsc::UnboundedSender<Result<StreamChunk, crate::types::QueueError>>,
        cancellation_token: CancellationToken,
    ) -> Result<(), GenerationError> {
        self.generate_stream_with_context(prompt, request, stream_sender, cancellation_token, None)
    }

    /// Generate text with streaming output and optional context state for incremental processing.
    ///
    /// When context_state is provided, this method will use incremental processing
    /// to avoid reprocessing tokens that are already in the context, significantly
    /// improving performance for continued conversations in streaming scenarios.
    fn generate_stream_with_context(
        &mut self,
        prompt: &str,
        request: GenerationRequest,
        stream_sender: mpsc::UnboundedSender<Result<StreamChunk, crate::types::QueueError>>,
        cancellation_token: CancellationToken,
        context_state: Option<&mut crate::types::sessions::ContextState>,
    ) -> Result<(), GenerationError> {
        let start_time = Instant::now();

        // Extract configuration from request using appropriate config for streaming
        let mut config = GenerationConfig::for_streaming();

        // Override with request-specific values
        if let Some(max_tokens) = request.max_tokens {
            config.max_tokens = max_tokens;
        }
        if let Some(temperature) = request.temperature {
            config.temperature = temperature;
        }
        if let Some(top_p) = request.top_p {
            config.top_p = top_p;
        }
        config.stop_tokens = request.stop_tokens.clone();

        // Validate configuration
        config.validate()?;

        // Create batch and process prompt (using incremental processing if context state available)
        let mut batch = LlamaBatch::new(self.batch_size, 1);
        let n_tokens = if context_state.is_some() {
            self.process_prompt_incremental(prompt, &mut batch, context_state, &cancellation_token)?
        } else {
            self.process_prompt(prompt, &mut batch, &cancellation_token)?
        };

        // Create sampler and stoppers
        let mut sampler = self.create_sampler(&config);
        let mut stoppers = self.create_stoppers(&config);

        // Generation state
        let mut generated_text = String::new();
        let mut tokens_generated = 0u32;
        let mut n_cur = n_tokens;

        // Main streaming generation loop (matches queue.rs:881-979)
        while tokens_generated < config.max_tokens {
            // Check for cancellation before each token
            if cancellation_token.is_cancelled() {
                debug!("Streaming request cancelled during token generation");
                let _ = stream_sender.send(Err(crate::types::QueueError::WorkerError(
                    "Request cancelled".to_string(),
                )));
                return Ok(());
            }

            // Sample next token
            let context = self
                .context
                .lock()
                .map_err(|_| GenerationError::ContextLock)?;
            let token = sampler.sample(&context, batch.n_tokens() - 1);
            drop(context); // Release the lock early

            // Check for end of sequence token
            if self.model.is_eog_token(token) {
                return self.handle_streaming_completion(
                    &generated_text,
                    tokens_generated,
                    start_time,
                    &stream_sender,
                    "EndOfSequence",
                );
            }

            // Convert token to string
            let token_text = match self.model.token_to_str(token, Special::Tokenize) {
                Ok(s) => s,
                Err(e) => {
                    trace!("Failed to convert token to string in streaming: {}", e);
                    // Continue generation - this may happen with some tokens and shouldn't be fatal
                    continue;
                }
            };

            generated_text.push_str(&token_text);
            tokens_generated += 1;

            // Send the streaming chunk immediately (matches queue.rs:924-932)
            let chunk = StreamChunk {
                text: token_text.clone(),
                is_complete: false,
                token_count: tokens_generated,
                finish_reason: None,
            };

            if stream_sender.send(Ok(chunk)).is_err() {
                warn!("Stream receiver disconnected, stopping generation");
                return Ok(());
            }

            // Check stoppers for early termination
            {
                let context = self
                    .context
                    .lock()
                    .map_err(|_| GenerationError::ContextLock)?;
                for stopper in &mut stoppers {
                    if let Some(crate::types::FinishReason::Stopped(reason)) =
                        stopper.should_stop(&context, &batch)
                    {
                        return self.handle_streaming_completion(
                            &generated_text,
                            tokens_generated,
                            start_time,
                            &stream_sender,
                            &reason,
                        );
                    }
                }
            }

            // Check for stop tokens in the accumulated generated text
            if self.should_stop(&generated_text, &config.stop_tokens) {
                return self.handle_streaming_completion(
                    &generated_text,
                    tokens_generated,
                    start_time,
                    &stream_sender,
                    "StopToken",
                );
            }

            // Prepare next batch for continued generation
            batch.clear();
            if let Err(e) = batch.add(token, n_cur as i32, &[0], true) {
                error!("Failed to add continuation token for streaming: {}", e);
                let _ = stream_sender.send(Err(crate::types::QueueError::WorkerError(format!(
                    "Failed to prepare continuation batch: {}",
                    e
                ))));
                return Ok(());
            }

            // Decode the new token
            {
                let mut context = self
                    .context
                    .lock()
                    .map_err(|_| GenerationError::ContextLock)?;
                if let Err(e) = context.decode(&mut batch) {
                    error!("Failed to decode continuation batch for streaming: {}", e);
                    let _ = stream_sender.send(Err(crate::types::QueueError::WorkerError(
                        format!("Failed to decode continuation batch: {}", e),
                    )));
                    return Ok(());
                }
            }

            n_cur += 1;
        }

        // If we exit the loop due to max tokens, send final completion chunk
        self.handle_streaming_completion(
            &generated_text,
            tokens_generated,
            start_time,
            &stream_sender,
            "MaxTokens",
        )
    }

    fn generate_text_with_template_offset(
        &mut self,
        prompt: &str,
        request: GenerationRequest,
        cancellation_token: CancellationToken,
        template_token_count: Option<usize>,
    ) -> Result<GenerationResponse, GenerationError> {
        let start_time = Instant::now();
        let _request_id = Ulid::new();

        // Extract configuration from request using appropriate config for batch generation
        let mut config = GenerationConfig::for_batch_generation();

        // Override with request-specific values
        if let Some(max_tokens) = request.max_tokens {
            config.max_tokens = max_tokens;
        }
        if let Some(temperature) = request.temperature {
            config.temperature = temperature;
        }
        if let Some(top_p) = request.top_p {
            config.top_p = top_p;
        }
        config.stop_tokens = request.stop_tokens.clone();

        // Validate configuration
        config.validate()?;

        // Create batch and process prompt with template offset
        let mut batch = LlamaBatch::new(self.batch_size, 1);
        let n_tokens = self.process_prompt_with_template_offset(
            prompt,
            &mut batch,
            template_token_count,
            &cancellation_token,
        )?;

        // Create sampler and stoppers
        let mut sampler = self.create_sampler(&config);
        let mut stoppers = self.create_stoppers(&config);

        // Generation state
        let mut generated_text = String::new();
        let mut finish_reason = FinishReason::Stopped("Maximum tokens reached".to_string());
        let mut tokens_generated = 0u32;
        let mut n_cur = n_tokens;

        // Main generation loop (same as generate_text)
        while tokens_generated < config.max_tokens {
            // Check for cancellation before each token
            if cancellation_token.is_cancelled() {
                finish_reason = FinishReason::Stopped("Error: Request cancelled".to_string());
                break;
            }

            // Sample next token
            let context = self
                .context
                .lock()
                .map_err(|_| GenerationError::ContextLock)?;
            let token = sampler.sample(&context, batch.n_tokens() - 1);
            drop(context); // Release the lock early

            // Check for end of sequence token
            if self.model.is_eog_token(token) {
                finish_reason = FinishReason::Stopped("End of sequence token detected".to_string());
                break;
            }

            // Convert token to string with buffer reuse
            let token_str = match self.model.token_to_str(token, Special::Tokenize) {
                Ok(s) => s,
                Err(e) => {
                    trace!("Failed to convert token to string: {}", e);
                    continue;
                }
            };

            // Efficient string concatenation
            if generated_text.capacity() - generated_text.len() < token_str.len() {
                generated_text.reserve(token_str.len() * STRING_CAPACITY_MULTIPLIER);
            }
            generated_text.push_str(&token_str);
            tokens_generated += 1;

            // Check stoppers for early termination
            {
                let context = self
                    .context
                    .lock()
                    .map_err(|_| GenerationError::ContextLock)?;
                for stopper in &mut stoppers {
                    if let Some(FinishReason::Stopped(reason)) =
                        stopper.should_stop(&context, &batch)
                    {
                        finish_reason = FinishReason::Stopped(reason);
                        break;
                    }
                }
            }

            // If a stopper triggered, break out of the generation loop
            if !matches!(finish_reason, FinishReason::Stopped(ref r) if r == "Maximum tokens reached")
            {
                break;
            }

            // Check for stop tokens in the generated text
            if self.should_stop(&generated_text, &config.stop_tokens) {
                finish_reason = FinishReason::Stopped("Stop token detected".to_string());
                break;
            }

            // Prepare next batch for continued generation
            batch.clear();
            if let Err(e) = batch.add(token, n_cur as i32, &[0], true) {
                error!("Failed to add continuation token: {}", e);
                finish_reason = FinishReason::Stopped(
                    "Error: Failed to prepare continuation batch".to_string(),
                );
                break;
            }

            // Decode the new token
            {
                let mut context = self
                    .context
                    .lock()
                    .map_err(|_| GenerationError::ContextLock)?;
                if let Err(e) = context.decode(&mut batch) {
                    error!("Failed to decode continuation batch: {}", e);
                    finish_reason = FinishReason::Stopped(
                        "Error: Failed to decode continuation batch".to_string(),
                    );
                    break;
                }
            }

            n_cur += 1;
        }

        let generation_time = start_time.elapsed();

        Ok(GenerationResponse {
            generated_text,
            tokens_generated,
            generation_time,
            finish_reason,
            complete_token_sequence: None, // Generator doesn't track tokens for caching
        })
    }

    fn generate_stream_with_template_offset(
        &mut self,
        prompt: &str,
        request: GenerationRequest,
        stream_sender: mpsc::UnboundedSender<Result<StreamChunk, crate::types::QueueError>>,
        cancellation_token: CancellationToken,
        template_token_count: Option<usize>,
    ) -> Result<(), GenerationError> {
        let start_time = Instant::now();

        // Extract configuration from request using appropriate config for streaming
        let mut config = GenerationConfig::for_streaming();

        // Override with request-specific values
        if let Some(max_tokens) = request.max_tokens {
            config.max_tokens = max_tokens;
        }
        if let Some(temperature) = request.temperature {
            config.temperature = temperature;
        }
        if let Some(top_p) = request.top_p {
            config.top_p = top_p;
        }
        config.stop_tokens = request.stop_tokens.clone();

        // Validate configuration
        config.validate()?;

        // Create batch and process prompt with template offset
        let mut batch = LlamaBatch::new(self.batch_size, 1);
        let n_tokens = self.process_prompt_with_template_offset(
            prompt,
            &mut batch,
            template_token_count,
            &cancellation_token,
        )?;

        // Create sampler and stoppers
        let mut sampler = self.create_sampler(&config);
        let mut stoppers = self.create_stoppers(&config);

        // Generation state
        let mut generated_text = String::new();
        let mut tokens_generated = 0u32;
        let mut n_cur = n_tokens;

        // Main streaming generation loop (same as generate_stream_with_context)
        while tokens_generated < config.max_tokens {
            // Check for cancellation before each token
            if cancellation_token.is_cancelled() {
                debug!("Streaming request cancelled during token generation");
                let _ = stream_sender.send(Err(crate::types::QueueError::WorkerError(
                    "Request cancelled".to_string(),
                )));
                return Ok(());
            }

            // Sample next token
            let context = self
                .context
                .lock()
                .map_err(|_| GenerationError::ContextLock)?;
            let token = sampler.sample(&context, batch.n_tokens() - 1);
            drop(context); // Release the lock early

            // Check for end of sequence token
            if self.model.is_eog_token(token) {
                return self.handle_streaming_completion(
                    &generated_text,
                    tokens_generated,
                    start_time,
                    &stream_sender,
                    "EndOfSequence",
                );
            }

            // Convert token to string
            let token_text = match self.model.token_to_str(token, Special::Tokenize) {
                Ok(s) => s,
                Err(e) => {
                    trace!("Failed to convert token to string in streaming: {}", e);
                    continue;
                }
            };

            generated_text.push_str(&token_text);
            tokens_generated += 1;

            // Send the streaming chunk immediately
            let chunk = StreamChunk {
                text: token_text.clone(),
                is_complete: false,
                token_count: tokens_generated,
                finish_reason: None,
            };

            if stream_sender.send(Ok(chunk)).is_err() {
                warn!("Stream receiver disconnected, stopping generation");
                return Ok(());
            }

            // Check stoppers for early termination
            {
                let context = self
                    .context
                    .lock()
                    .map_err(|_| GenerationError::ContextLock)?;
                for stopper in &mut stoppers {
                    if let Some(crate::types::FinishReason::Stopped(reason)) =
                        stopper.should_stop(&context, &batch)
                    {
                        return self.handle_streaming_completion(
                            &generated_text,
                            tokens_generated,
                            start_time,
                            &stream_sender,
                            &reason,
                        );
                    }
                }
            }

            // Check for stop tokens in the accumulated generated text
            if self.should_stop(&generated_text, &config.stop_tokens) {
                return self.handle_streaming_completion(
                    &generated_text,
                    tokens_generated,
                    start_time,
                    &stream_sender,
                    "StopToken",
                );
            }

            // Prepare next batch for continued generation
            batch.clear();
            if let Err(e) = batch.add(token, n_cur as i32, &[0], true) {
                error!("Failed to add continuation token for streaming: {}", e);
                let _ = stream_sender.send(Err(crate::types::QueueError::WorkerError(format!(
                    "Failed to prepare continuation batch: {}",
                    e
                ))));
                return Ok(());
            }

            // Decode the new token
            {
                let mut context = self
                    .context
                    .lock()
                    .map_err(|_| GenerationError::ContextLock)?;
                if let Err(e) = context.decode(&mut batch) {
                    error!("Failed to decode continuation batch for streaming: {}", e);
                    let _ = stream_sender.send(Err(crate::types::QueueError::WorkerError(
                        format!("Failed to decode continuation batch: {}", e),
                    )));
                    return Ok(());
                }
            }

            n_cur += 1;
        }

        // If we exit the loop due to max tokens, send final completion chunk
        self.handle_streaming_completion(
            &generated_text,
            tokens_generated,
            start_time,
            &stream_sender,
            "MaxTokens",
        )
    }
}

impl<'a> LlamaCppGenerator<'a> {
    /// Handle streaming completion by sending the final chunk.
    ///
    /// This consolidates the completion handling logic from queue.rs:handle_streaming_completion
    /// while focusing on the core streaming completion without tool call extraction.
    fn handle_streaming_completion(
        &self,
        _generated_text: &str,
        tokens_generated: u32,
        start_time: Instant,
        stream_sender: &mpsc::UnboundedSender<Result<StreamChunk, crate::types::QueueError>>,
        finish_reason: &str,
    ) -> Result<(), GenerationError> {
        let generation_time = start_time.elapsed();

        debug!(
            "Streaming generation completed: {} tokens in {}ms (reason: {})",
            tokens_generated,
            generation_time.as_millis(),
            finish_reason
        );

        // Send final completion chunk
        let final_chunk = StreamChunk {
            text: String::new(), // Final chunk has empty text
            is_complete: true,
            token_count: tokens_generated,
            finish_reason: Some(FinishReason::Stopped(finish_reason.to_string())),
        };

        if stream_sender.send(Ok(final_chunk)).is_err() {
            warn!("Failed to send final streaming chunk - receiver disconnected");
            return Err(GenerationError::StreamClosed);
        }

        Ok(())
    }
}
