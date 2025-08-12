# Performance Tuning

Optimize SwissArmyHammer for speed, memory usage, and scalability in different environments.

## Performance Overview

SwissArmyHammer is designed for performance across several dimensions:

- **Startup Time**: Fast initialization for CLI commands
- **Memory Usage**: Efficient memory management for large codebases  
- **I/O Performance**: Optimized file system operations
- **Search Speed**: Fast semantic search with vector databases
- **Template Rendering**: Efficient Liquid template processing
- **Concurrent Operations**: Parallel execution where beneficial

## Benchmarking

### Built-in Benchmarks

SwissArmyHammer includes comprehensive benchmarks:

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark suites
cargo bench search
cargo bench templates
cargo bench workflows

# Compare with baseline
cargo bench -- --save-baseline main
git checkout feature-branch
cargo bench -- --baseline main
```

### Profiling Tools

#### CPU Profiling

```bash
# Install profiling tools
cargo install cargo-flamegraph

# Profile a specific command
cargo flamegraph --bin sah -- search query "error handling"

# Profile with perf (Linux)
perf record --call-graph=dwarf cargo run --bin sah -- search index "**/*.rs"
perf report
```

#### Memory Profiling

```bash
# Install memory profilers
cargo install cargo-profdata

# Profile memory usage
valgrind --tool=massif cargo run --bin sah -- search index "**/*.rs"
ms_print massif.out.12345

# Use heaptrack (Linux)
heaptrack cargo run --bin sah -- search index "**/*.rs"
heaptrack_gui heaptrack.sah.12345.gz
```

## Configuration Tuning

### General Performance Settings

```toml
# ~/.swissarmyhammer/sah.toml

[general]
# Disable auto-reload for better performance
auto_reload = false

# Increase timeout for large operations
default_timeout_ms = 60000

[template]
# Increase cache size for frequently used templates
cache_size = 2000

# Disable template recompilation in production
recompile_templates = false

[workflow]
# Increase parallel action limit for powerful machines
max_parallel_actions = 8

# Enable workflow caching
enable_caching = true
cache_dir = "/tmp/sah-workflow-cache"

[search]
# Use faster but larger embedding model
embedding_model = "nomic-embed-code"

# Increase memory limits for large indexes
max_memory_mb = 2048

# Optimize index for read performance
index_compression = false
```

### Memory Optimization

```toml
[security]
# Reduce memory limits for resource-constrained environments
max_memory_mb = 256
max_disk_usage_mb = 1024

[search]
# Limit file size for indexing
max_file_size = 524288  # 512KB

# Reduce embedding dimensions for smaller memory footprint
embedding_dimensions = 384  # vs 768 default

[template]
# Smaller template cache
cache_size = 100

# Aggressive cache eviction
cache_ttl_ms = 300000  # 5 minutes
```

### I/O Optimization

```toml
[general]
# Use faster file watching (when available)
file_watcher = "polling"  # or "native"

# Batch file operations
batch_size = 100

[search]
# Use faster storage backend
storage_backend = "memory"  # for small indexes
# storage_backend = "disk"   # for large indexes

# Enable compression for large indexes
enable_compression = true

# Use faster hash function
hash_algorithm = "xxhash"
```

## Search Performance

### Indexing Optimization

#### Selective Indexing

```bash
# Index only important directories
sah search index "src/**/*.{rs,py,js}" --exclude "**/target/**"

# Avoid large generated files
sah search index "**/*.rs" \
  --exclude "**/target/**" \
  --exclude "**/node_modules/**" \
  --exclude "**/*.generated.*"

# Set file size limits
sah search index "**/*.rs" --max-size 1048576  # 1MB limit
```

#### Parallel Indexing

```toml
[search]
# Enable parallel file processing
parallel_indexing = true
indexing_threads = 4

# Batch processing for better throughput
batch_size = 50

# Use memory mapping for large files
use_mmap = true
```

#### Incremental Indexing

```bash
# Only index changed files (much faster)
sah search index "**/*.rs"  # Skips unchanged files automatically

# Force full reindex only when needed
sah search index "**/*.rs" --force
```

### Query Optimization

#### Efficient Queries

```bash
# Use specific, focused queries
sah search query "async function error handling" --limit 5

# Adjust similarity threshold for faster results
sah search query "database connection" --threshold 0.7

# Use exact matches when possible
sah search query "fn main()" --threshold 0.9
```

#### Query Caching

```toml
[search]
# Enable query result caching
cache_results = true
result_cache_size = 1000
result_cache_ttl_ms = 300000  # 5 minutes

# Cache embeddings for repeated queries
cache_embeddings = true
embedding_cache_size = 10000
```

## Template Performance

### Template Optimization

#### Efficient Template Design

```liquid
{% comment %}Good: Filter once, use multiple times{% endcomment %}
{% assign active_users = users | where: "active", true %}
Active users: {{active_users | size}}
Names: {{active_users | map: "name" | join: ", "}}

{% comment %}Avoid: Repeated filtering{% endcomment %}
Active users: {{users | where: "active", true | size}}
Names: {{users | where: "active", true | map: "name" | join: ", "}}
```

#### Loop Optimization

```liquid
{% comment %}Good: Early termination{% endcomment %}
{% for item in items limit:10 %}
  {% if item.important %}
    {{item.name}}
    {% break %}
  {% endif %}
{% endfor %}

{% comment %}Good: Batch operations{% endcomment %}
{% assign important_items = items | where: "important", true %}
{% for item in important_items limit:10 %}
  {{item.name}}
{% endfor %}
```

#### Template Caching

```toml
[template]
# Aggressive caching for production
cache_size = 5000
cache_compiled_templates = true

# Pre-compile frequently used templates
precompile_templates = [
  "code-review",
  "documentation", 
  "test-generator"
]
```

### Variable Management

```liquid
{% comment %}Cache expensive computations{% endcomment %}
{% assign file_count = files | size %}
{% if file_count > 0 %}
  Processing {{file_count}} files...
  {% for file in files %}
    File: {{file.name}} ({{forloop.index}}/{{file_count}})
  {% endfor %}
{% endif %}
```

## Workflow Performance

### Parallel Execution

```toml
[workflow]
# Optimize for CPU cores
max_parallel_actions = 8

# Enable fork-join optimization
optimize_forks = true

# Use async execution where possible
prefer_async = true
```

### Action Optimization

#### Shell Actions

```markdown
**Actions:**
# Good: Combine related commands
- shell: `cargo build && cargo test --lib` (timeout: 300s)

# Avoid: Separate slow commands
- shell: `cargo build` (timeout: 120s)
- shell: `cargo test --lib` (timeout: 180s)
```

#### Prompt Actions

```markdown
**Actions:**
# Good: Batch similar prompts
- prompt: multi-analyzer files="$(find . -name '*.rs' | head -10)" analysis_type="comprehensive"

# Avoid: Individual file analysis
- prompt: code-reviewer file="src/main.rs"
- prompt: code-reviewer file="src/lib.rs"
```

### State Machine Optimization

```markdown
# Good: Minimize state transitions
### build-and-test
**Actions:**
- shell: `cargo build --release`
- shell: `cargo test --release`
**Transitions:**
- On success → deploy
- On failure → failed

# Avoid: Too many small states
### build
**Actions:**
- shell: `cargo build --release`
**Transitions:**
- Always → test

### test  
**Actions:**
- shell: `cargo test --release`
**Transitions:**
- On success → deploy
```

## System-Level Optimization

### File System Performance

#### SSD Optimization

```bash
# Use SSD for search database
mkdir -p /mnt/ssd/sah-cache
sah config set search.index_path "/mnt/ssd/sah-cache/search.db"

# Use tmpfs for temporary operations
mkdir -p /tmp/sah-temp
sah config set workflow.temp_dir "/tmp/sah-temp"
```

#### Network File Systems

```toml
[general]
# Reduce file watching on network filesystems
auto_reload = false

# Use local cache
local_cache_dir = "/tmp/sah-cache"

[search]
# Cache index locally
local_index_cache = true
cache_dir = "/tmp/sah-search-cache"
```

### Memory Management

#### Large Scale Operations

```bash
# For large codebases, use streaming operations
export SAH_STREAMING_MODE=true
export SAH_MAX_MEMORY=4G

# Process in batches
sah search index "**/*.rs" --batch-size 100

# Use disk-based sorting for large datasets
export SAH_USE_DISK_SORT=true
```

#### Memory-Constrained Environments

```toml
[search]
# Use smaller embedding model
embedding_model = "all-MiniLM-L6-v2"  # 384 dimensions vs 768

# Reduce cache sizes
embedding_cache_size = 1000
result_cache_size = 100

# Enable aggressive garbage collection
gc_threshold = 1000
```

### CPU Optimization

#### Multi-core Systems

```toml
[general]
# Use all available cores
worker_threads = 0  # Auto-detect

[search]
# Parallel indexing
indexing_threads = 8
search_threads = 4

[workflow]
# Parallel action execution
max_parallel_actions = 16
```

#### Single-core Systems

```toml
[general]
# Minimize threading overhead
worker_threads = 1

[search]
# Sequential processing
indexing_threads = 1
search_threads = 1

[workflow]
# Sequential execution
max_parallel_actions = 1
```

## Monitoring and Profiling

### Runtime Metrics

```bash
# Enable detailed timing
export SAH_ENABLE_TIMING=true
export SAH_LOG_LEVEL=debug

# Monitor with built-in metrics
sah doctor --check performance

# Profile specific operations
time sah search query "error handling"
time sah prompt test code-reviewer --var file=src/main.rs
```

### Performance Monitoring

```toml
[logging]
# Enable performance logging
enable_timing = true
log_slow_operations = true
slow_operation_threshold_ms = 1000

[metrics]
# Export metrics for monitoring
enable_metrics = true
metrics_port = 9090
metrics_endpoint = "/metrics"
```

### Continuous Performance Testing

```bash
# Add performance tests to CI
#!/bin/bash
# performance-test.sh

# Index performance
time_start=$(date +%s%N)
sah search index "**/*.rs" --force >/dev/null 2>&1
time_end=$(date +%s%N)
index_time=$(( (time_end - time_start) / 1000000 ))

echo "Index time: ${index_time}ms"

# Query performance
time_start=$(date +%s%N)
sah search query "async function" >/dev/null 2>&1
time_end=$(date +%s%N)
query_time=$(( (time_end - time_start) / 1000000 ))

echo "Query time: ${query_time}ms"

# Fail if performance regression
if [ $index_time -gt 30000 ]; then
    echo "Index performance regression!"
    exit 1
fi

if [ $query_time -gt 1000 ]; then
    echo "Query performance regression!"
    exit 1
fi
```

## Performance Troubleshooting

### Common Issues

#### Slow Startup

```bash
# Check file system performance
time ls -la ~/.swissarmyhammer/

# Disable auto-reload
sah config set general.auto_reload false

# Clear caches
rm -rf ~/.swissarmyhammer/cache/
```

#### High Memory Usage

```bash
# Monitor memory usage
ps aux | grep sah
pmap $(pidof sah)

# Reduce cache sizes
sah config set template.cache_size 100
sah config set search.embedding_cache_size 1000

# Enable streaming mode
export SAH_STREAMING_MODE=true
```

#### Slow Search Performance

```bash
# Check index size
ls -lh ~/.swissarmyhammer/search.db

# Rebuild index with optimizations
sah search index "**/*.rs" --force --optimize

# Use smaller embedding model
sah config set search.embedding_model "all-MiniLM-L6-v2"
```

By applying these performance tuning techniques, SwissArmyHammer can be optimized for various environments and use cases, from resource-constrained development machines to high-performance CI/CD servers.