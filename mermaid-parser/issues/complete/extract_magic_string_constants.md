# Extract Magic String Constants

**Priority**: Low  
**Impact**: Codebase-wide consistency  
**Effort**: Low  

## Problem

Magic strings are scattered throughout the codebase, making maintenance difficult and error-prone. These strings represent diagram type identifiers, keywords, and common patterns that should be centralized for consistency and easier updates.

### Current Magic Strings Found

#### Diagram Type Headers
Found across multiple parser files:

```rust
// Architecture related
"architecture"          // Found in 3+ files
"architecture-beta"     // Found in 3+ files

// State diagram related  
"stateDiagram"          // Found in 4+ files
"stateDiagram-v2"       // Found in 3+ files

// Flow diagram related
"flowchart"             // Found in 5+ files
"flowchart TD"          // Found in multiple tests
"flowchart LR"          // Found in multiple tests
"graph"                 // Found in 3+ files

// Sequence diagram related
"sequenceDiagram"       // Found in 4+ files

// Other diagram types
"sankey-beta"           // Found in 3+ files
"timeline"              // Found in 3+ files  
"gantt"                 // Found in 3+ files
"pie"                   // Found in 3+ files
"c4context"             // Found in 2+ files
"c4container"           // Found in 2+ files
"block-beta"            // Found in 2+ files
"packet-beta"           // Found in 2+ files
"xychart-beta"          // Found in 2+ files
"treemap-beta"          // Found in 2+ files
```

#### Comment Patterns
```rust
"//"                    // Found in 20+ files
"%%"                    // Found in 20+ files
```

#### Common Directives
```rust
"title "                // Found in 10+ files
"accTitle: "            // Found in 8+ files
"accDescr: "            // Found in 8+ files
"accDescr {"            // Found in 5+ files
```

#### Arrow Types and Symbols
```rust
"-->"                   // Found in 10+ files
"->"                    // Found in 8+ files
"==>"                   // Found in 5+ files
"=>"                    // Found in 5+ files
".->"                   // Found in 3+ files
```

## Solution

Create a centralized constants module with organized string definitions:

```rust
// src/common/constants.rs

/// Diagram type identifiers and headers
pub mod diagram_headers {
    // Architecture diagrams
    pub const ARCHITECTURE: &str = "architecture";
    pub const ARCHITECTURE_BETA: &str = "architecture-beta";
    pub const ARCHITECTURE_HEADERS: &[&str] = &[ARCHITECTURE, ARCHITECTURE_BETA];
    
    // State diagrams
    pub const STATE_V1: &str = "stateDiagram";
    pub const STATE_V2: &str = "stateDiagram-v2";
    pub const STATE_HEADERS: &[&str] = &[STATE_V1, STATE_V2];
    
    // Flow diagrams
    pub const FLOWCHART: &str = "flowchart";
    pub const GRAPH: &str = "graph";
    pub const FLOW_HEADERS: &[&str] = &[FLOWCHART, GRAPH];
    
    // Sequence diagrams
    pub const SEQUENCE: &str = "sequenceDiagram";
    pub const SEQUENCE_HEADERS: &[&str] = &[SEQUENCE];
    
    // Sankey diagrams
    pub const SANKEY: &str = "sankey";
    pub const SANKEY_BETA: &str = "sankey-beta";
    pub const SANKEY_HEADERS: &[&str] = &[SANKEY, SANKEY_BETA];
    
    // Timeline diagrams
    pub const TIMELINE: &str = "timeline";
    pub const TIMELINE_HEADERS: &[&str] = &[TIMELINE];
    
    // Gantt diagrams
    pub const GANTT: &str = "gantt";
    pub const GANTT_TEST_CLICK: &str = "gantttestclick";
    pub const GANTT_HEADERS: &[&str] = &[GANTT, GANTT_TEST_CLICK];
    
    // Pie charts
    pub const PIE: &str = "pie";
    pub const PIE_HEADERS: &[&str] = &[PIE];
    
    // C4 diagrams
    pub const C4_CONTEXT: &str = "c4context";
    pub const C4_CONTAINER: &str = "c4container";
    pub const C4_COMPONENT: &str = "c4component";
    pub const C4_DYNAMIC: &str = "c4dynamic";
    pub const C4_DEPLOYMENT: &str = "c4deployment";
    pub const C4_HEADERS: &[&str] = &[
        C4_CONTEXT, C4_CONTAINER, C4_COMPONENT, C4_DYNAMIC, C4_DEPLOYMENT
    ];
    
    // Block diagrams
    pub const BLOCK: &str = "block";
    pub const BLOCK_BETA: &str = "block-beta";
    pub const BLOCK_HEADERS: &[&str] = &[BLOCK, BLOCK_BETA];
    
    // Packet diagrams
    pub const PACKET: &str = "packet";
    pub const PACKET_BETA: &str = "packet-beta";
    pub const PACKET_HEADERS: &[&str] = &[PACKET, PACKET_BETA];
    
    // XY charts
    pub const XYCHART: &str = "xychart";
    pub const XYCHART_BETA: &str = "xychart-beta";
    pub const XYCHART_HEADERS: &[&str] = &[XYCHART, XYCHART_BETA];
    
    // Treemap diagrams
    pub const TREEMAP: &str = "treemap";
    pub const TREEMAP_BETA: &str = "treemap-beta";
    pub const TREEMAP_HEADERS: &[&str] = &[TREEMAP, TREEMAP_BETA];
    
    // Other diagram types
    pub const MINDMAP: &str = "mindmap";
    pub const QUADRANT: &str = "quadrant";
    pub const QUADRANT_CHART: &str = "quadrantchart";
    pub const JOURNEY: &str = "journey";
    pub const KANBAN: &str = "kanban";
    pub const RADAR: &str = "radar";
    pub const REQUIREMENT: &str = "requirement";
    pub const REQUIREMENT_DIAGRAM: &str = "requirementdiagram";
    pub const ER_DIAGRAM: &str = "erdiagram";
    pub const ER_DIAGRAM_TITLE: &str = "erdiagramtitletext";
    pub const CLASS_DIAGRAM: &str = "classdiagram";
    pub const GIT_GRAPH: &str = "gitgraph";
    pub const INFO: &str = "info";
}

/// Common directive prefixes
pub mod directives {
    pub const TITLE: &str = "title ";
    pub const ACC_TITLE: &str = "accTitle: ";
    pub const ACC_DESC: &str = "accDescr: ";
    pub const ACC_DESC_START: &str = "accDescr {";
    pub const ACC_DESC_END: &str = "}";
}

/// Comment patterns
pub mod comments {
    pub const DOUBLE_SLASH: &str = "//";
    pub const DOUBLE_PERCENT: &str = "%%";
    pub const COMMENT_PREFIXES: &[&str] = &[DOUBLE_SLASH, DOUBLE_PERCENT];
}

/// Arrow types and symbols used in diagrams
pub mod arrows {
    // Basic arrows
    pub const ARROW_RIGHT: &str = "-->";
    pub const ARROW_RIGHT_SHORT: &str = "->";
    pub const ARROW_LEFT: &str = "<--";
    pub const ARROW_LEFT_SHORT: &str = "<-";
    
    // Thick arrows
    pub const THICK_ARROW_RIGHT: &str = "==>";
    pub const THICK_ARROW_LEFT: &str = "<==";
    pub const THICK_ARROW_BIDIRECTIONAL: &str = "<=>";
    
    // Dotted arrows
    pub const DOTTED_ARROW_RIGHT: &str = ".->";
    pub const DOTTED_ARROW_LEFT: &str = "<-.";
    pub const DOTTED_ARROW_BIDIRECTIONAL: &str = "<-.>";
    
    // Special sequence diagram arrows
    pub const SEQUENCE_SYNC: &str = "->>";
    pub const SEQUENCE_ASYNC: &str = "->";
    pub const SEQUENCE_REPLY: &str = "-->>";
    
    // All arrow patterns for pattern matching
    pub const ALL_ARROWS: &[&str] = &[
        ARROW_RIGHT, ARROW_RIGHT_SHORT, ARROW_LEFT, ARROW_LEFT_SHORT,
        THICK_ARROW_RIGHT, THICK_ARROW_LEFT, THICK_ARROW_BIDIRECTIONAL,
        DOTTED_ARROW_RIGHT, DOTTED_ARROW_LEFT, DOTTED_ARROW_BIDIRECTIONAL,
        SEQUENCE_SYNC, SEQUENCE_ASYNC, SEQUENCE_REPLY,
    ];
}

/// Flow directions commonly used in diagrams
pub mod directions {
    pub const TOP_DOWN: &str = "TD";
    pub const TOP_BOTTOM: &str = "TB";
    pub const BOTTOM_TOP: &str = "BT";
    pub const LEFT_RIGHT: &str = "LR";
    pub const RIGHT_LEFT: &str = "RL";
    
    pub const ALL_DIRECTIONS: &[&str] = &[
        TOP_DOWN, TOP_BOTTOM, BOTTOM_TOP, LEFT_RIGHT, RIGHT_LEFT
    ];
}

/// Common test patterns
pub mod test_patterns {
    pub const FLOWCHART_TD: &str = "flowchart TD";
    pub const FLOWCHART_LR: &str = "flowchart LR";
    pub const SIMPLE_SEQUENCE: &str = "sequenceDiagram\n    A->>B: Hello";
    pub const SIMPLE_STATE: &str = "stateDiagram-v2\n    [*] --> A";
}

/// Error message templates
pub mod error_messages {
    pub const EXPECTED_HEADER: &str = "Expected {} header";
    pub const INVALID_SYNTAX: &str = "Invalid syntax";
    pub const UNEXPECTED_TOKEN: &str = "Unexpected token";
    pub const MISSING_CLOSING: &str = "Missing closing bracket";
    pub const INVALID_ARROW: &str = "Invalid arrow syntax";
}
```

### Usage Examples

```rust
// In parsers/architecture.rs
use crate::common::constants::diagram_headers;

if validate_diagram_header(line, line_num, diagram_headers::ARCHITECTURE_HEADERS, &mut first_line_processed)? {
    continue;
}

// In parsers/flowchart.rs  
use crate::common::constants::{diagram_headers, arrows, directions};

// Check for flowchart header
if !diagram_headers::FLOW_HEADERS.iter().any(|h| trimmed.starts_with(h)) {
    return Err(ParseError::SyntaxError { /* ... */ });
}

// Parse arrow type
if line.contains(arrows::ARROW_RIGHT) {
    // Handle right arrow
} else if line.contains(arrows::THICK_ARROW_RIGHT) {
    // Handle thick arrow
}

// In tests
use crate::common::constants::test_patterns;

#[test]
fn test_simple_flowchart() {
    let result = parse_diagram(test_patterns::FLOWCHART_TD);
    assert!(result.is_ok());
}
```

## Implementation Steps

### Phase 1: Create Constants Module

1. **Create `src/common/constants.rs`** with all identified constants
2. **Add module to `src/common/mod.rs`**:
   ```rust
   pub mod constants;
   ```
3. **Export commonly used constants** in `src/lib.rs` if needed

### Phase 2: Update Parser Files

**High-frequency replacements first:**

1. **Comment patterns** (affects 20+ files):
   ```rust
   // Before
   if trimmed.starts_with("//") || trimmed.starts_with("%%") {
   
   // After
   use crate::common::constants::comments;
   if comments::COMMENT_PREFIXES.iter().any(|p| trimmed.starts_with(p)) {
   ```

2. **Diagram headers** (affects 15+ files):
   ```rust
   // Before
   if !(trimmed.starts_with("architecture") || trimmed.starts_with("architecture-beta")) {
   
   // After
   use crate::common::constants::diagram_headers;
   if !diagram_headers::ARCHITECTURE_HEADERS.iter().any(|h| trimmed.starts_with(h)) {
   ```

3. **Common directives** (affects 10+ files):
   ```rust
   // Before
   if trimmed.starts_with("title ") {
   
   // After
   use crate::common::constants::directives;
   if trimmed.starts_with(directives::TITLE) {
   ```

### Phase 3: Update Test Files

Replace magic strings in test files:

```rust
// Before
let input = "flowchart TD\n    A --> B";

// After  
use crate::common::constants::test_patterns;
let input = test_patterns::FLOWCHART_TD;
```

### Phase 4: Update Error Messages

Standardize error message templates:

```rust
// Before
message: "Expected architecture header".to_string(),

// After
use crate::common::constants::error_messages;
message: format!(error_messages::EXPECTED_HEADER, "architecture"),
```

## Detailed Constant Organization

### Hierarchical Organization

```rust
// src/common/constants.rs

/// Diagram-specific constants organized by type
pub mod diagrams {
    pub mod architecture {
        pub const HEADER: &str = "architecture";
        pub const HEADER_BETA: &str = "architecture-beta";
        pub const ALL_HEADERS: &[&str] = &[HEADER, HEADER_BETA];
    }
    
    pub mod flowchart {
        pub const HEADER: &str = "flowchart";
        pub const GRAPH_ALIAS: &str = "graph";
        pub const ALL_HEADERS: &[&str] = &[HEADER, GRAPH_ALIAS];
        
        pub mod directions {
            pub const TD: &str = "TD";
            pub const LR: &str = "LR";
            pub const ALL: &[&str] = &[TD, LR];
        }
    }
    
    // Similar organization for other diagram types...
}

/// Syntax elements used across multiple diagram types
pub mod syntax {
    pub mod comments {
        pub const LINE_COMMENT: &str = "//";
        pub const BLOCK_COMMENT: &str = "%%";
        pub const ALL: &[&str] = &[LINE_COMMENT, BLOCK_COMMENT];
    }
    
    pub mod arrows {
        pub const BASIC_RIGHT: &str = "-->";
        pub const BASIC_LEFT: &str = "<--";
        pub const THICK_RIGHT: &str = "==>";
        // ... more arrow types
    }
    
    pub mod brackets {
        pub const ROUND_OPEN: &str = "(";
        pub const ROUND_CLOSE: &str = ")";
        pub const SQUARE_OPEN: &str = "[";
        pub const SQUARE_CLOSE: &str = "]";
        pub const CURLY_OPEN: &str = "{";
        pub const CURLY_CLOSE: &str = "}";
    }
}
```

### Validation Helpers

```rust
/// Helper functions for working with constants
impl constants {
    /// Check if a line starts with any comment prefix
    pub fn is_comment_line(line: &str) -> bool {
        let trimmed = line.trim();
        syntax::comments::ALL.iter().any(|prefix| trimmed.starts_with(prefix))
    }
    
    /// Check if a string matches any arrow pattern
    pub fn is_arrow(text: &str) -> bool {
        syntax::arrows::ALL.iter().any(|arrow| text.contains(arrow))
    }
    
    /// Get the diagram type from a header line
    pub fn detect_diagram_type(line: &str) -> Option<&'static str> {
        let trimmed = line.trim().to_lowercase();
        
        if diagrams::architecture::ALL_HEADERS.iter().any(|h| trimmed.starts_with(h)) {
            return Some("architecture");
        }
        
        if diagrams::flowchart::ALL_HEADERS.iter().any(|h| trimmed.starts_with(h)) {
            return Some("flowchart");
        }
        
        // ... check other diagram types
        
        None
    }
}
```

## Benefits

- **Maintainability**: All magic strings centralized in one location
- **Consistency**: Identical strings used everywhere they appear
- **Refactoring safety**: Changes to strings only need to happen in one place
- **Documentation**: Constants serve as documentation of supported syntax
- **Type safety**: Can create typed constants for better compile-time checking
- **Testing**: Easier to create comprehensive tests using predefined patterns

## Enhanced Features

### Typed Constants

```rust
// For even better type safety
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagramType {
    Architecture,
    Flowchart,
    Sequence,
    // ... other types
}

impl DiagramType {
    pub fn headers(&self) -> &'static [&'static str] {
        match self {
            Self::Architecture => diagram_headers::ARCHITECTURE_HEADERS,
            Self::Flowchart => diagram_headers::FLOW_HEADERS,
            Self::Sequence => diagram_headers::SEQUENCE_HEADERS,
            // ... other types
        }
    }
    
    pub fn name(&self) -> &'static str {
        match self {
            Self::Architecture => "architecture",
            Self::Flowchart => "flowchart", 
            Self::Sequence => "sequence",
            // ... other types
        }
    }
}
```

### Build-time Validation

```rust
// In build.rs - validate that all constants are used
fn validate_constants() {
    // Check that all defined constants are actually used in the codebase
    // Report unused constants
    // Validate that magic strings in code match defined constants
}
```

## Files to Modify

### New Files
- `src/common/constants.rs`

### Modified Files  
- `src/common/mod.rs` (add constants module)
- `src/parsers/*.rs` (15+ parser files)
- `tests/*.rs` (20+ test files)
- `src/lib.rs` (if re-exporting constants)

## Migration Strategy

1. **Create constants module** with all identified strings
2. **Update one parser at a time** starting with most frequent patterns
3. **Update test files** to use predefined test patterns
4. **Add validation helpers** for common operations
5. **Consider typed constants** for additional type safety

## Risk Assessment

**Very Low Risk**: 
- Pure refactoring with no behavioral changes
- Each replacement can be done incrementally
- Easy to verify correctness by checking string equality

**Benefits outweigh effort**:
- Low effort, high maintainability benefit
- Foundation for future syntax extensions
- Easier to ensure consistency across parsers