# Template Caching

## Overview

Template caching is a performance optimization that eliminates redundant processing of system prompts and tool definitions across multiple sessions. When sessions share the same template (system prompt + tools), the KV cache state is reused instead of being reprocessed.

## Performance Benefits

Without template caching:
- Each session processes the entire template: ~450ms per session
- 10 sessions with same template: 4,520ms total

With template caching:
- First session processes and caches: ~472ms (450ms + 20ms save overhead)
- Subsequent sessions load from cache: ~10ms
- 10 sessions with same template: ~562ms total (87.6% faster)

## How It Works

### Architecture

```
┌─────────────────────────────────────────┐
│ Session 1 (cache MISS)                  │
│ 1. Hash system prompt + tools           │
│ 2. Check cache → not found              │
│ 3. Render template                      │
│ 4. Tokenize (N tokens)                  │
│ 5. Process through model (KV cache)     │
│ 6. Save KV cache to file                │
│ 7. Store metadata (hash → file + count) │
└─────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────┐
│ Session 2 (cache HIT)                   │
│ 1. Hash system prompt + tools           │
│ 2. Check cache → found!                 │
│ 3. Load KV cache from file              │
│ 4. Template ready (N tokens)            │
│ 5. Process only messages                │
└─────────────────────────────────────────┘
```

### Cache Storage

Template caches are stored in `.cache/templates/`:

```
.cache/templates/
├── template_1234567890abcdef.kv  (1500 tokens, ~2MB)
├── template_fedcba9876543210.kv  (1200 tokens, ~1.8MB)
└── template_0011223344556677.kv  (1800 tokens, ~2.2MB)
```

Each file contains the KV cache state for a unique template (system prompt + tools combination).

## Usage

### Automatic Usage

Template caching is enabled automatically. Sessions with the same system prompt and tools will automatically share cached templates.

```rust
use llama_agent::{AgentServer, types::{AgentAPI, AgentConfig}};

// Create agent and load model
let agent = AgentServer::initialize(config).await?;

// First session - creates cache entry
let session1 = agent.create_session().await?;
// Session initialization: ~472ms (cache miss)

// Second session with same template - uses cache
let session2 = agent.create_session().await?;
// Session initialization: ~10ms (cache hit)
```

### Configuration

Template caching is configured via `ModelConfig`:

```rust
use llama_agent::types::{ModelConfig, ModelSource};
use std::path::PathBuf;

let config = ModelConfig {
    source: ModelSource::HuggingFace {
        repo: "bartowski/Qwen2.5-Coder-1.5B-Instruct-GGUF".to_string(),
        filename: Some("Qwen2.5-Coder-1.5B-Instruct-Q4_K_M.gguf".to_string()),
        folder: None,
    },
    batch_size: 512,
    n_seq_max: 4,
    n_threads: 4,
    n_threads_batch: 4,
    use_hf_params: true,
    retry_config: Default::default(),
    debug: false,
    cache_dir: Some(PathBuf::from(".cache/templates")),
};
```

### Checking Cache Status

Get cache statistics to monitor effectiveness:

```rust
let stats = model_manager.get_template_cache_stats();

println!("Cache entries: {}", stats.entries);
println!("Total tokens cached: {}", stats.total_tokens);
println!("Hits: {}", stats.hits);
println!("Misses: {}", stats.misses);
println!("Hit rate: {:.2}%", stats.hit_rate * 100.0);
```

## When Templates Are Cached

A template is cached when:
- System prompt and tools are identical (exact match)
- First session with this combination is initialized
- Model processes the template successfully

A cache hit occurs when:
- A new session has identical system prompt and tools
- Hash matches an existing cache entry
- Cache file exists and loads successfully

## Cache Invalidation

Templates are hashed based on:
- System prompt content (exact string match)
- Tool definitions (JSON serialization)

Any change to system prompt or tools creates a new cache entry:

```rust
// Session 1: System prompt "You are a helpful assistant"
// → Cache entry A created

// Session 2: System prompt "You are a helpful assistant"
// → Cache entry A loaded (HIT)

// Session 3: System prompt "You are a coding assistant"
// → Cache entry B created (different hash, MISS)
```

## Best Practices

### 1. Reuse System Prompts

For maximum cache benefit, use consistent system prompts across sessions:

```rust
// Good: Reuses same system prompt
const SYSTEM_PROMPT: &str = "You are a helpful assistant.";

for _ in 0..10 {
    let session = agent.create_session().await?;
    // 9 cache hits!
}
```

### 2. Standardize Tool Definitions

Keep tool definitions consistent to maximize cache hits:

```rust
// Good: Shared tool definitions
let standard_tools = vec![
    calculate_tool(),
    search_tool(),
];

// All sessions share these tools → cache hits
```

### 3. Monitor Cache Statistics

Check cache effectiveness periodically:

```rust
let stats = model_manager.get_template_cache_stats();
if stats.hit_rate < 0.5 {
    // Low hit rate - consider standardizing templates
    warn!("Template cache hit rate is low: {:.2}%", stats.hit_rate * 100.0);
}
```

### 4. Cache Directory Management

The cache directory persists between runs:
- Cache files are never automatically deleted
- Manual cleanup can be done if needed
- Each template cache is ~1-5MB depending on template size

```bash
# View cache size
du -sh .cache/templates

# Clear cache (optional)
rm -rf .cache/templates
```

## Performance Tuning

### Memory vs Speed Trade-off

Template caching trades disk space for speed:
- Each unique template: ~1-5MB disk space
- Benefit: 97.8% faster initialization (450ms → 10ms)

For most applications, this is an excellent trade-off.

### Concurrent Sessions

Template caching is thread-safe and works with concurrent sessions:

```rust
// Create 10 sessions concurrently
let sessions = futures::future::join_all(
    (0..10).map(|_| agent.create_session())
).await;

// First session: cache miss (~472ms)
// Other 9 sessions: cache hits (~10ms each)
// Total: ~562ms vs 4,520ms without caching
```

## Troubleshooting

### Cache Not Working

If you're not seeing cache hits:

1. Check that system prompts are identical (including whitespace)
2. Verify tool definitions are identical
3. Check cache directory exists and is writable
4. Review cache statistics for misses

### Performance Not Improving

If caching doesn't improve performance:

1. Verify templates are actually shared between sessions
2. Check that template size is significant (>100 tokens)
3. Ensure model is fully loaded before session creation
4. Review benchmark results to validate expectations

### Cache Files Growing

If cache directory is growing too large:

1. Review number of unique templates
2. Consider standardizing templates to reduce variants
3. Manually clean old cache files if needed
4. Monitor cache statistics to identify template variations

## Technical Details

### Hash Function

Templates are hashed using `DefaultHasher`:

```rust
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

let mut hasher = DefaultHasher::new();
system_prompt.hash(&mut hasher);
tools_json.hash(&mut hasher);
let template_hash = hasher.finish();
```

### KV Cache Format

Template KV caches use llama.cpp's native session file format:
- Binary format optimized for fast loading
- Contains KV cache state for positions 0..N
- Compatible with llama.cpp session save/load

### Position Tracking

When loading a cached template:
- Template occupies positions 0..N in KV cache
- Messages start at position N
- Generator uses offset to position tokens correctly

## Examples

See `examples/template_caching.rs` for a complete working example demonstrating:
- Cache miss on first session
- Cache hit on subsequent sessions
- Performance measurements and statistics
- Best practices for maximizing cache effectiveness
