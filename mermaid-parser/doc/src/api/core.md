# Core API

The core API provides the main entry point for parsing Mermaid diagrams and working with the resulting Abstract Syntax Trees (ASTs).

## Main Functions

### `parse_diagram`

```rust
pub fn parse_diagram(input: &str) -> Result<DiagramType>
```

The primary function for parsing Mermaid diagrams. This function automatically detects the diagram type and returns the appropriate AST representation.

**Arguments:**
- `input: &str` - The Mermaid diagram text to parse

**Returns:**
- `Result<DiagramType>` - The parsed diagram or a parse error

**Example:**

```rust
use mermaid_parser::{parse_diagram, DiagramType};

let input = r#"
flowchart TD
    A[Start] --> B{Decision}
    B -->|Yes| C[Success]
    B -->|No| D[Failure]
"#;

match parse_diagram(input) {
    Ok(DiagramType::Flowchart(diagram)) => {
        println!("Parsed flowchart with {} nodes", diagram.nodes.len());
    }
    Ok(other) => println!("Parsed different diagram type: {:?}", other),
    Err(e) => eprintln!("Parse error: {}", e),
}
```

## Core Types

### `DiagramType`

An enum representing all supported Mermaid diagram types:

```rust
pub enum DiagramType {
    Sankey(SankeyDiagram),
    Timeline(TimelineDiagram),
    Journey(JourneyDiagram),
    Sequence(SequenceDiagram),
    Class(ClassDiagram),
    State(StateDiagram),
    Flowchart(FlowchartDiagram),
    Gantt(GanttDiagram),
    Pie(PieDiagram),
    // ... and many more
}
```

Each variant contains the specific AST for that diagram type. See the [AST Types](./ast.md) documentation for detailed information about each diagram's structure.

### `Result<T>`

Type alias for parser results:

```rust
pub type Result<T> = std::result::Result<T, ParseError>;
```

This is used throughout the API to provide consistent error handling. See [Error Types](./errors.md) for complete error documentation.

## Usage Patterns

### Basic Parsing

```rust
use mermaid_parser::parse_diagram;

let diagram = parse_diagram("pie title Pets\n    \"Dogs\" : 386\n    \"Cats\" : 85")?;
println!("Parsed diagram: {:?}", diagram);
```

### Type-Specific Handling

```rust
use mermaid_parser::{parse_diagram, DiagramType};

match parse_diagram(input)? {
    DiagramType::Sankey(sankey) => {
        for link in &sankey.links {
            println!("Flow: {} -> {} ({})", link.source, link.target, link.value);
        }
    }
    DiagramType::Sequence(sequence) => {
        for message in &sequence.messages {
            println!("Message: {} -> {}", message.from, message.to);
        }
    }
    other => println!("Unsupported diagram type for this analysis: {:?}", other),
}
```

### Error Handling

```rust
use mermaid_parser::{parse_diagram, ParseError};

match parse_diagram("invalid input") {
    Ok(diagram) => process_diagram(diagram),
    Err(ParseError::EmptyInput) => println!("Please provide diagram content"),
    Err(ParseError::SyntaxError { message, line, column, .. }) => {
        println!("Syntax error at {}:{}: {}", line, column, message);
    }
    Err(e) => println!("Parse failed: {}", e),
}
```

## Performance Considerations

- The parser uses zero-copy parsing where possible for memory efficiency
- Diagram type detection is O(1) after finding the first meaningful line
- Large diagrams (>1MB) may benefit from preprocessing
- See [Performance](../performance.md) for detailed benchmarks and optimization tips
