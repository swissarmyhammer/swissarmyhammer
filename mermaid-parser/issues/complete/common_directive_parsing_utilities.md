# Common Directive Parsing Utilities

**Priority**: Medium  
**Impact**: 10+ files affected  
**Effort**: Medium  

## Problem

Many parsers implement identical logic for parsing common directives like `title`, `accTitle`, and `accDescr`. This creates code duplication and inconsistent handling across diagram types.

### Current Duplicated Pattern

Found in `architecture.rs`, `flowchart.rs`, `sequence.rs`, `gantt.rs`, `pie.rs`, `timeline.rs`, `quadrant.rs`, and 5+ other parser files:

```rust
// Title parsing
if trimmed.starts_with("title ") {
    let title = trimmed.strip_prefix("title ").unwrap().trim();
    diagram.title = Some(title.to_string());
    continue;
}

// Accessibility title
if trimmed.starts_with("accTitle: ") {
    diagram.accessibility.title = Some(
        trimmed.strip_prefix("accTitle: ").unwrap().trim().to_string()
    );
    continue;
}

// Accessibility description  
if trimmed.starts_with("accDescr: ") {
    diagram.accessibility.description = Some(
        trimmed.strip_prefix("accDescr: ").unwrap().trim().to_string()
    );
    continue;
}
```

### Variations Found

Some parsers also handle:
- Multi-line accessibility descriptions with `accDescr {` and `}`
- Different title formats
- Comment variations

## Solution

Create shared directive parsing utilities in `src/common/parser_utils.rs`:

```rust
/// Parses common diagram directives (title, accTitle, accDescr)
/// Returns true if the line was handled, false if not recognized
pub fn parse_common_directives(
    line: &str,
    title: &mut Option<String>,
    accessibility: &mut Accessibility,
) -> bool {
    let trimmed = line.trim();
    
    // Parse title directive
    if let Some(title_text) = trimmed.strip_prefix("title ") {
        *title = Some(title_text.trim().to_string());
        return true;
    }
    
    // Parse accessibility title
    if let Some(acc_title) = trimmed.strip_prefix("accTitle: ") {
        accessibility.title = Some(acc_title.trim().to_string());
        return true;
    }
    
    // Parse accessibility description
    if let Some(acc_desc) = trimmed.strip_prefix("accDescr: ") {
        accessibility.description = Some(acc_desc.trim().to_string());
        return true;
    }
    
    false
}

/// Enhanced version that handles multi-line accessibility descriptions
pub struct CommonDirectiveParser {
    in_multiline_desc: bool,
    multiline_content: Vec<String>,
}

impl CommonDirectiveParser {
    pub fn new() -> Self {
        Self {
            in_multiline_desc: false,
            multiline_content: Vec::new(),
        }
    }
    
    /// Parse common directives with multi-line support
    /// Returns true if line was handled
    pub fn parse_line(
        &mut self,
        line: &str,
        title: &mut Option<String>,
        accessibility: &mut Accessibility,
    ) -> bool {
        let trimmed = line.trim();
        
        // Handle multi-line accessibility description end
        if self.in_multiline_desc && trimmed == "}" {
            self.in_multiline_desc = false;
            if !self.multiline_content.is_empty() {
                accessibility.description = Some(self.multiline_content.join(" "));
                self.multiline_content.clear();
            }
            return true;
        }
        
        // Handle multi-line accessibility description content
        if self.in_multiline_desc {
            if !trimmed.is_empty() && !trimmed.starts_with("//") && !trimmed.starts_with("%%") {
                self.multiline_content.push(trimmed.to_string());
            }
            return true;
        }
        
        // Handle multi-line accessibility description start
        if trimmed.starts_with("accDescr {") {
            self.in_multiline_desc = true;
            return true;
        }
        
        // Handle single-line directives
        parse_common_directives(line, title, accessibility)
    }
}
```

### Usage in Parsers

```rust
// Simple usage for parsers without multi-line support
if parse_common_directives(line, &mut diagram.title, &mut diagram.accessibility) {
    continue;
}

// Enhanced usage for parsers with multi-line support
if common_parser.parse_line(line, &mut diagram.title, &mut diagram.accessibility) {
    continue;
}
```

## Implementation Steps

1. **Add utility functions to `src/common/parser_utils.rs`**
   - `parse_common_directives()`
   - `CommonDirectiveParser` struct and implementation

2. **Update parser files** (in order of complexity):
   - **Simple directives only**:
     - `src/parsers/architecture.rs`
     - `src/parsers/block.rs`
     - `src/parsers/packet.rs`
     - `src/parsers/quadrant.rs`
     - `src/parsers/radar.rs`
     - `src/parsers/requirement.rs`
     - `src/parsers/treemap.rs`
   
   - **Multi-line directive support**:
     - `src/parsers/pie.rs`
     - `src/parsers/timeline.rs`
     - `src/parsers/journey.rs`

3. **Add comprehensive tests**
   - Test single-line directives
   - Test multi-line accessibility descriptions
   - Test edge cases and malformed input

4. **Verify behavior consistency** across all parsers

## Benefits

- **Reduced duplication**: Eliminates ~15 lines of code per parser (150+ lines total)
- **Consistent behavior**: All parsers handle directives identically
- **Enhanced features**: Easy to add new common directives
- **Better accessibility**: Standardized accessibility parsing
- **Maintainability**: Changes to directive parsing logic in one place

## Detailed Implementation

### Core Utility Functions

```rust
// src/common/parser_utils.rs

/// Standard accessibility information for diagrams
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Accessibility {
    pub title: Option<String>,
    pub description: Option<String>,
}

/// Result of parsing a common directive
#[derive(Debug, PartialEq)]
pub enum DirectiveResult {
    /// Line was not a recognized directive
    NotRecognized,
    /// Line was handled successfully
    Handled,
    /// Line starts a multi-line block
    StartMultiLine,
    /// Line ends a multi-line block
    EndMultiLine,
}

/// Parse a single line for common directives
pub fn parse_directive_line(line: &str) -> Option<(DirectiveType, String)> {
    let trimmed = line.trim();
    
    if let Some(content) = trimmed.strip_prefix("title ") {
        return Some((DirectiveType::Title, content.trim().to_string()));
    }
    
    if let Some(content) = trimmed.strip_prefix("accTitle: ") {
        return Some((DirectiveType::AccTitle, content.trim().to_string()));
    }
    
    if let Some(content) = trimmed.strip_prefix("accDescr: ") {
        return Some((DirectiveType::AccDesc, content.trim().to_string()));
    }
    
    if trimmed == "accDescr {" {
        return Some((DirectiveType::AccDescStart, String::new()));
    }
    
    None
}

#[derive(Debug, PartialEq)]
pub enum DirectiveType {
    Title,
    AccTitle,
    AccDesc,
    AccDescStart,
}
```

### Enhanced Parser Integration

```rust
// In each parser file
use crate::common::parser_utils::{parse_common_directives, CommonDirectiveParser};

pub fn parse(input: &str) -> Result<DiagramType> {
    let mut diagram = ArchitectureDiagram::default();
    let mut first_line_processed = false;
    let mut common_parser = CommonDirectiveParser::new(); // For parsers with multi-line support
    
    for (line_num, line) in input.lines().enumerate() {
        // Header validation (from previous issue)
        if validate_diagram_header(line, line_num, &["architecture"], &mut first_line_processed)? {
            continue;
        }
        
        // Common directive parsing
        if common_parser.parse_line(line, &mut diagram.title, &mut diagram.accessibility) {
            continue;
        }
        
        // Parser-specific logic continues...
    }
    
    Ok(DiagramType::Architecture(diagram))
}
```

## Testing Strategy

Add comprehensive tests in `src/common/parser_utils.rs`:

```rust
#[cfg(test)]
mod directive_tests {
    use super::*;

    #[test]
    fn test_parse_title_directive() {
        let mut title = None;
        let mut acc = Accessibility::default();
        
        assert!(parse_common_directives("title My Diagram", &mut title, &mut acc));
        assert_eq!(title, Some("My Diagram".to_string()));
    }

    #[test]
    fn test_parse_accessibility_directives() {
        let mut title = None;
        let mut acc = Accessibility::default();
        
        assert!(parse_common_directives("accTitle: Accessible Title", &mut title, &mut acc));
        assert_eq!(acc.title, Some("Accessible Title".to_string()));
        
        assert!(parse_common_directives("accDescr: Description", &mut title, &mut acc));
        assert_eq!(acc.description, Some("Description".to_string()));
    }

    #[test]
    fn test_multiline_accessibility_description() {
        let mut parser = CommonDirectiveParser::new();
        let mut title = None;
        let mut acc = Accessibility::default();
        
        // Start multi-line
        assert!(parser.parse_line("accDescr {", &mut title, &mut acc));
        
        // Content lines
        assert!(parser.parse_line("This is a long", &mut title, &mut acc));
        assert!(parser.parse_line("accessibility description", &mut title, &mut acc));
        
        // End multi-line
        assert!(parser.parse_line("}", &mut title, &mut acc));
        
        assert_eq!(acc.description, Some("This is a long accessibility description".to_string()));
    }

    #[test]
    fn test_unrecognized_directive() {
        let mut title = None;
        let mut acc = Accessibility::default();
        
        assert!(!parse_common_directives("unknown directive", &mut title, &mut acc));
        assert_eq!(title, None);
        assert_eq!(acc.title, None);
        assert_eq!(acc.description, None);
    }
}
```

## Files to Modify

### New Files
- Tests for directive parsing utilities

### Modified Files
- `src/common/parser_utils.rs` (add directive functions)
- `src/common/ast.rs` (may need to standardize `Accessibility` struct)
- `src/parsers/*.rs` (10+ parser files)

## Migration Strategy

1. **Phase 1**: Implement utility functions and tests
2. **Phase 2**: Migrate simple parsers (architecture, block, packet, etc.)
3. **Phase 3**: Migrate complex parsers with multi-line support
4. **Phase 4**: Standardize `Accessibility` struct across all diagram types

## Risk Assessment

**Low to Medium Risk**: 
- Most changes are straightforward refactoring
- Multi-line parsing changes are more complex and need careful testing
- Need to ensure consistent `Accessibility` struct definition

**Mitigation**: 
- Implement comprehensive tests for utility functions
- Migrate parsers incrementally
- Run full test suite after each migration