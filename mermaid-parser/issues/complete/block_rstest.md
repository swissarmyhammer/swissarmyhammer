# Add rstest file-based tests for Block parser

## Description
The Block parser (`src/parsers/block.rs`) has been implemented but lacks comprehensive file-based testing using rstest. This is required per the original specification to validate the parser against real Mermaid diagram examples.

## Current State
- Parser implementation: ✅ Complete
- Unit tests in parser file: ✅ Present
- Test data files: ✅ Available in `test/block/`
- File-based rstest: ❌ Missing

## Requirements
1. Create `tests/block_test.rs` file
2. Implement rstest file-based testing following the pattern used in other test files (e.g., `tests/er_test.rs`)
3. Use `#[rstest]` macro to automatically run tests against all `.mermaid` files in `test/block/`
4. Ensure all test files parse successfully
5. Validate the AST structure matches expected block diagram elements (blocks, connections, labels)

## Implementation Pattern
```rust
use rstest::rstest;
use std::fs;
use std::path::PathBuf;
use mermaid_parser::{parse_diagram, DiagramType};

#[rstest]
fn test_block_files(#[files("test/block/*.mermaid")] path: PathBuf) {
    let content = fs::read_to_string(&path)
        .expect(&format!("Failed to read file: {:?}", path));
    
    let result = parse_diagram(&content);
    assert!(result.is_ok(), "Failed to parse {:?}: {:?}", path, result.err());
    
    if let Ok(DiagramType::Block(ast)) = result {
        // Validate basic AST structure
        // Ensure blocks and connections are present
    }
}
```

## Success Criteria
- All `.mermaid` files in `test/block/` parse without errors
- Test validates block structures and connections
- Clear error messages for parsing failures