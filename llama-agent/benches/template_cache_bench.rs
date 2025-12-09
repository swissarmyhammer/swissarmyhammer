//! Performance benchmarks for template caching system
//!
//! These benchmarks measure the cache metadata operations including hashing,
//! lookup, insertion, and eviction. They do NOT measure actual KV cache loading
//! or template processing, as those require real llama.cpp models.
//!
//! To run these benchmarks:
//! ```bash
//! cargo bench --bench template_cache_bench
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use llama_agent::{Message, MessageRole, Session, SessionId, TemplateCache, ToolDefinition};
use std::time::SystemTime;
use tempfile::TempDir;

/// Benchmark: Hash computation for template content
///
/// Measures the time to compute a hash of system prompt + tools JSON.
/// This is a critical path operation that happens on every session initialization.
fn bench_template_hashing(c: &mut Criterion) {
    let system_prompt = "You are a helpful assistant with extensive knowledge in programming, mathematics, and general problem-solving.";
    let tools_json = r#"[{"name":"calculate","description":"Perform mathematical calculations","input_schema":{"type":"object","properties":{"expression":{"type":"string"}},"required":["expression"]}},{"name":"search","description":"Search for information","input_schema":{"type":"object","properties":{"query":{"type":"string"}},"required":["query"]}}]"#;

    c.bench_function("template_hash", |b| {
        b.iter(|| {
            let hash =
                TemplateCache::hash_template(black_box(system_prompt), black_box(tools_json));
            black_box(hash);
        });
    });
}

/// Benchmark: Cache lookup (miss scenario)
///
/// Measures the time for a cache lookup that misses. This is the cold-start
/// scenario for the first session with a given template.
fn bench_cache_lookup_miss(c: &mut Criterion) {
    let temp_dir = TempDir::new().unwrap();
    let mut cache = TemplateCache::new(temp_dir.path().to_path_buf()).unwrap();

    let hash = TemplateCache::hash_template("unique_system", "unique_tools");

    c.bench_function("cache_lookup_miss", |b| {
        b.iter(|| {
            let result = cache.get(black_box(hash));
            black_box(result);
        });
    });
}

/// Benchmark: Cache lookup (hit scenario)
///
/// Measures the time for a cache lookup that hits. This is the common case
/// for subsequent sessions using the same template.
fn bench_cache_lookup_hit(c: &mut Criterion) {
    let temp_dir = TempDir::new().unwrap();
    let mut cache = TemplateCache::new(temp_dir.path().to_path_buf()).unwrap();

    let hash = TemplateCache::hash_template("system", "tools");
    cache
        .insert(hash, 1234, "system".to_string(), "tools".to_string())
        .unwrap();

    c.bench_function("cache_lookup_hit", |b| {
        b.iter(|| {
            let result = cache.get(black_box(hash));
            black_box(result);
        });
    });
}

/// Benchmark: Cache insertion
///
/// Measures the time to insert a new cache entry. This happens after
/// processing a new template for the first time.
fn bench_cache_insert(c: &mut Criterion) {
    c.bench_function("cache_insert", |b| {
        b.iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                let cache = TemplateCache::new(temp_dir.path().to_path_buf()).unwrap();
                let hash = TemplateCache::hash_template("system", "tools");
                (cache, temp_dir, hash)
            },
            |(mut cache, _temp_dir, hash)| {
                let result = cache
                    .insert(
                        black_box(hash),
                        black_box(1234),
                        black_box("system".to_string()),
                        black_box("tools".to_string()),
                    )
                    .unwrap();
                black_box(result);
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

/// Benchmark: Cache verification
///
/// Measures the time to verify that a cache entry matches expected content.
/// This is used to ensure cache hits are valid.
fn bench_cache_verify(c: &mut Criterion) {
    let temp_dir = TempDir::new().unwrap();
    let mut cache = TemplateCache::new(temp_dir.path().to_path_buf()).unwrap();

    let hash = TemplateCache::hash_template("system", "tools");
    cache
        .insert(hash, 1234, "system".to_string(), "tools".to_string())
        .unwrap();

    c.bench_function("cache_verify", |b| {
        b.iter(|| {
            let result = cache.verify(black_box(hash), black_box("system"), black_box("tools"));
            black_box(result);
        });
    });
}

/// Benchmark: Cache statistics computation
///
/// Measures the time to compute cache statistics including hit rate and
/// token counts across all entries.
fn bench_cache_stats(c: &mut Criterion) {
    let temp_dir = TempDir::new().unwrap();
    let mut cache = TemplateCache::new(temp_dir.path().to_path_buf()).unwrap();

    // Populate cache with multiple entries
    for i in 0..10 {
        let hash = TemplateCache::hash_template(&format!("system{}", i), &format!("tools{}", i));
        cache
            .insert(hash, 1234, format!("system{}", i), format!("tools{}", i))
            .unwrap();
    }

    c.bench_function("cache_stats", |b| {
        b.iter(|| {
            let stats = cache.stats();
            black_box(stats);
        });
    });
}

/// Benchmark: Cache eviction (LRU)
///
/// Measures the time to evict the least recently used entry when cache is full.
/// This is important for managing memory in long-running systems.
fn bench_cache_eviction(c: &mut Criterion) {
    c.bench_function("cache_eviction", |b| {
        b.iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                let mut cache =
                    TemplateCache::with_max_entries(temp_dir.path().to_path_buf(), Some(5))
                        .unwrap();

                // Fill cache to capacity
                for i in 0..5 {
                    let hash = TemplateCache::hash_template(
                        &format!("system{}", i),
                        &format!("tools{}", i),
                    );
                    cache
                        .insert(hash, 1234, format!("system{}", i), format!("tools{}", i))
                        .unwrap();
                }

                (cache, temp_dir)
            },
            |(mut cache, _temp_dir)| {
                // Insert one more to trigger eviction
                let hash = TemplateCache::hash_template("new_system", "new_tools");
                let result = cache
                    .insert(
                        black_box(hash),
                        black_box(5678),
                        black_box("new_system".to_string()),
                        black_box("new_tools".to_string()),
                    )
                    .unwrap();
                black_box(result);
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

/// Benchmark: Session creation with varying tool counts
///
/// Measures the overhead of creating sessions with different numbers of tools.
/// This simulates the session initialization path before template caching.
fn bench_session_creation_with_tools(c: &mut Criterion) {
    let mut group = c.benchmark_group("session_creation");

    for num_tools in [0, 5, 10, 20, 50].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_tools),
            num_tools,
            |b, &num_tools| {
                b.iter(|| {
                    let tools: Vec<ToolDefinition> = (0..num_tools)
                        .map(|i| ToolDefinition {
                            name: format!("tool_{}", i),
                            description: format!("Tool number {}", i),
                            parameters: serde_json::json!({
                                "type": "object",
                                "properties": {
                                    "param": {"type": "string"}
                                },
                                "required": ["param"]
                            }),
                            server_name: format!("server_{}", i % 3),
                        })
                        .collect();

                    let session = Session {
                        id: SessionId::new(),
                        messages: vec![Message {
                            role: MessageRole::System,
                            content: "You are a helpful assistant.".to_string(),
                            tool_call_id: None,
                            tool_name: None,
                            timestamp: SystemTime::now(),
                        }],
                        mcp_servers: Vec::new(),
                        available_tools: tools,
                        available_prompts: Vec::new(),
                        created_at: SystemTime::now(),
                        updated_at: SystemTime::now(),
                        compaction_history: Vec::new(),
                        transcript_path: None,
                        context_state: None,
                        template_token_count: None,
                    };

                    black_box(session);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Multiple cache operations in sequence
///
/// Measures the performance of a realistic cache usage pattern:
/// hash -> lookup (miss) -> insert -> lookup (hit) -> verify
fn bench_cache_workflow(c: &mut Criterion) {
    c.bench_function("cache_workflow", |b| {
        b.iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                let cache = TemplateCache::new(temp_dir.path().to_path_buf()).unwrap();
                (cache, temp_dir)
            },
            |(mut cache, _temp_dir)| {
                let system = "You are a helpful assistant.";
                let tools = r#"[{"name":"tool1"}]"#;

                // Hash
                let hash = TemplateCache::hash_template(black_box(system), black_box(tools));

                // Lookup (miss)
                let miss = cache.get(black_box(hash));
                black_box(miss);

                // Insert
                cache
                    .insert(
                        black_box(hash),
                        black_box(1234),
                        black_box(system.to_string()),
                        black_box(tools.to_string()),
                    )
                    .unwrap();

                // Lookup (hit)
                let hit = cache.get(black_box(hash));
                black_box(hit);

                // Verify
                let verified = cache.verify(black_box(hash), black_box(system), black_box(tools));
                black_box(verified);
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

/// Benchmark: Hashing with varying content sizes
///
/// Measures how hash computation time scales with content size.
fn bench_hash_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_scaling");

    for size in [100, 500, 1000, 5000, 10000].iter() {
        let system_prompt = "a".repeat(*size);
        let tools_json = "b".repeat(*size);

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let hash =
                    TemplateCache::hash_template(black_box(&system_prompt), black_box(&tools_json));
                black_box(hash);
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_template_hashing,
    bench_cache_lookup_miss,
    bench_cache_lookup_hit,
    bench_cache_insert,
    bench_cache_verify,
    bench_cache_stats,
    bench_cache_eviction,
    bench_session_creation_with_tools,
    bench_cache_workflow,
    bench_hash_scaling,
);

criterion_main!(benches);
