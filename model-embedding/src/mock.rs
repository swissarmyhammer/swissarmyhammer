//! Mock embedder for use in tests across workspace crates.
//!
//! Gated behind the `test-support` feature flag. Provides a [`MockEmbedder`]
//! that returns fixed-dimension vectors, can simulate `ModelNotLoaded` errors,
//! and can inject failures at specific call indices.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::error::EmbeddingError;
use crate::private::Sealed;
use crate::types::EmbeddingResult;
use crate::TextEmbedder;

/// A mock embedder that returns fixed-dimension vectors and can simulate failures.
///
/// Each call to `embed_text` returns a vector of `[0.1; dimension]` unless
/// the call index is in the failure list.
pub struct MockEmbedder {
    dimension: usize,
    loaded: bool,
    /// Indices of embed_text calls (0-based) that should return an error.
    fail_on_calls: Vec<usize>,
    /// If set, return `ModelNotLoaded` instead of `TextProcessing` for failures.
    model_not_loaded: bool,
    /// Shared counter for how many times embed_text was called.
    call_count: Arc<Mutex<usize>>,
}

impl MockEmbedder {
    /// Create a loaded mock that always succeeds.
    pub fn new(dimension: usize) -> Self {
        Self {
            dimension,
            loaded: true,
            fail_on_calls: vec![],
            model_not_loaded: false,
            call_count: Arc::new(Mutex::new(0)),
        }
    }

    /// Create a mock that fails with `ModelNotLoaded` on the given call indices.
    pub fn with_model_not_loaded(dimension: usize, fail_on_calls: Vec<usize>) -> Self {
        Self {
            fail_on_calls,
            model_not_loaded: true,
            ..Self::new(dimension)
        }
    }

    /// Create a mock that fails with `TextProcessing` error on the given call indices.
    pub fn with_failures(dimension: usize, fail_on_calls: Vec<usize>) -> Self {
        Self {
            fail_on_calls,
            ..Self::new(dimension)
        }
    }

    /// Number of times `embed_text` has been called on this mock.
    ///
    /// Tests can clone the mock through an `Arc` and check this counter after
    /// exercising code that should have driven the embedder.
    pub fn call_count(&self) -> usize {
        *self.call_count.lock().unwrap()
    }
}

impl Sealed for MockEmbedder {}

#[async_trait]
impl TextEmbedder for MockEmbedder {
    async fn load(&self) -> Result<(), EmbeddingError> {
        Ok(())
    }

    async fn embed_text(&self, text: &str) -> Result<EmbeddingResult, EmbeddingError> {
        let call_idx = {
            let mut count = self.call_count.lock().unwrap();
            let idx = *count;
            *count += 1;
            idx
        };

        if self.fail_on_calls.contains(&call_idx) {
            if self.model_not_loaded {
                return Err(EmbeddingError::ModelNotLoaded);
            }
            return Err(EmbeddingError::TextProcessing(format!(
                "mock failure at call {call_idx}",
            )));
        }

        let embedding = vec![0.1_f32; self.dimension];
        Ok(EmbeddingResult::new(
            text.to_string(),
            embedding,
            text.split_whitespace().count(),
            1,
        ))
    }

    fn embedding_dimension(&self) -> Option<usize> {
        if self.loaded && self.dimension > 0 {
            Some(self.dimension)
        } else {
            None
        }
    }

    fn is_loaded(&self) -> bool {
        self.loaded
    }
}
