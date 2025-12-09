# Parser Documentation

This section provides detailed documentation for each diagram parser implemented in mermaid-parser. Each parser is specialized for a specific diagram type and optimized for its unique syntax patterns.

## Parser Architecture

All parsers in mermaid-parser follow a consistent architecture:

1. **Lexical Analysis**: Tokenize the input text into meaningful tokens
2. **Syntax Parsing**: Build an Abstract Syntax Tree (AST) using parser combinators
3. **Semantic Validation**: Verify the logical correctness of the parsed diagram
4. **Error Recovery**: Provide helpful error messages and recovery strategies

## Available Parsers

### Core Diagram Types

- **[Sankey Diagrams](./sankey.md)**: Flow data visualization with weighted connections
- **[Flowcharts](./flowchart.md)**: General-purpose flow diagrams with nodes and edges
- **[Sequence Diagrams](./sequence.md)**: Message passing between participants over time
- **[Class Diagrams](./class.md)**: Object-oriented class relationships and hierarchies
- **[State Diagrams](./state.md)**: State machine representations with transitions

### Specialized Parsers

- **[Gantt Charts](./gantt.md)**: Project timeline and task scheduling
- **[Timeline Diagrams](./timeline.md)**: Chronological event sequences
- **[Other Parsers](./others.md)**: Documentation for additional diagram types

## Parser Selection

The main [`parse_diagram`](../api/core.md#parse_diagram) function automatically detects the diagram type and routes to the appropriate parser. This is done through pattern matching on the first meaningful line of the input.

## Adding New Parsers

For information on implementing new parsers, see the [Contributing](../contributing.md) guide and [Parser Architecture](../parser-architecture.md) documentation.