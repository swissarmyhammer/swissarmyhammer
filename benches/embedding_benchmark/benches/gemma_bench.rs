//! Timing harness: EmbeddingGemma-300M on ANE (CoreML) vs CPU (llama.cpp).
//!
//! Same model, quantized:
//! - ANE: Static-shape FP32 .mlpackage (seq128) for Apple Neural Engine
//! - CPU: GGUF Q8_0 from ggml-org/embeddinggemma-300M-GGUF (auto-downloaded)
//!
//! Reports: load time, first-embed latency, then min/mean/max/p50/p95 over N embeds.
//!
//! Run: cargo bench -p embedding-benchmark --bench gemma_bench

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

fn mlpackage_path() -> PathBuf {
    let config = ane_config();
    config.model_path()
}

fn ane_config() -> ane_embedding::AneEmbeddingConfig {
    let dir = workspace_root().join("var/data/models/embeddinggemma-300m");
    ane_embedding::AneEmbeddingConfig {
        model_dir: dir,
        model_prefix: "EmbeddingGemma-300M".to_string(),
        normalize_embeddings: true,
        seq_length: 128,
        debug: false,
    }
}

fn llama_config() -> llama_embedding::EmbeddingConfig {
    llama_embedding::EmbeddingConfig {
        model_source: model_loader::ModelSource::HuggingFace {
            repo: "ggml-org/embeddinggemma-300M-GGUF".to_string(),
            filename: Some("embeddinggemma-300M-Q8_0.gguf".to_string()),
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
    p50: Duration,
    p95: Duration,
}

fn compute_stats(first: Duration, samples: Vec<Duration>) -> Stats {
    let min = *samples.iter().min().unwrap();
    let max = *samples.iter().max().unwrap();
    let mean = samples.iter().sum::<Duration>() / samples.len() as u32;

    let mut sorted = samples;
    sorted.sort();
    let p50 = sorted[sorted.len() / 2];
    let p95 = sorted[(sorted.len() as f64 * 0.95) as usize];

    Stats {
        first,
        min,
        max,
        mean,
        p50,
        p95,
    }
}

fn print_stats(label: &str, load_time: Duration, stats: &Stats) {
    println!("  {label}");
    println!("    load:       {:>10.2?}", load_time);
    println!("    first:      {:>10.2?}", stats.first);
    println!("    min:        {:>10.2?}", stats.min);
    println!("    mean:       {:>10.2?}", stats.mean);
    println!("    max:        {:>10.2?}", stats.max);
    println!("    p50:        {:>10.2?}", stats.p50);
    println!("    p95:        {:>10.2?}", stats.p95);
}

fn bench_embedder(rt: &tokio::runtime::Runtime, model: &dyn TextEmbedder) -> Stats {
    rt.block_on(async {
        // First embed
        let t = Instant::now();
        model.embed_text(TEXTS[0]).await.expect("embed failed");
        let first = t.elapsed();

        // N subsequent embeds, cycling through texts
        let mut samples = Vec::with_capacity(N);
        for i in 0..N {
            let text = TEXTS[i % TEXTS.len()];
            let t = Instant::now();
            model.embed_text(text).await.expect("embed failed");
            samples.push(t.elapsed());
        }

        compute_stats(first, samples)
    })
}

fn main() {
    let rt = tokio::runtime::Runtime::new().unwrap();

    println!("EmbeddingGemma-300M: ANE (FP32+palettize4 CoreML) vs CPU (Q8_0 llama.cpp)");
    println!("ANE uses static-shape seq128 — inputs padded/truncated to 128 tokens");
    println!("N={N} embeds per config, cycling {} texts\n", TEXTS.len());

    // ANE (CoreML) — static-shape FP32 model
    if mlpackage_path().exists() {
        let t0 = Instant::now();
        let model = rt.block_on(async {
            let model = ane_embedding::AneEmbeddingModel::new(ane_config());
            model.load().await.expect("Failed to load ANE model");
            model
        });
        let load_time = t0.elapsed();
        let stats = bench_embedder(&rt, &model);
        print_stats("ANE CoreML (FP32+palettize4, static seq128)", load_time, &stats);

        // Leak the model to avoid SIGABRT on CoreML teardown.
        std::mem::forget(model);
    } else {
        println!(
            "  ANE CoreML: skipped (no .mlpackage at {})",
            mlpackage_path().display()
        );
    }

    println!();

    // llama.cpp CPU
    let t0 = Instant::now();
    match rt.block_on(async {
        let config = llama_config();
        let model = llama_embedding::EmbeddingModel::new(config)
            .await
            .map_err(|e| model_embedding::EmbeddingError::Backend(e.into()))?;
        model.load().await.map(|()| model)
    }) {
        Ok(model) => {
            let load_time = t0.elapsed();
            let stats = bench_embedder(&rt, &model);
            print_stats("llama.cpp CPU (Q8_0)", load_time, &stats);
        }
        Err(e) => {
            println!("  llama.cpp CPU: skipped ({e})");
        }
    }

    println!();
}
