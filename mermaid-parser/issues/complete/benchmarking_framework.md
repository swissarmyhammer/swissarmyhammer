# Setup performance benchmarking framework

## Description
Implement a benchmarking framework to measure parser performance and ensure it meets the goal of parsing 1000+ diagrams in under 1 second. This is mentioned as a key performance target in PROJECT_STATUS.md.

## Requirements
1. Add benchmark dependencies to Cargo.toml (criterion)
2. Create `benches/` directory
3. Implement benchmarks for each parser
4. Measure parsing performance on various diagram sizes
5. Create performance regression tests

## Benchmark Areas
- Individual parser performance
- Auto-detection overhead
- Large diagram handling (576 flowchart samples)
- Memory usage profiling
- Parallel parsing capabilities

## Implementation Structure
```rust
// benches/parser_benchmarks.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use mermaid_parser::parse_diagram;

fn benchmark_flowchart(c: &mut Criterion) {
    let simple = "flowchart TD\n    A --> B";
    let complex = // Load large flowchart
    
    c.bench_function("flowchart_simple", |b| {
        b.iter(|| parse_diagram(black_box(simple)))
    });
    
    c.bench_function("flowchart_complex", |b| {
        b.iter(|| parse_diagram(black_box(&complex)))
    });
}

fn benchmark_batch_parsing(c: &mut Criterion) {
    // Benchmark parsing 1000+ diagrams
}
```

## Success Criteria
- Benchmarks run with `cargo bench`
- Performance meets or exceeds targets
- Results tracked over time
- Memory usage stays within reasonable bounds
- Identifies performance bottlenecks