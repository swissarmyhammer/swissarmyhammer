//! Integration tests for swissarmyhammer-embedding.
//!
//! These tests exercise the full pipeline: resolve model name → download from
//! HuggingFace (cached) → load backend → embed text. They hit the network on
//! first run and use the HF cache thereafter.
//!
//! Gated behind the `embedding-models` cargo feature so they don't compile or
//! run in the default workspace build. Run with:
//!   cargo nextest run --features embedding-models --profile embedding-models

#![cfg(feature = "embedding-models")]

use model_embedding::TextEmbedder;
use swissarmyhammer_embedding::Embedder;

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Runtime::new().unwrap()
}

use model_embedding::cosine_similarity;

// -- Qwen3 Embedding (ANE on macOS arm64 only) -------------------------------

/// Output dimension of the Qwen3 embedding model.
const QWEN3_EMBEDDING_DIM: usize = 1024;

/// Model name of the builtin embedding model.
const QWEN_MODEL_NAME: &str = "qwen-embedding";

/// Build the builtin Qwen embedder and load it. Shared by every test below.
async fn load_qwen_embedder() -> Embedder {
    let embedder = Embedder::from_model_name(QWEN_MODEL_NAME)
        .await
        .expect("Failed to create qwen-embedding embedder");
    embedder.load().await.expect("Failed to load model");
    embedder
}

/// Real path: the builtin `qwen-embedding` model loads and embeds one text.
/// On macOS it must resolve to the ANE backend.
#[test]
fn qwen_embedding_load_and_embed() {
    let rt = rt();
    rt.block_on(async {
        let embedder = load_qwen_embedder().await;
        #[cfg(target_os = "macos")]
        assert_eq!(embedder.backend_name(), "ane");

        let result = embedder
            .embed_text("fn main() { println!(\"hello\"); }")
            .await
            .expect("Failed to embed text");

        let embedding = result.embedding();
        assert!(!embedding.is_empty(), "Embedding should not be empty");
        assert_eq!(
            result.dimension(),
            QWEN3_EMBEDDING_DIM,
            "Expected {QWEN3_EMBEDDING_DIM}-dim embedding"
        );
    });
}

#[test]
fn qwen_embedding_nonzero_values() {
    let rt = rt();
    rt.block_on(async {
        let embedder = load_qwen_embedder().await;

        let result = embedder
            .embed_text("struct Vec<T> { ptr: *mut T, len: usize }")
            .await
            .expect("embed failed");

        let embedding = result.embedding();
        let nonzero = embedding.iter().filter(|v| v.abs() > 1e-6).count();
        assert!(
            nonzero > embedding.len() / 2,
            "Most values should be nonzero, got {nonzero}/{}",
            embedding.len()
        );
    });
}

#[test]
fn qwen_embedding_similarity() {
    let rt = rt();
    rt.block_on(async {
        let embedder = load_qwen_embedder().await;

        let rust_result = embedder
            .embed_text("fn fibonacci(n: u64) -> u64 { if n <= 1 { n } else { fibonacci(n-1) + fibonacci(n-2) } }")
            .await
            .expect("embed failed");

        let python_result = embedder
            .embed_text("def fibonacci(n): return n if n <= 1 else fibonacci(n-1) + fibonacci(n-2)")
            .await
            .expect("embed failed");

        let unrelated_result = embedder
            .embed_text("The quick brown fox jumps over the lazy dog.")
            .await
            .expect("embed failed");

        let sim_code = cosine_similarity(rust_result.embedding(), python_result.embedding());
        let sim_unrelated =
            cosine_similarity(rust_result.embedding(), unrelated_result.embedding());

        assert!(
            sim_code > sim_unrelated,
            "Similar code should have higher similarity ({sim_code:.4}) than unrelated text ({sim_unrelated:.4})"
        );
    });
}
