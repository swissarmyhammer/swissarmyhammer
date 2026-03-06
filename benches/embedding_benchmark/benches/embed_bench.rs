//! Criterion benchmarks: Qwen3-Embedding-0.6B on ANE (CoreML) vs CPU (llama.cpp).
//!
//! Same model, two backends:
//! - ANE: CoreML .mlpackage at var/data/models/qwen3-embedding-0.6b/
//! - CPU: GGUF Q8_0 from Qwen/Qwen3-Embedding-0.6B-GGUF (auto-downloaded)
//!
//! Run: cargo bench -p embedding-benchmark

use std::path::PathBuf;

use criterion::{criterion_group, criterion_main, Criterion};
use model_embedding::TextEmbedder;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn has_mlpackage() -> bool {
    workspace_root()
        .join("var/data/models/qwen3-embedding-0.6b/Qwen3-Embedding-0.6B.mlpackage")
        .exists()
}

fn ane_config() -> ane_embedding::AneEmbeddingConfig {
    let dir = workspace_root().join("var/data/models/qwen3-embedding-0.6b");
    ane_embedding::AneEmbeddingConfig {
        model_source: model_loader::ModelSource::Local {
            folder: dir,
            filename: Some("Qwen3-Embedding-0.6B.mlpackage".to_string()),
        },
        max_sequence_length: 512,
        normalize_embeddings: true,
        debug: false,
    }
}

fn llama_config() -> llama_embedding::EmbeddingConfig {
    // Same model (Qwen3-Embedding-0.6B) in GGUF Q8_0 format for llama.cpp
    llama_embedding::EmbeddingConfig {
        model_source: model_loader::ModelSource::HuggingFace {
            repo: "Qwen/Qwen3-Embedding-0.6B-GGUF".to_string(),
            filename: Some("Qwen3-Embedding-0.6B-Q8_0.gguf".to_string()),
            folder: None,
        },
        normalize_embeddings: true,
        max_sequence_length: None,
        debug: false,
    }
}

const SHORT_TEXT: &str = "The quick brown fox jumps over the lazy dog.";
const LONG_TEXT: &str = "Quantum computing leverages quantum mechanical phenomena such as \
    superposition and entanglement to perform computations that would be impractical \
    for classical computers. Quantum bits, or qubits, can exist in multiple states \
    simultaneously, enabling massive parallelism for certain problem classes including \
    cryptography, optimization, and molecular simulation.";

fn bench_ane_embed(c: &mut Criterion) {
    if !has_mlpackage() {
        eprintln!("Skipping ANE benchmarks: .mlpackage not found");
        return;
    }

    let rt = tokio::runtime::Runtime::new().unwrap();

    let model = rt.block_on(async {
        let model = ane_embedding::AneEmbeddingModel::new(ane_config());
        model.load().await.expect("Failed to load ANE model");
        model
    });

    let mut group = c.benchmark_group("qwen3_0.6b_ane_coreml");
    group.sample_size(20);

    group.bench_function("embed_short", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                model
                    .embed_text(SHORT_TEXT)
                    .await
                    .expect("ANE embed failed")
            });
    });

    group.bench_function("embed_long", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                model
                    .embed_text(LONG_TEXT)
                    .await
                    .expect("ANE embed failed")
            });
    });

    group.finish();
}

fn bench_llama_embed(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let model = match rt.block_on(async {
        let config = llama_config();
        let model = llama_embedding::EmbeddingModel::new(config)
            .await
            .map_err(|e| model_embedding::EmbeddingError::Backend(e.into()))?;
        model.load().await.map(|()| model)
    }) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Skipping llama benchmarks: {e}");
            return;
        }
    };

    let mut group = c.benchmark_group("qwen3_0.6b_llama_cpu");
    group.sample_size(20);

    group.bench_function("embed_short", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                model
                    .embed_text(SHORT_TEXT)
                    .await
                    .expect("Llama embed failed")
            });
    });

    group.bench_function("embed_long", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                model
                    .embed_text(LONG_TEXT)
                    .await
                    .expect("Llama embed failed")
            });
    });

    group.finish();
}

criterion_group!(benches, bench_ane_embed, bench_llama_embed);
criterion_main!(benches);
