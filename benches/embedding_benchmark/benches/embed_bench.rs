//! Timing harness: Qwen3-Embedding-0.6B on ANE (CoreML) vs CPU (llama.cpp).
//!
//! Same model, quantized:
//! - ANE: CoreML .mlpackage (4-bit palettized) at var/data/models/qwen3-embedding-0.6b/
//! - CPU: GGUF Q8_0 from Qwen/Qwen3-Embedding-0.6B-GGUF (auto-downloaded)
//!
//! Reports: load time, first-embed latency, then min/mean/max over N subsequent embeds.
//!
//! Run: cargo bench -p embedding-benchmark

use std::path::PathBuf;
use std::time::{Duration, Instant};

use model_embedding::TextEmbedder;

const N: usize = 50;

const TEXTS: &[&str] = &[
    "The quick brown fox jumps over the lazy dog.",
    "Quantum computing leverages quantum mechanical phenomena such as \
     superposition and entanglement to perform computations.",
    "Rust is a systems programming language focused on safety and performance.",
    "The mitochondria is the powerhouse of the cell.",
    "In 1969, humans first set foot on the Moon during the Apollo 11 mission.",
    "Machine learning models learn patterns from data to make predictions.",
    "The Great Wall of China stretches over 13,000 miles across northern China.",
    "Photosynthesis converts sunlight into chemical energy in plants.",
    "TCP/IP is the fundamental protocol suite powering the internet.",
    "A neural network consists of layers of interconnected nodes called neurons.",
];

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

struct Stats {
    first: Duration,
    min: Duration,
    max: Duration,
    mean: Duration,
    samples: Vec<Duration>,
}

fn compute_stats(first: Duration, samples: Vec<Duration>) -> Stats {
    let min = *samples.iter().min().unwrap();
    let max = *samples.iter().max().unwrap();
    let mean = samples.iter().sum::<Duration>() / samples.len() as u32;
    Stats {
        first,
        min,
        max,
        mean,
        samples,
    }
}

fn print_stats(label: &str, load_time: Duration, stats: &Stats) {
    println!("\n=== {label} ===");
    println!("  load model:   {:>10.2?}", load_time);
    println!("  first embed:  {:>10.2?}", stats.first);
    println!("  subsequent ({N} embeds, cycling {} texts):", TEXTS.len());
    println!("    min:        {:>10.2?}", stats.min);
    println!("    mean:       {:>10.2?}", stats.mean);
    println!("    max:        {:>10.2?}", stats.max);

    // p50/p95 from sorted samples
    let mut sorted = stats.samples.clone();
    sorted.sort();
    let p50 = sorted[sorted.len() / 2];
    let p95 = sorted[(sorted.len() as f64 * 0.95) as usize];
    println!("    p50:        {:>10.2?}", p50);
    println!("    p95:        {:>10.2?}", p95);
}

fn main() {
    let rt = tokio::runtime::Runtime::new().unwrap();

    // --- ANE (CoreML) ---
    if has_mlpackage() {
        let t0 = Instant::now();
        let model = rt.block_on(async {
            let model = ane_embedding::AneEmbeddingModel::new(ane_config());
            model.load().await.expect("Failed to load ANE model");
            model
        });
        let load_time = t0.elapsed();

        let stats = rt.block_on(async {
            // First embed
            let t = Instant::now();
            model
                .embed_text(TEXTS[0])
                .await
                .expect("ANE embed failed");
            let first = t.elapsed();

            // N subsequent embeds, cycling through texts
            let mut samples = Vec::with_capacity(N);
            for i in 0..N {
                let text = TEXTS[i % TEXTS.len()];
                let t = Instant::now();
                model.embed_text(text).await.expect("ANE embed failed");
                samples.push(t.elapsed());
            }

            compute_stats(first, samples)
        });

        print_stats("ANE CoreML (4-bit palettized)", load_time, &stats);
    } else {
        eprintln!("Skipping ANE: .mlpackage not found");
    }

    // --- llama.cpp CPU ---
    let t0 = Instant::now();
    let model = match rt.block_on(async {
        let config = llama_config();
        let model = llama_embedding::EmbeddingModel::new(config)
            .await
            .map_err(|e| model_embedding::EmbeddingError::Backend(e.into()))?;
        model.load().await.map(|()| model)
    }) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Skipping llama.cpp: {e}");
            return;
        }
    };
    let load_time = t0.elapsed();

    let stats = rt.block_on(async {
        // First embed
        let t = Instant::now();
        model
            .embed_text(TEXTS[0])
            .await
            .expect("Llama embed failed");
        let first = t.elapsed();

        // N subsequent embeds, cycling through texts
        let mut samples = Vec::with_capacity(N);
        for i in 0..N {
            let text = TEXTS[i % TEXTS.len()];
            let t = Instant::now();
            model.embed_text(text).await.expect("Llama embed failed");
            samples.push(t.elapsed());
        }

        compute_stats(first, samples)
    });

    print_stats("llama.cpp CPU (Q8_0)", load_time, &stats);
}
