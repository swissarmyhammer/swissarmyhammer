# Performance Considerations

The mermaid-parser is designed with performance in mind, using efficient parsing techniques and memory management strategies.

## Design Principles

### Parser Combinator Efficiency

The crate uses the [Chumsky](https://github.com/zesterer/chumsky) parser combinator library, which provides:

- **Zero-copy parsing** where possible
- **Streaming support** for large inputs
- **Backtracking optimization** to minimize redundant work
- **Error recovery** without complete re-parsing

### Memory Management

- **Minimal allocations** during parsing through string slicing
- **Structured AST representation** avoiding deep nesting where possible
- **Visitor pattern** for efficient tree traversal without copying

### Diagram Type Detection

The parser uses a fast diagram type detection algorithm:

```rust
// Fast prefix matching - O(1) lookup after initial line scan
match first_word.as_str() {
    "sankey-beta" => Ok("sankey"),
    "flowchart" | "graph" => Ok("flowchart"),
    "timeline" => Ok("timeline"),
    // ... other patterns
}
```

## Performance Characteristics

### Time Complexity

- **Diagram detection**: O(n) where n is characters until first meaningful line
- **Parsing**: Generally O(n) for most diagram types, where n is input size
- **AST traversal**: O(m) where m is number of AST nodes

### Space Complexity

- **Memory usage**: O(n) proportional to input size and AST complexity
- **Stack usage**: Bounded by maximum nesting depth in diagrams

## Optimization Strategies

### For Large Diagrams

```rust
use mermaid_parser::parse_diagram;

// For very large diagrams, consider pre-validation
fn parse_large_diagram(input: &str) -> Result<DiagramType, String> {
    // Quick size check
    if input.len() > 1_000_000 {
        eprintln!("Warning: Large diagram ({}MB)", input.len() / 1_000_000);
    }
    
    parse_diagram(input)
        .map_err(|e| format!("Parse error: {}", e))
}
```

### For Batch Processing

```rust
use mermaid_parser::parse_diagram;

fn parse_multiple_diagrams(inputs: &[&str]) -> Vec<Result<DiagramType, String>> {
    inputs.iter()
        .map(|input| parse_diagram(input).map_err(|e| e.to_string()))
        .collect()
}
```

### Memory-Efficient Analysis

```rust
use mermaid_parser::{parse_diagram, common::visitor::{AstVisitor, NodeCounter}};

fn count_nodes_efficiently(input: &str) -> Result<usize, String> {
    let diagram = parse_diagram(input)
        .map_err(|e| e.to_string())?;
    
    let mut counter = NodeCounter::new();
    diagram.accept(&mut counter);
    
    Ok(counter.nodes())
}
```

## Benchmarking

The crate includes benchmark suites in `benches/parser_benchmarks.rs`:

```rust
// Run benchmarks with:
// cargo bench
```

Benchmarks cover:
- **Diagram type detection** across all supported types
- **Parsing speed** for various diagram sizes
- **Memory allocation patterns** during parsing
- **AST traversal performance** with visitor patterns

## Performance Tips

### Input Preparation

- **Remove unnecessary whitespace** if memory is constrained
- **Validate input format** before parsing complex diagrams
- **Use streaming** for very large inputs when possible

### Error Handling

- **Fast-fail validation** for obviously invalid inputs
- **Structured error handling** to avoid string formatting overhead
- **Error caching** for repeated parsing attempts

### Memory Management

```rust
// Efficient pattern for repeated parsing
fn parse_and_analyze(inputs: &[String]) -> Vec<usize> {
    let mut results = Vec::with_capacity(inputs.len());
    
    for input in inputs {
        if let Ok(diagram) = parse_diagram(input) {
            let mut counter = NodeCounter::new();
            diagram.accept(&mut counter);
            results.push(counter.nodes());
        }
    }
    
    results
}
```

## Limitations

- **Complex nested structures** may require more memory
- **Error recovery** has overhead compared to fail-fast parsing
- **Large diagrams** (>10MB) may benefit from streaming approaches

## Profiling

Use standard Rust profiling tools:

```bash
# CPU profiling
cargo bench --bench parser_benchmarks

# Memory profiling
valgrind --tool=massif target/release/examples/basic_parsing

# Flamegraph generation
cargo flamegraph --example basic_parsing
```

The parser is optimized for typical use cases with diagrams under 1MB, providing efficient parsing for most real-world scenarios.
