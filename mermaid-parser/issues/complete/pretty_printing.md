# Implement AST to Mermaid pretty printing

## Description
Add functionality to convert parsed AST back to well-formatted Mermaid syntax. This enables diagram reformatting, transformation, and programmatic diagram generation.

## Requirements
1. Implement Display trait for all AST types
2. Support configurable formatting options
3. Preserve semantic meaning exactly
4. Generate minimal, readable output
5. Support style preservation

## Pretty Printer Design
```rust
pub trait MermaidPrinter {
    fn to_mermaid(&self) -> String;
    fn to_mermaid_pretty(&self, options: &PrintOptions) -> String;
}

pub struct PrintOptions {
    pub indent_width: usize,
    pub max_line_length: usize,
    pub align_arrows: bool,
    pub sort_nodes: bool,
    pub compact_mode: bool,
}

impl MermaidPrinter for DiagramType {
    fn to_mermaid(&self) -> String {
        match self {
            DiagramType::Flowchart(f) => f.to_mermaid(),
            DiagramType::Sequence(s) => s.to_mermaid(),
            // ... all types
        }
    }
}
```

## Formatting Examples
```mermaid
// Input (messy)
flowchart TD
A[Start]-->B{Decision}
B-->|Yes|C[Process]
B-->|No|D[End]
C-->D

// Output (pretty)
flowchart TD
    A[Start] --> B{Decision}
    B -->|Yes| C[Process]
    B -->|No| D[End]
    C --> D
```

## Use Cases
- Code formatting tools
- Diagram normalization
- Programmatic diagram generation
- Diagram transformation tools
- Version control friendly output

## Success Criteria
- Round-trip accuracy (parse→print→parse)
- Readable, consistent output
- Configurable formatting options
- Preserves all diagram features
- Handles edge cases gracefully