//! A scripted, weight-free [`TextGenerator`] test double.
//!
//! # Why this exists
//!
//! The only part of llama-agent that genuinely needs a real model loaded onto a
//! GPU is the FFI decode step (feed tokens into llama.cpp, read back logits).
//! Everything layered above that — the streaming loop, the generation budget
//! arithmetic, stop-token handling, chunk accounting, end-of-sequence detection
//! and the "0 tokens generated" completion shape — is deterministic logic. That
//! logic should be exercisable in milliseconds, without weights and without a
//! GPU.
//!
//! [`ScriptedModel`] is the seam that makes that possible. It implements the
//! same public [`TextGenerator`] trait that the real [`LlamaCppGenerator`]
//! implements, so the generation paths drive it exactly as they drive the real
//! model. Instead of sampling logits it replays a caller-supplied list of
//! [`ScriptToken`]s and then signals end-of-sequence. The streamed [`StreamChunk`]
//! contract it produces is byte-for-byte identical to the production
//! `GenerationHelper` streaming path:
//!
//! - one chunk per emitted token: `{ text, is_complete: false, token_count: 1,
//!   finish_reason: None }`;
//! - a final completion chunk: `{ text: "", is_complete: true, token_count: 0,
//!   finish_reason: Some(FinishReason::Stopped(reason)) }`;
//! - the same completion reason strings (`"EndOfSequence"`, `"MaxTokens"`,
//!   `"StopToken"`, `"ContextWindowFull"`).
//!
//! This is the keystone the model-dependent coverage cards build on: they can
//! drive `generate_stream` / `generate_text` through a `ScriptedModel`, then
//! assert on the observable behaviour with no real-model download.
//!
//! [`LlamaCppGenerator`]: crate::generation::LlamaCppGenerator

use std::sync::{Arc, Mutex};
use std::time::Instant;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::{GenerationConfig, GenerationError, TextGenerator};
use crate::types::{FinishReason, GenerationRequest, GenerationResponse, QueueError, StreamChunk};

/// A single scripted decode step.
///
/// A real model, at each step, either produces a token that maps to some text or
/// produces an end-of-generation token. `ScriptToken` models exactly that
/// choice, with the token's display text carried inline so the double needs no
/// tokenizer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptToken {
    /// Emit a token that decodes to the given text. Equivalent to a real model
    /// sampling a non-EOG token and converting it to a string.
    Text(String),
    /// Emit an end-of-generation token. Equivalent to a real model sampling a
    /// token for which `is_eog_token` is true. Generation completes with the
    /// `EndOfSequence` reason and no further tokens are produced.
    EndOfSequence,
}

impl ScriptToken {
    /// Build a text token from anything string-like.
    pub fn text(s: impl Into<String>) -> Self {
        ScriptToken::Text(s.into())
    }
}

/// A weight-free [`TextGenerator`] that replays a fixed script of tokens.
///
/// Construct one with [`ScriptedModel::new`] passing the tokens to replay, then
/// drive it through any [`TextGenerator`] method just like the real generator.
/// The model records every prompt it is fed (see [`ScriptedModel::fed_prompts`])
/// so budget and chat-template behaviour can be asserted, and exposes a
/// configurable context size (see [`ScriptedModel::with_context_size`]) so the
/// context-window guard can be exercised.
///
/// Cloning a `ScriptedModel` shares the recorded-prompt log (it is held behind
/// an `Arc<Mutex<_>>`), which keeps assertions simple when a caller needs both a
/// handle to inspect and a value to move into the generator.
#[derive(Clone)]
pub struct ScriptedModel {
    /// The tokens to replay, in order. Replay stops at the first
    /// [`ScriptToken::EndOfSequence`] or when the list is exhausted.
    script: Vec<ScriptToken>,
    /// The simulated context window size, in tokens. Generation stops with the
    /// `ContextWindowFull` reason once the prompt plus generated tokens would
    /// reach this limit, mirroring the production guard.
    context_size: usize,
    /// Every prompt fed to a generation call, in call order. Shared across
    /// clones so the recording survives moving the model into a generator.
    fed_prompts: Arc<Mutex<Vec<String>>>,
}

/// Default simulated context window. Large enough that the context-window guard
/// never fires unless a test deliberately shrinks it with
/// [`ScriptedModel::with_context_size`].
const DEFAULT_SCRIPTED_CONTEXT_SIZE: usize = 4096;

impl ScriptedModel {
    /// Create a scripted model that will replay `script`, then stop.
    ///
    /// If the script does not end in [`ScriptToken::EndOfSequence`], generation
    /// still terminates cleanly once the script is exhausted or the
    /// `max_tokens` budget is reached — whichever comes first.
    pub fn new(script: impl IntoIterator<Item = ScriptToken>) -> Self {
        Self {
            script: script.into_iter().collect(),
            context_size: DEFAULT_SCRIPTED_CONTEXT_SIZE,
            fed_prompts: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Convenience constructor for a script of plain text tokens followed by an
    /// implicit end-of-sequence.
    ///
    /// `ScriptedModel::from_texts(["a", "b"])` is equivalent to
    /// `ScriptedModel::new([ScriptToken::text("a"), ScriptToken::text("b"),
    /// ScriptToken::EndOfSequence])`.
    pub fn from_texts<S: Into<String>>(texts: impl IntoIterator<Item = S>) -> Self {
        let mut script: Vec<ScriptToken> =
            texts.into_iter().map(|s| ScriptToken::text(s)).collect();
        script.push(ScriptToken::EndOfSequence);
        Self::new(script)
    }

    /// Set the simulated context window size, enabling the context-window guard
    /// to be exercised. Returns `self` for builder-style chaining.
    pub fn with_context_size(mut self, context_size: usize) -> Self {
        self.context_size = context_size;
        self
    }

    /// The prompts fed to this model across all generation calls, in order.
    ///
    /// Lets tests assert what the generation layer actually handed the model —
    /// for example to verify chat-template rendering or budget computations.
    pub fn fed_prompts(&self) -> Vec<String> {
        self.fed_prompts
            .lock()
            .expect("scripted model prompt log poisoned")
            .clone()
    }

    /// The most recent prompt fed to this model, if any.
    pub fn last_prompt(&self) -> Option<String> {
        self.fed_prompts
            .lock()
            .expect("scripted model prompt log poisoned")
            .last()
            .cloned()
    }

    /// Record a fed prompt for later inspection.
    fn record_prompt(&self, prompt: &str) {
        self.fed_prompts
            .lock()
            .expect("scripted model prompt log poisoned")
            .push(prompt.to_string());
    }

    /// Resolve the generation budget for a request, mirroring the production
    /// streaming path: the caller's `max_tokens` is the number of NEW tokens to
    /// produce, defaulting to 512 when unset.
    fn budget(request: &GenerationRequest) -> usize {
        request.max_tokens.unwrap_or(512) as usize
    }

    /// Run the shared scripted streaming loop, emitting [`StreamChunk`]s through
    /// `send_chunk` and returning the completion reason.
    ///
    /// This is the single source of truth for the double's streaming behaviour;
    /// both the unbounded ([`TextGenerator::generate_stream`]) and any future
    /// sender variants funnel through it. It deliberately mirrors the control
    /// flow of `GenerationHelper::generate_stream_with_borrowed_model`:
    /// context-window guard, cancellation, EOS detection, stop-token check, and
    /// the per-token / completion chunk shapes.
    ///
    /// `prompt_tokens` is the simulated prompt length used by the
    /// context-window guard. For the scripted double we approximate it with the
    /// whitespace-split word count of the prompt, which is enough to drive the
    /// guard deterministically.
    fn run_stream<F>(
        &self,
        prompt: &str,
        request: &GenerationRequest,
        cancellation_token: &CancellationToken,
        mut send_chunk: F,
    ) -> Result<(), GenerationError>
    where
        F: FnMut(StreamChunk) -> Result<(), ()>,
    {
        self.record_prompt(prompt);

        let mut config = GenerationConfig::for_streaming();
        if let Some(max_tokens) = request.max_tokens {
            config.max_tokens = max_tokens;
        }
        config.stop_tokens = request.stop_tokens.clone();
        config.validate().map_err(GenerationError::InvalidConfig)?;

        let max_tokens = Self::budget(request);
        let prompt_tokens = simulated_prompt_tokens(prompt);

        let mut generated_text = String::new();
        let mut tokens_generated = 0usize;
        let mut script = self.script.iter();

        while tokens_generated < max_tokens {
            // Context-window guard, mirroring the production loop's check
            // against `context_size - 1`.
            if prompt_tokens + tokens_generated >= self.context_size.saturating_sub(1) {
                return self.complete(tokens_generated, "ContextWindowFull", &mut send_chunk);
            }

            if cancellation_token.is_cancelled() {
                // Production sends a worker error and returns Ok(()); the double
                // simply stops streaming cleanly.
                return Ok(());
            }

            // Pull the next scripted decode step. Running out of script is a
            // clean stop, equivalent to the script implicitly ending.
            let Some(step) = script.next() else {
                return self.complete(tokens_generated, "EndOfSequence", &mut send_chunk);
            };

            let token_text = match step {
                ScriptToken::EndOfSequence => {
                    return self.complete(tokens_generated, "EndOfSequence", &mut send_chunk);
                }
                ScriptToken::Text(text) => text.clone(),
            };

            generated_text.push_str(&token_text);
            tokens_generated += 1;

            let chunk = StreamChunk {
                text: token_text,
                is_complete: false,
                token_count: 1,
                finish_reason: None,
            };
            if send_chunk(chunk).is_err() {
                // Receiver disconnected: production returns Ok(()) and stops.
                return Ok(());
            }

            if should_stop(&generated_text, &config.stop_tokens) {
                return self.complete(tokens_generated, "StopToken", &mut send_chunk);
            }
        }

        self.complete(tokens_generated, "MaxTokens", &mut send_chunk)
    }

    /// Emit the final completion chunk, mirroring
    /// `GenerationHelper::handle_streaming_completion`: empty text,
    /// `is_complete: true`, `token_count: 0`, and the completion reason.
    fn complete<F>(
        &self,
        tokens_generated: usize,
        reason: &str,
        send_chunk: &mut F,
    ) -> Result<(), GenerationError>
    where
        F: FnMut(StreamChunk) -> Result<(), ()>,
    {
        let _ = tokens_generated;
        let final_chunk = StreamChunk {
            text: String::new(),
            is_complete: true,
            token_count: 0,
            finish_reason: Some(FinishReason::Stopped(reason.to_string())),
        };
        let _ = send_chunk(final_chunk);
        Ok(())
    }

    /// Run the scripted loop and collect the result into a [`GenerationResponse`],
    /// used by the non-streaming [`TextGenerator`] methods. The accumulated text
    /// and token count come from the same loop that drives streaming, so the two
    /// paths stay consistent.
    fn run_batch(
        &self,
        prompt: &str,
        request: &GenerationRequest,
        cancellation_token: &CancellationToken,
    ) -> Result<GenerationResponse, GenerationError> {
        let start_time = Instant::now();
        let mut generated_text = String::new();
        let mut tokens_generated = 0u32;
        let mut finish_reason = FinishReason::Stopped("EndOfSequence".to_string());

        self.run_stream(prompt, request, cancellation_token, |chunk| {
            if let Some(reason) = chunk.finish_reason {
                finish_reason = reason;
            } else {
                generated_text.push_str(&chunk.text);
                tokens_generated += chunk.token_count as u32;
            }
            Ok(())
        })?;

        Ok(GenerationResponse {
            generated_text,
            tokens_generated,
            generation_time: start_time.elapsed(),
            finish_reason,
            complete_token_sequence: None,
        })
    }
}

/// Approximate the token length of a prompt for the context-window guard.
///
/// The scripted double has no tokenizer, so it uses the whitespace-split word
/// count as a deterministic stand-in. An empty prompt counts as zero tokens.
fn simulated_prompt_tokens(prompt: &str) -> usize {
    prompt.split_whitespace().count()
}

/// Stop-token check matching `GenerationHelper::should_stop`: stop if the
/// accumulated text contains any configured stop token.
fn should_stop(generated_text: &str, stop_tokens: &[String]) -> bool {
    stop_tokens
        .iter()
        .any(|stop_token| generated_text.contains(stop_token))
}

impl TextGenerator for ScriptedModel {
    fn generate_text(
        &mut self,
        prompt: &str,
        request: GenerationRequest,
        cancellation_token: CancellationToken,
    ) -> Result<GenerationResponse, GenerationError> {
        self.run_batch(prompt, &request, &cancellation_token)
    }

    fn generate_stream(
        &mut self,
        prompt: &str,
        request: GenerationRequest,
        stream_sender: mpsc::UnboundedSender<Result<StreamChunk, QueueError>>,
        cancellation_token: CancellationToken,
    ) -> Result<(), GenerationError> {
        self.run_stream(prompt, &request, &cancellation_token, |chunk| {
            stream_sender.send(Ok(chunk)).map_err(|_| ())
        })
    }

    fn generate_text_with_context(
        &mut self,
        prompt: &str,
        request: GenerationRequest,
        cancellation_token: CancellationToken,
        _context_state: Option<&mut crate::types::sessions::ContextState>,
    ) -> Result<GenerationResponse, GenerationError> {
        self.run_batch(prompt, &request, &cancellation_token)
    }

    fn generate_stream_with_context(
        &mut self,
        prompt: &str,
        request: GenerationRequest,
        stream_sender: mpsc::UnboundedSender<Result<StreamChunk, QueueError>>,
        cancellation_token: CancellationToken,
        _context_state: Option<&mut crate::types::sessions::ContextState>,
    ) -> Result<(), GenerationError> {
        self.run_stream(prompt, &request, &cancellation_token, |chunk| {
            stream_sender.send(Ok(chunk)).map_err(|_| ())
        })
    }

    fn generate_text_with_template_offset(
        &mut self,
        prompt: &str,
        request: GenerationRequest,
        cancellation_token: CancellationToken,
        _template_token_count: Option<usize>,
    ) -> Result<GenerationResponse, GenerationError> {
        self.run_batch(prompt, &request, &cancellation_token)
    }

    fn generate_stream_with_template_offset(
        &mut self,
        prompt: &str,
        request: GenerationRequest,
        stream_sender: mpsc::UnboundedSender<Result<StreamChunk, QueueError>>,
        cancellation_token: CancellationToken,
        _template_token_count: Option<usize>,
    ) -> Result<(), GenerationError> {
        self.run_stream(prompt, &request, &cancellation_token, |chunk| {
            stream_sender.send(Ok(chunk)).map_err(|_| ())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ids::SessionId;

    /// Build a default streaming request with an optional token budget.
    fn request(max_tokens: Option<u32>) -> GenerationRequest {
        let mut req = GenerationRequest::new(SessionId::new());
        req.max_tokens = max_tokens;
        req
    }

    /// Drain an unbounded receiver into a Vec of chunks, unwrapping each Ok.
    fn drain(mut rx: mpsc::UnboundedReceiver<Result<StreamChunk, QueueError>>) -> Vec<StreamChunk> {
        let mut chunks = Vec::new();
        while let Ok(item) = rx.try_recv() {
            chunks.push(item.expect("scripted stream should not yield errors"));
        }
        chunks
    }

    #[test]
    fn scripted_model_streams_exact_tokens() {
        // A 5-token script should stream 5 text chunks plus a completion chunk,
        // with the concatenated text equal to the script and a token_count of 5.
        let mut model = ScriptedModel::from_texts(["Hello", ", ", "scripted", " ", "world"]);
        let (tx, rx) = mpsc::unbounded_channel();

        model
            .generate_stream("say hello", request(Some(64)), tx, CancellationToken::new())
            .expect("streaming should succeed");

        let chunks = drain(rx);

        // 5 per-token chunks + 1 completion chunk.
        assert_eq!(chunks.len(), 6, "expected 5 token chunks and 1 completion");

        let token_chunks = &chunks[..5];
        let streamed_text: String = token_chunks.iter().map(|c| c.text.as_str()).collect();
        assert_eq!(streamed_text, "Hello, scripted world");

        for chunk in token_chunks {
            assert!(!chunk.is_complete);
            assert_eq!(chunk.token_count, 1);
            assert!(chunk.finish_reason.is_none());
        }

        // Total tokens carried across the stream is 5.
        let total: usize = chunks.iter().map(|c| c.token_count).sum();
        assert_eq!(total, 5);

        let completion = chunks.last().unwrap();
        assert!(completion.is_complete);
        assert_eq!(completion.text, "");
        assert_eq!(completion.token_count, 0);
        assert_eq!(
            completion.finish_reason,
            Some(FinishReason::Stopped("EndOfSequence".to_string()))
        );

        // The prompt the loop was fed is recorded for inspection.
        assert_eq!(model.fed_prompts(), vec!["say hello".to_string()]);
    }

    #[test]
    fn scripted_model_immediate_eos_yields_empty() {
        // Scripting EOS first reproduces the "0 tokens generated" shape: no token
        // chunks at all, and a normal completion (Ok) with the EndOfSequence
        // reason — NOT an error.
        let mut model = ScriptedModel::new([ScriptToken::EndOfSequence]);
        let (tx, rx) = mpsc::unbounded_channel();

        let result =
            model.generate_stream("anything", request(Some(64)), tx, CancellationToken::new());

        assert!(result.is_ok(), "immediate EOS is a normal completion");

        let chunks = drain(rx);
        assert_eq!(chunks.len(), 1, "only the completion chunk is emitted");

        let completion = &chunks[0];
        assert!(completion.is_complete);
        assert_eq!(completion.text, "");
        assert_eq!(completion.token_count, 0);
        assert_eq!(
            completion.finish_reason,
            Some(FinishReason::Stopped("EndOfSequence".to_string()))
        );

        // No tokens were generated: summing token_count across the stream is 0.
        let total: usize = chunks.iter().map(|c| c.token_count).sum();
        assert_eq!(total, 0);
    }

    #[test]
    fn scripted_model_emits_tool_call_sequence_midstream() {
        // A tool-call sequence is just a run of text tokens forming the call
        // syntax; the double can emit it mid-stream to drive the agentic loop's
        // tool path.
        let mut model = ScriptedModel::from_texts([
            "Let me check. ",
            "<tool_call>",
            "{\"name\":\"echo\"}",
            "</tool_call>",
        ]);
        let (tx, rx) = mpsc::unbounded_channel();

        model
            .generate_stream(
                "use a tool",
                request(Some(64)),
                tx,
                CancellationToken::new(),
            )
            .expect("streaming should succeed");

        let text: String = drain(rx).iter().map(|c| c.text.clone()).collect();
        assert_eq!(
            text,
            "Let me check. <tool_call>{\"name\":\"echo\"}</tool_call>"
        );
    }

    #[test]
    fn scripted_model_stops_at_max_tokens_budget() {
        // With a script longer than the budget, generation stops at the budget
        // with the MaxTokens reason.
        let mut model = ScriptedModel::from_texts(["a", "b", "c", "d", "e", "f", "g", "h"]);
        let (tx, rx) = mpsc::unbounded_channel();

        model
            .generate_stream("count", request(Some(3)), tx, CancellationToken::new())
            .expect("streaming should succeed");

        let chunks = drain(rx);
        let token_chunks: Vec<_> = chunks.iter().filter(|c| !c.is_complete).collect();
        assert_eq!(token_chunks.len(), 3, "budget caps generation at 3 tokens");

        let completion = chunks.last().unwrap();
        assert_eq!(
            completion.finish_reason,
            Some(FinishReason::Stopped("MaxTokens".to_string()))
        );
    }

    #[test]
    fn scripted_model_honors_stop_tokens() {
        // The accumulated text crossing a stop token ends generation with the
        // StopToken reason.
        let mut model = ScriptedModel::from_texts(["keep ", "going ", "STOP", " never"]);
        let (tx, rx) = mpsc::unbounded_channel();

        let mut req = request(Some(64));
        req.stop_tokens = vec!["STOP".to_string()];
        model
            .generate_stream("until stop", req, tx, CancellationToken::new())
            .expect("streaming should succeed");

        let chunks = drain(rx);
        let text: String = chunks
            .iter()
            .filter(|c| !c.is_complete)
            .map(|c| c.text.clone())
            .collect();
        assert_eq!(text, "keep going STOP");

        let completion = chunks.last().unwrap();
        assert_eq!(
            completion.finish_reason,
            Some(FinishReason::Stopped("StopToken".to_string()))
        );
    }

    #[test]
    fn scripted_model_context_size_guard_fires() {
        // A tiny context size with a multi-word prompt should trip the
        // context-window guard before the full script is replayed.
        let mut model =
            ScriptedModel::from_texts(["one", "two", "three", "four"]).with_context_size(4);
        let (tx, rx) = mpsc::unbounded_channel();

        // Prompt of 2 words → simulated_prompt_tokens == 2. Guard fires when
        // 2 + generated >= 4 - 1 == 3, i.e. after 1 generated token.
        model
            .generate_stream(
                "alpha beta",
                request(Some(64)),
                tx,
                CancellationToken::new(),
            )
            .expect("streaming should succeed");

        let chunks = drain(rx);
        let token_chunks: Vec<_> = chunks.iter().filter(|c| !c.is_complete).collect();
        assert_eq!(token_chunks.len(), 1, "guard fires after one token");

        let completion = chunks.last().unwrap();
        assert_eq!(
            completion.finish_reason,
            Some(FinishReason::Stopped("ContextWindowFull".to_string()))
        );
    }

    #[test]
    fn scripted_model_generate_text_collects_full_response() {
        // The non-streaming path yields the concatenated text and token count.
        let mut model = ScriptedModel::from_texts(["foo", "bar", "baz"]);

        let response = model
            .generate_text("batch please", request(Some(64)), CancellationToken::new())
            .expect("batch generation should succeed");

        assert_eq!(response.generated_text, "foobarbaz");
        assert_eq!(response.tokens_generated, 3);
        assert_eq!(
            response.finish_reason,
            FinishReason::Stopped("EndOfSequence".to_string())
        );
    }

    #[test]
    fn scripted_model_records_each_fed_prompt() {
        // Every generation call records its prompt, in order, for budget /
        // template assertions.
        let mut model = ScriptedModel::from_texts(["x"]);
        let _ = model.generate_text("first", request(Some(8)), CancellationToken::new());
        let _ = model.generate_text("second", request(Some(8)), CancellationToken::new());

        assert_eq!(
            model.fed_prompts(),
            vec!["first".to_string(), "second".to_string()]
        );
        assert_eq!(model.last_prompt(), Some("second".to_string()));
    }
}
