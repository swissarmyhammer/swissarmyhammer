//! Timing harness: unixcoder-base on ANE (CoreML).
//!
//! 126M encoder-only code embedding model (RoBERTa, 768-dim):
//! - ANE: Palettize4 .mlpackage (seq256) for Apple Neural Engine
//! - No GGUF available for llama.cpp comparison
//!
//! Run: cargo bench -p embedding-benchmark --bench unixcoder_bench

use std::path::PathBuf;
use std::time::{Duration, Instant};

use model_embedding::TextEmbedder;

const N: usize = 50;

const TEXTS: &[&str] = &[
    "fn main() { println!(\"Hello, world!\"); }",
    "def fibonacci(n):\n    if n <= 1:\n        return n\n    return fibonacci(n-1) + fibonacci(n-2)",
    "class LinkedList<T> { head: Option<Box<Node<T>>> }",
    "SELECT u.name, COUNT(o.id) FROM users u JOIN orders o ON u.id = o.user_id GROUP BY u.name",
    "async function fetchData(url) { const res = await fetch(url); return res.json(); }",
    "impl Iterator for Fibonacci { type Item = u64; fn next(&mut self) -> Option<u64> { } }",
    "func (s *Server) handleRequest(w http.ResponseWriter, r *http.Request) { }",
    "#include <iostream>\nint main() { std::cout << \"Hello\" << std::endl; return 0; }",
    "const router = express.Router(); router.get('/api/users', getUsers);",
    "public record Point(double x, double y) implements Comparable<Point> { }",
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
    let dir = workspace_root().join("var/data/models/unixcoder-base");
    ane_embedding::AneEmbeddingConfig {
        model_dir: dir,
        model_prefix: "unixcoder-base".to_string(),
        normalize_embeddings: true,
        seq_length: 256,
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

fn main() {
    let rt = tokio::runtime::Runtime::new().unwrap();

    println!("unixcoder-base (126M): ANE (palettize4 CoreML)");
    println!("ANE uses static-shape seq256 — inputs padded/truncated to 256 tokens");
    println!(
        "N={N} embeds per config, cycling {} code snippets\n",
        TEXTS.len()
    );

    if mlpackage_path().exists() {
        let t0 = Instant::now();
        let model = rt.block_on(async {
            let model = ane_embedding::AneEmbeddingModel::new(ane_config());
            model.load().await.expect("Failed to load ANE model");
            model
        });
        let load_time = t0.elapsed();

        let stats = rt.block_on(async {
            let t = Instant::now();
            model.embed_text(TEXTS[0]).await.expect("embed failed");
            let first = t.elapsed();

            let mut samples = Vec::with_capacity(N);
            for i in 0..N {
                let text = TEXTS[i % TEXTS.len()];
                let t = Instant::now();
                model.embed_text(text).await.expect("embed failed");
                samples.push(t.elapsed());
            }

            compute_stats(first, samples)
        });

        print_stats("ANE CoreML (palettize4, static seq256)", load_time, &stats);
        drop(model);
    } else {
        println!(
            "  ANE CoreML: skipped (no .mlpackage at {})",
            mlpackage_path().display()
        );
    }

    println!();
}
