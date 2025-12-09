# Create usage examples directory

## Description
Create an examples directory with practical usage examples for the mermaid-parser library. This helps users quickly understand how to integrate the parser into their projects.

## Requirements
1. Create `examples/` directory
2. Add example programs for common use cases
3. Include examples for each diagram type
4. Show error handling patterns
5. Demonstrate AST traversal and analysis

## Example Programs to Create
- `examples/basic_parsing.rs` - Simple parsing example
- `examples/detect_type.rs` - Auto-detection of diagram types
- `examples/parse_all_types.rs` - Parsing different diagram types
- `examples/error_handling.rs` - Handling parse errors gracefully
- `examples/ast_analysis.rs` - Analyzing parsed AST structures
- `examples/batch_processing.rs` - Processing multiple files
- `examples/validation.rs` - Validating diagram semantics

## Example Structure
```rust
// examples/basic_parsing.rs
use mermaid_parser::{parse_diagram, DiagramType};
use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let content = fs::read_to_string("diagram.mermaid")?;
    
    match parse_diagram(&content)? {
        DiagramType::Flowchart(ast) => {
            println!("Parsed flowchart with {} nodes", ast.nodes.len());
        }
        // Handle other types...
    }
    
    Ok(())
}
```

## Success Criteria
- Examples can be run with `cargo run --example <name>`
- Each example is self-contained and educational
- Examples cover common real-world use cases
- Code is well-commented and easy to understand