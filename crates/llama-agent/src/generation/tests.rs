//! Tests for generation configuration validation.
//!
//! These tests focus on GenerationConfig validation and related functionality
//! without requiring model loading or mock implementations.

use super::*;

/// Tests for [`super::send_with_backpressure`], which fixed the cutoff
/// at exactly the stream channel's buffer size (100 tokens) seen in the
/// kanban app.
///
/// The prior implementation used `try_send().is_err()` everywhere, treating
/// both `Full` (consumer briefly behind) and `Closed` (receiver dropped) as
/// "disconnected" — so any time the ACP consumer fell behind by 100 chunks
/// the generator quit. These tests pin the two-outcome split: full must
/// retry until drain; closed must exit.
#[cfg(test)]
mod backpressure_tests {
    use super::super::send_with_backpressure;
    use crate::types::{FinishReason, QueueError, StreamChunk};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, Instant};
    use tokio::sync::mpsc;

    /// Build a stream chunk carrying `text`; values are irrelevant to the
    /// backpressure logic, only the slot occupancy matters.
    fn make_chunk(text: &str) -> Result<StreamChunk, QueueError> {
        Ok(StreamChunk {
            text: text.to_string(),
            is_complete: false,
            token_count: 1,
            finish_reason: None,
        })
    }

    /// A full channel must *not* surface as a disconnect: the producer
    /// retries until the consumer drains and the send completes. This is the
    /// exact race that cut kanban-app generations off at 100 tokens.
    #[test]
    fn full_channel_backpressures_until_consumer_drains() {
        // Use a tokio runtime so the channel works, but drive the producer
        // off the runtime (mimicking the sync model-decode thread).
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();

        let (tx, mut rx) = rt.block_on(async { mpsc::channel(2) });
        // Fill the channel to capacity ahead of time.
        rt.block_on(async {
            tx.send(make_chunk("a")).await.unwrap();
            tx.send(make_chunk("b")).await.unwrap();
        });

        let sent_at = Arc::new(AtomicUsize::new(0));
        let sent_at_writer = sent_at.clone();
        let producer = thread::spawn(move || {
            // This will block until at least one slot frees up.
            let start = Instant::now();
            let r = send_with_backpressure(&tx, make_chunk("c"));
            sent_at_writer.store(start.elapsed().as_millis() as usize, Ordering::SeqCst);
            r
        });

        // Brief pause to prove the producer is blocked, then drain.
        thread::sleep(Duration::from_millis(50));
        assert_eq!(
            sent_at.load(Ordering::SeqCst),
            0,
            "producer should still be blocked on the full channel"
        );

        // Drain one slot. The producer must unblock and complete.
        let drained = rt.block_on(rx.recv()).expect("first chunk drains cleanly");
        assert_eq!(drained.unwrap().text, "a");

        let result = producer.join().expect("producer thread joined");
        assert!(result.is_ok(), "send must succeed after the consumer drains");
        let elapsed_ms = sent_at.load(Ordering::SeqCst);
        assert!(
            elapsed_ms >= 40,
            "producer should have spent ~50ms backpressured, was {}ms",
            elapsed_ms
        );

        // The retry then lands.
        rt.block_on(async {
            let b = rx.recv().await.unwrap().unwrap();
            assert_eq!(b.text, "b");
            let c = rx.recv().await.unwrap().unwrap();
            assert_eq!(c.text, "c");
        });
    }

    /// A *closed* channel (receiver dropped) must surface as an error
    /// immediately — that is the only condition that should end generation
    /// early. Without this distinction, the producer would either spin
    /// forever or bail on a full channel.
    #[test]
    fn closed_channel_returns_error_quickly() {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();
        let (tx, rx) = rt.block_on(async { mpsc::channel(1) });
        drop(rx);

        let start = Instant::now();
        let result = send_with_backpressure(&tx, make_chunk("x"));
        let elapsed = start.elapsed();

        assert!(result.is_err(), "closed channel must surface as an error");
        assert!(
            elapsed < Duration::from_millis(100),
            "closed must NOT spin (took {:?})",
            elapsed
        );
    }

    /// The bug scenario: a slow consumer falls 100 chunks behind, then
    /// catches up. With the old `try_send().is_err()` code the producer
    /// would have bailed on chunk 3 (capacity 2); with backpressure it
    /// completes all 5 and the consumer receives them in order.
    #[test]
    fn producer_emits_more_chunks_than_channel_capacity() {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        let (tx, mut rx) = rt.block_on(async { mpsc::channel(2) });

        let producer = thread::spawn(move || -> Result<(), u32> {
            for i in 0..5u32 {
                let payload = Ok(StreamChunk {
                    text: format!("t{i}"),
                    is_complete: false,
                    token_count: 1,
                    finish_reason: None,
                });
                if send_with_backpressure(&tx, payload).is_err() {
                    return Err(i);
                }
            }
            // Final completion chunk.
            let done = Ok(StreamChunk {
                text: String::new(),
                is_complete: true,
                token_count: 0,
                finish_reason: Some(FinishReason::Stopped("EndOfSequence".into())),
            });
            send_with_backpressure(&tx, done).map_err(|_| 99u32)?;
            Ok(())
        });

        // Slow consumer: 25ms between drains. Channel of 2 will saturate
        // quickly and exercise the backpressure path repeatedly.
        let mut collected: Vec<String> = Vec::new();
        let mut saw_complete = false;
        rt.block_on(async {
            while let Some(item) = rx.recv().await {
                let chunk = item.expect("no error chunks");
                if chunk.is_complete {
                    saw_complete = true;
                    break;
                }
                collected.push(chunk.text);
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        });

        let producer_result = producer.join().expect("producer joined");
        assert!(
            producer_result.is_ok(),
            "producer must complete all 5 chunks despite capacity 2, got: {:?}",
            producer_result
        );
        assert_eq!(
            collected,
            vec!["t0", "t1", "t2", "t3", "t4"],
            "consumer must receive every chunk in order — none dropped"
        );
        assert!(saw_complete, "completion chunk must arrive");
    }
}

/// Guard against regression: every chunk-emit site in the streaming paths
/// must go through `send_with_backpressure`. A stray `try_send` on a chunk
/// would re-introduce the 100-token cutoff. This is a source-level grep,
/// run as a test so it gates CI.
#[cfg(test)]
mod no_try_send_for_chunks {
    /// Searches a source file for `try_send(Ok(` (stream payloads) — those
    /// MUST be `send_with_backpressure`. `try_send(Err(...))` sites are
    /// allowed only on cancellation paths through the helper itself.
    fn assert_no_try_send_chunks(path: &str) {
        let body = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("read {path}: {e}"));
        for (i, line) in body.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") || trimmed.starts_with("///") {
                continue;
            }
            assert!(
                !line.contains("try_send(Ok("),
                "{path}:{lineno} emits chunks via raw try_send — must use \
                 send_with_backpressure (line: {line:?})",
                lineno = i + 1,
            );
        }
    }

    #[test]
    fn standard_streaming_path_uses_backpressure() {
        assert_no_try_send_chunks("src/generation/mod.rs");
    }

    #[test]
    fn mtp_streaming_path_uses_backpressure() {
        assert_no_try_send_chunks("src/generation/mtp/streaming.rs");
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_generation_config_default() {
        let config = GenerationConfig::default();

        assert_eq!(config.max_tokens, 4096);
        assert_eq!(config.temperature, 0.7);
        assert_eq!(config.top_p, 0.9);
        assert!(config.stop_tokens.is_empty());
        assert_eq!(config.seed, 1234);
        assert!(config.use_greedy);
    }

    #[test]
    fn test_generation_config_for_batch() {
        let config = GenerationConfig::for_batch_generation();

        assert_eq!(config.max_tokens, 4096);
        assert!(!config.use_greedy); // Should allow flexible sampling
    }

    #[test]
    fn test_generation_config_for_streaming() {
        let config = GenerationConfig::for_streaming();

        assert_eq!(config.max_tokens, 4096);
        assert!(!config.use_greedy); // Should allow flexible sampling
    }

    #[test]
    fn test_generation_config_for_compaction() {
        let config = GenerationConfig::for_compaction();

        assert_eq!(config.max_tokens, 512);
        assert_eq!(config.temperature, 0.0); // Deterministic
        assert!(config.use_greedy); // Matches existing compaction behavior
    }

    #[test]
    fn test_generation_config_validation_success() {
        let config = GenerationConfig {
            max_tokens: 100,
            temperature: 0.7,
            top_p: 0.9,
            stop_tokens: vec!["stop".to_string()],
            seed: 1234,
            use_greedy: false,
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_generation_config_validation_zero_tokens() {
        let config = GenerationConfig {
            max_tokens: 0,
            ..Default::default()
        };

        assert!(config.validate().is_err());
        assert!(config
            .validate()
            .unwrap_err()
            .contains("max_tokens must be greater than 0"));
    }

    #[test]
    fn test_generation_config_validation_excessive_tokens() {
        let config = GenerationConfig {
            max_tokens: 200_000,
            ..Default::default()
        };

        assert!(config.validate().is_err());
        assert!(config
            .validate()
            .unwrap_err()
            .contains("cannot exceed 100,000"));
    }

    #[test]
    fn test_generation_config_validation_invalid_temperature() {
        let config = GenerationConfig {
            temperature: 3.0,
            ..Default::default()
        };

        assert!(config.validate().is_err());
        assert!(config
            .validate()
            .unwrap_err()
            .contains("temperature must be between 0.0 and 2.0"));
    }

    #[test]
    fn test_generation_config_validation_invalid_top_p() {
        let config = GenerationConfig {
            top_p: 1.5,
            ..Default::default()
        };

        assert!(config.validate().is_err());
        assert!(config
            .validate()
            .unwrap_err()
            .contains("top_p must be between 0.0 and 1.0"));
    }

    #[test]
    fn test_generation_config_validation_too_many_stop_tokens() {
        let config = GenerationConfig {
            stop_tokens: (0..15).map(|i| format!("stop{}", i)).collect(),
            ..Default::default()
        };

        assert!(config.validate().is_err());
        assert!(config
            .validate()
            .unwrap_err()
            .contains("Cannot specify more than 10 stop tokens"));
    }

    #[test]
    fn test_generation_config_validation_empty_stop_token() {
        let config = GenerationConfig {
            stop_tokens: vec!["".to_string()],
            ..Default::default()
        };

        assert!(config.validate().is_err());
        assert!(config
            .validate()
            .unwrap_err()
            .contains("Stop tokens cannot be empty"));
    }

    #[test]
    fn test_generation_config_validation_long_stop_token() {
        let config = GenerationConfig {
            stop_tokens: vec!["a".repeat(100)],
            ..Default::default()
        };

        assert!(config.validate().is_err());
        assert!(config
            .validate()
            .unwrap_err()
            .contains("Stop tokens cannot exceed 50 characters"));
    }

    #[test]
    fn test_generation_error_creation() {
        let err = std::io::Error::other("test error");

        let gen_err = GenerationError::tokenization(err);
        assert!(matches!(gen_err, GenerationError::TokenizationFailed(_)));

        let err = std::io::Error::other("batch error");
        let gen_err = GenerationError::batch(err);
        assert!(matches!(gen_err, GenerationError::BatchFailed(_)));

        let err = std::io::Error::other("decode error");
        let gen_err = GenerationError::decoding(err);
        assert!(matches!(gen_err, GenerationError::DecodingFailed(_)));

        let err = std::io::Error::other("conversion error");
        let gen_err = GenerationError::token_conversion(err);
        assert!(matches!(gen_err, GenerationError::TokenConversionFailed(_)));

        let err = std::io::Error::other("context error");
        let gen_err = GenerationError::context(err);
        assert!(matches!(gen_err, GenerationError::ContextFailed(_)));

        let err = std::io::Error::other("generation error");
        let gen_err = GenerationError::generation(err);
        assert!(matches!(gen_err, GenerationError::GenerationFailed(_)));
    }

    #[test]
    fn test_generation_error_from_string() {
        let error_msg = "Configuration error".to_string();
        let gen_err: GenerationError = error_msg.into();

        match gen_err {
            GenerationError::InvalidConfig(msg) => {
                assert_eq!(msg, "Configuration error");
            }
            _ => panic!("Expected InvalidConfig error"),
        }
    }
}

#[cfg(test)]
mod template_offset_tests {
    //! Tests for the template-offset decision the production decode loop makes.
    //!
    //! These exercise the real `budget::template_offset_exhausted` predicate that
    //! both offset variants call to decide whether to enter the decode loop or
    //! return an empty response — not inline copies of the arithmetic. The
    //! decode loop itself binds a model and is covered by the small-model
    //! integration tests.

    use crate::generation::budget::template_offset_exhausted;

    #[test]
    fn zero_offset_with_tokens_is_not_exhausted() {
        // Zero offset means "no template" — every token is new, so the loop runs.
        assert!(!template_offset_exhausted(0, 150));
    }

    #[test]
    fn partial_offset_leaves_new_tokens() {
        // 100-token template prefix, 150-token prompt -> 50 new tokens to decode.
        assert!(!template_offset_exhausted(100, 150));
    }

    #[test]
    fn offset_equal_to_total_is_exhausted() {
        // The cached template covers the whole prompt -> nothing new to process.
        assert!(template_offset_exhausted(100, 100));
    }

    #[test]
    fn offset_exceeding_total_is_exhausted_without_underflow() {
        // An offset larger than the prompt must report "exhausted" rather than
        // letting the production `skip(offset)` underflow.
        assert!(template_offset_exhausted(100, 50));
    }
}
