# Add rstest file-based tests for Flowchart parser

## Description
The Flowchart parser (`src/parsers/flowchart.rs`) has been implemented but lacks comprehensive file-based testing using rstest. This is critical as flowcharts are marked as the "most comprehensive grammar" in Phase 4 of the implementation roadmap.

## Current State
- Parser implementation: ✅ Complete
- Unit tests in parser file: ✅ Present
- Test data files: ✅ Available in `test/flowchart/` (576 sample files!)
- File-based rstest: ❌ Missing

## Requirements
1. Create `tests/flowchart_test.rs` file
2. Implement rstest file-based testing following established patterns
3. Use `#[rstest]` macro to automatically run tests against all `.mermaid` files in `test/flowchart/`
4. Handle the large test suite (576 files) efficiently
5. Validate complex flowchart elements: nodes, edges, subgraphs, styles

## Implementation Pattern
```rust
use rstest::rstest;
use std::fs;
use std::path::PathBuf;
use mermaid_parser::{parse_diagram, DiagramType};

#[rstest]
fn test_flowchart_files(#[files("test/flowchart/*.mermaid")] path: PathBuf) {
    let content = fs::read_to_string(&path)
        .expect(&format!("Failed to read file: {:?}", path));
    
    let result = parse_diagram(&content);
    assert!(result.is_ok(), "Failed to parse {:?}: {:?}", path, result.err());
    
    if let Ok(DiagramType::Flowchart(ast)) = result {
        // Validate flowchart structures
        // Check node shapes, edge types, labels
        // Verify subgraph hierarchies
        // Validate style applications
    }
}
```

## Special Considerations
- Largest test suite (576 files) requires careful performance consideration
- Most complex grammar with many node shapes and edge types
- Support for subgraphs and nested structures
- Various directional flows (TB, LR, BT, RL)
- Style and class applications

## Success Criteria
- All 576 `.mermaid` files parse without errors
- Tests complete within reasonable time limits
- Complex flowchart features are properly validated
- Performance target: parse 1000+ diagrams in <1 second