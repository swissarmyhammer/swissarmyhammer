# Mermaid Parser

📚 **[Complete Documentation](https://wballard.github.io/mermaid-parser/)** 🚀 

[![Crates.io](https://img.shields.io/crates/v/mermaid-parser.svg)](https://crates.io/crates/mermaid-parser)
[![Documentation](https://docs.rs/mermaid-parser/badge.svg)](https://docs.rs/mermaid-parser)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/wballard/mermaid-parser)

A fast, reliable parser for [Mermaid](https://mermaid.js.org/) diagram syntax written in Rust. This crate provides comprehensive support for parsing various Mermaid diagram types into structured AST representations.

## Features

- 🚀 **Fast parsing** using the [Chumsky](https://github.com/zesterer/chumsky) parser combinator library
- 📊 **Comprehensive diagram support** for all major Mermaid diagram types
- 🔍 **Detailed error reporting** with source location information and helpful suggestions
- 🧪 **Thoroughly tested** with high code coverage and property-based testing
- 📖 **Well-documented** with extensive examples and API documentation
- 🔧 **Flexible AST** with visitor pattern support for analysis and transformation
- ⚡ **High performance** optimized for speed and memory efficiency
- 🛡️ **Robust error handling** with recovery strategies and clear error messages

## Supported Diagram Types

- **Flowcharts** - Flow diagrams with nodes and edges
- **Sequence diagrams** - Message flows between participants
- **Class diagrams** - Object-oriented class relationships
- **State diagrams** - State machines and transitions
- **Gantt charts** - Project timeline visualization
- **Pie charts** - Data distribution visualization
- **Journey maps** - User experience flows
- **Git graphs** - Version control branching visualization
- **Entity-relationship diagrams** - Database schema representation
- **C4 diagrams** - Software architecture visualization
- **Mindmaps** - Hierarchical information structure
- **Timeline diagrams** - Chronological event representation
- **Sankey diagrams** - Flow quantity visualization
- **XY charts** - Data plotting with multiple series
- **Quadrant charts** - Four-quadrant analysis
- **Requirement diagrams** - Requirements and relationships
- **Gitgraph diagrams** - Git branching workflows
- **Block diagrams** - Block-based representations
- **Packet diagrams** - Network packet visualization
- **Architecture diagrams** - System architecture layouts
- **Treemap diagrams** - Hierarchical data visualization
- **Kanban boards** - Task management workflows
- **Radar charts** - Multi-dimensional data comparison

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
mermaid-parser = "0.1"
```

## Quick Start

```rust
use mermaid_parser::{parse_diagram, DiagramType};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = r#"
        flowchart TD
            A[Start] --> B{Is it working?}
            B -->|Yes| C[Great!]
            B -->|No| D[Fix it]
            D --> B
    "#;

    match parse_diagram(input) {
        Ok(DiagramType::Flowchart(diagram)) => {
            println!("Parsed flowchart with {} nodes", diagram.nodes.len());
            for node in &diagram.nodes {
                println!("Node: {}", node.id);
            }
        }
        Ok(other) => println!("Parsed diagram: {:?}", other),
        Err(e) => eprintln!("Parse error: {}", e),
    }

    Ok(())
}
```

## Examples

### Parsing Different Diagram Types

```rust
use mermaid_parser::{parse_diagram, DiagramType};

// Parse a sequence diagram
let sequence = r#"
    sequenceDiagram
        participant A as Alice
        participant B as Bob
        A->>B: Hello Bob!
        B-->>A: Hello Alice!
"#;

if let Ok(DiagramType::Sequence(diagram)) = parse_diagram(sequence) {
    println!("Found {} participants", diagram.participants.len());
}

// Parse a class diagram
let class = r#"
    classDiagram
        class Animal {
            +String name
            +int age
            +makeSound()
        }
        class Dog {
            +String breed
            +bark()
        }
        Animal <|-- Dog
"#;

if let Ok(DiagramType::Class(diagram)) = parse_diagram(class) {
    println!("Found {} classes", diagram.classes.len());
}
```

### Error Handling

```rust
use mermaid_parser::{parse_diagram, ParseError};

let invalid_input = "flowchart TD\n    A --> ";

match parse_diagram(invalid_input) {
    Ok(diagram) => println!("Parsed successfully: {:?}", diagram),
    Err(ParseError::SyntaxError { message, expected, found, line, column }) => {
        eprintln!("Parse error at line {}, column {}: {}", line, column, message);
        eprintln!("Expected one of: {:?}, found: {}", expected, found);
    }
    Err(ParseError::EmptyInput) => {
        eprintln!("Input was empty or contained no valid diagram content");
    }
    Err(e) => eprintln!("Other error: {}", e),
}
```

### Using the Visitor Pattern

```rust
use mermaid_parser::{parse_diagram, DiagramType};
use mermaid_parser::common::visitor::{AstVisitor, NodeCounter};

let input = "flowchart TD\n    A --> B\n    B --> C\n    C --> D";

if let Ok(diagram) = parse_diagram(input) {
    let mut counter = NodeCounter::new();
    diagram.accept(&mut counter);
    println!("Diagram statistics: {} nodes, {} edges", 
             counter.nodes(), counter.edges());
}
```

## Testing

Run the test suite:

```bash
cargo test
```

Run tests with coverage:

```bash
cargo install cargo-llvm-cov
cargo llvm-cov --html --open
```

Run property-based tests:

```bash
cargo test --test property_tests
```

## Benchmarks

Run performance benchmarks:

```bash
cargo bench
```

The parser is designed for performance using efficient parsing techniques and memory management.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request. For major changes, please open an issue first to discuss what you would like to change.

### Development Setup

1. Clone the repository
2. Install Rust (1.70.0 or later)
3. Run `cargo test` to ensure everything works
4. Make your changes
5. Run `cargo fmt` and `cargo clippy`
6. Add tests for new functionality
7. Submit a pull request

## License

This project is licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for a detailed list of changes in each version.

## Related Projects

- [Mermaid.js](https://mermaid.js.org/) - The original JavaScript implementation
- [mermaid-cli](https://github.com/mermaid-js/mermaid-cli) - Command line interface for Mermaid

## Acknowledgments

This parser is built using the excellent [Chumsky](https://github.com/zesterer/chumsky) parser combinator library.