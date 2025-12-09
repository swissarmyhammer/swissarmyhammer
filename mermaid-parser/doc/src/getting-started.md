# Getting Started

Welcome to mermaid-parser! This guide will help you get up and running with diagram parsing in minutes.

## Installation

Add mermaid-parser to your project's dependencies:

```toml
[dependencies]
mermaid-parser = "0.1"
```

For the latest development features from the main branch:

```toml
[dependencies]
mermaid-parser = { git = "https://github.com/wballard/mermaid-parser" }
```

## Quick Start

Let's parse your first diagram:

```rust
use mermaid_parser::{parse_diagram, DiagramType};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Define a flowchart diagram
    let diagram_data = r#"
        flowchart TD
            A[Start] --> B{Decision Node}
            B -->|Yes| C[Process A]
            B -->|No| D[Process B]
            C --> E[End]
            D --> E
    "#;

    // Parse the diagram
    match parse_diagram(diagram_data)? {
        DiagramType::Flowchart(flowchart) => {
            println!("✅ Successfully parsed flowchart!");
            println!("📊 Found {} nodes", flowchart.nodes.len());
            println!("🔗 Found {} connections", flowchart.edges.len());
            
            // Analyze the nodes
            for (id, node) in &flowchart.nodes {
                println!("🔍 Node {}: {:?}", id, node.text);
            }
        }
        other_type => {
            println!("🎯 Parsed diagram type: {:?}", other_type);
        }
    }

    Ok(())
}
```

## Core Concepts

### Diagram Type Detection

The parser's neural classification system automatically identifies diagram types:

```rust
use mermaid_parser::{parse_diagram, DiagramType};

// The AI analyzes the input and routes to the appropriate parser
let sankey_input = "sankey-beta\nA,B,10\nB,C,5";
let sequence_input = "sequenceDiagram\nAlice->>Bob: Hello";
let class_input = "classDiagram\nclass Animal";

// Each input activates a different specialized neural pathway
match parse_diagram(sankey_input)? {
    DiagramType::Sankey(diagram) => println!("🌊 Flow diagram activated"),
    _ => {}
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

### Error Handling & Recovery

The neural error recovery system provides intelligent feedback:

```rust
use mermaid_parser::{parse_diagram, ParseError};

let malformed_input = "flowchart TD\n    A => B"; // Wrong arrow syntax

match parse_diagram(malformed_input) {
    Ok(diagram) => println!("✅ Parsing successful"),
    Err(ParseError::EnhancedSyntaxError { message, location, suggestions, .. }) => {
        println!("🔍 Neural analysis detected issue at line {}, column {}", 
                 location.line, location.column);
        println!("💡 Error: {}", message);
        println!("🎯 AI suggestions:");
        for suggestion in suggestions.iter() {
            println!("   • {}", suggestion);
        }
    }
    Err(e) => println!("❌ Parse error: {}", e),
}
```

### AST Analysis

Access the parsed neural network structure:

```rust
use mermaid_parser::{parse_diagram, DiagramType};

let input = "flowchart LR\n    A --> B --> C";
if let Ok(DiagramType::Flowchart(diagram)) = parse_diagram(input) {
    // Navigate the neural structure
    println!("🧠 Neural network contains {} processing nodes", diagram.nodes.len());
    
    // Analyze connections
    for edge in &diagram.edges {
        println!("🔗 Data flow: {} → {}", edge.from, edge.to);
        if let Some(label) = &edge.label {
            println!("   📝 Connection type: {}", label);
        }
    }
}
```

## Supported Diagram Types

The mermaid-parser neural network supports extensive diagram vocabularies:

| Diagram Type | Status | Neural Pathway |
|--------------|--------|----------------|
| Flowchart | ✅ | `flowchart`, `graph` |
| Sequence | ✅ | `sequenceDiagram` |
| Class | ✅ | `classDiagram` |
| State | ✅ | `stateDiagram` |
| Sankey | ✅ | `sankey-beta` |
| Timeline | ✅ | `timeline` |
| Gantt | ✅ | `gantt` |
| Journey | ✅ | `journey` |
| And 15+ more | ✅ | See [Supported Diagrams](./supported-diagrams.md) |

## Next Steps

Now that your neural parsing matrix is initialized, explore these advanced capabilities:

- **[Basic Usage](./basic-usage.md)**: Common parsing patterns and techniques
- **[Error Handling](./error-handling.md)**: Advanced error recovery strategies  
- **[AST Analysis](./ast-analysis.md)**: Deep neural network exploration
- **[Visitor Pattern](./visitor-pattern.md)**: Systematic AST traversal methods
- **[Performance](./performance.md)**: Optimizing your parsing neural network

## Examples Repository

Check out the `examples/` directory for additional neural network configurations:

```bash
# Run basic parsing neural network
cargo run --example basic_parsing

# Activate type detection matrix
cargo run --example detect_type

# Execute batch processing protocols
cargo run --example batch_processing
```

Ready to dive deeper? Let's explore [Basic Usage](./basic-usage.md) patterns!