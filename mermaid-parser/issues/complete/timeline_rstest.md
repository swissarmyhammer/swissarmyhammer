# Add rstest file-based tests for Timeline parser

## Description
The Timeline parser (`src/parsers/timeline.rs`) has been implemented but lacks comprehensive file-based testing using rstest. This is marked as Priority 2 in Phase 1, with structured content and sections.

## Current State
- Parser implementation: ✅ Complete (recently implemented per commit history)
- Unit tests in parser file: ✅ Present
- Test data files: ✅ Available in `test/timeline/` (25 sample files)
- File-based rstest: ❌ Missing

## Requirements
1. Create `tests/timeline_test.rs` file
2. Implement rstest file-based testing following established patterns
3. Use `#[rstest]` macro to automatically run tests against all `.mermaid` files in `test/timeline/`
4. Validate timeline-specific structures: sections, events, periods
5. Ensure chronological ordering is preserved

## Implementation Pattern
```rust
use rstest::rstest;
use std::fs;
use std::path::PathBuf;
use mermaid_parser::{parse_diagram, DiagramType};

#[rstest]
fn test_timeline_files(#[files("test/timeline/*.mermaid")] path: PathBuf) {
    let content = fs::read_to_string(&path)
        .expect(&format!("Failed to read file: {:?}", path));
    
    let result = parse_diagram(&content);
    assert!(result.is_ok(), "Failed to parse {:?}: {:?}", path, result.err());
    
    if let Ok(DiagramType::Timeline(ast)) = result {
        // Validate timeline structure
        // Check sections and events
        // Verify chronological relationships
    }
}
```

## Special Considerations
- Priority 2 implementation - important for early validation
- Structured content with sections and events
- May include various time formats and periods
- Recent implementation needs thorough testing

## Success Criteria
- All 25 `.mermaid` files in `test/timeline/` parse without errors
- Timeline structure properly validated
- Sections and events correctly parsed
- Clear error messages for invalid timelines