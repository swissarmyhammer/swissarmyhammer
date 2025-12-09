# Error Types

The mermaid-parser provides comprehensive error handling through the `ParseError` enum, which covers all possible error conditions during parsing.

## Core Error Type

### `ParseError`

```rust
pub enum ParseError {
    EmptyInput,
    UnknownDiagramType(String),
    UnsupportedDiagramType(String),
    LexError { message: String, line: usize, column: usize },
    SyntaxError { message: String, expected: Vec<String>, found: String, line: usize, column: usize },
    EnhancedSyntaxError { message: String, location: Location, snippet: Box<String>, suggestions: Box<Vec<String>>, expected: Box<Vec<String>>, found: String },
    SemanticError { message: String, context: String },
    IoError(String),
}
```

## Error Variants

### `EmptyInput`

Returned when the input contains no valid diagram content (empty string, only whitespace, or only comments).

**Example:**
```rust
use mermaid_parser::{parse_diagram, ParseError};

assert_eq!(parse_diagram(""), Err(ParseError::EmptyInput));
assert_eq!(parse_diagram("// Just comments"), Err(ParseError::EmptyInput));
```

### `UnknownDiagramType(String)`

Returned when a diagram type keyword is not recognized. Contains the unknown keyword.

**Example:**
```rust
match parse_diagram("unknownDiagram\nsome content") {
    Err(ParseError::UnknownDiagramType(diagram_type)) => {
        println!("Unknown diagram type: {}", diagram_type);
    }
    _ => {}
}
```

### `UnsupportedDiagramType(String)`

Returned when a diagram type is recognized but not yet implemented. Contains the diagram type name.

### `LexError`

Lexical analysis errors during tokenization.

**Fields:**
- `message: String` - Description of the lexical error
- `line: usize` - Line number where error occurred (1-indexed)
- `column: usize` - Column number where error occurred (1-indexed)

**Example:**
```rust
match parse_diagram("flowchart TD\n    A --> \u{1F4A9}") {  // Invalid character
    Err(ParseError::LexError { message, line, column }) => {
        println!("Lexical error at {}:{}: {}", line, column, message);
    }
    _ => {}
}
```

### `SyntaxError`

Standard syntax parsing errors with expectation information.

**Fields:**
- `message: String` - Human-readable error description
- `expected: Vec<String>` - List of expected tokens/constructs
- `found: String` - What was actually found
- `line: usize` - Line number (1-indexed)
- `column: usize` - Column number (1-indexed)

**Example:**
```rust
match parse_diagram("flowchart TD\n    A => B") {  // Wrong arrow type
    Err(ParseError::SyntaxError { message, expected, found, line, column }) => {
        println!("Syntax error at {}:{}: {}", line, column, message);
        println!("Expected one of: {:?}", expected);
        println!("Found: {}", found);
    }
    _ => {}
}
```

### `EnhancedSyntaxError`

Advanced syntax errors with context snippets and suggestions.

**Fields:**
- `message: String` - Error description
- `location: Location` - Precise location information
- `snippet: Box<String>` - Code snippet showing the error context
- `suggestions: Box<Vec<String>>` - Suggested fixes
- `expected: Box<Vec<String>>` - Expected constructs
- `found: String` - Actual content found

**Example:**
```rust
match parse_diagram(complex_invalid_input) {
    Err(ParseError::EnhancedSyntaxError { message, snippet, suggestions, .. }) => {
        println!("Error: {}", message);
        println!("Context:\n{}", snippet);
        println!("Suggestions:");
        for suggestion in suggestions.iter() {
            println!("  - {}", suggestion);
        }
    }
    _ => {}
}
```

### `SemanticError`

Errors where syntax is valid but the meaning is incorrect.

**Fields:**
- `message: String` - Description of the semantic issue
- `context: String` - Additional context about the error

**Example:**
```rust
// Valid syntax but semantic issue (e.g., duplicate node IDs)
match parse_diagram("flowchart TD\n    A --> B\n    A[Different Label] --> C") {
    Err(ParseError::SemanticError { message, context }) => {
        println!("Semantic error: {}", message);
        println!("Context: {}", context);
    }
    _ => {}
}
```

### `IoError(String)`

Input/output related errors when reading diagram content.

## Helper Types

### `Location`

```rust
pub struct Location {
    pub line: usize,
    pub column: usize,
}
```

Represents a specific location in the input text for precise error reporting.

### `Result<T>`

```rust
pub type Result<T> = std::result::Result<T, ParseError>;
```

Type alias used throughout the crate for consistent error handling.

## Error Handling Patterns

### Basic Error Handling

```rust
use mermaid_parser::{parse_diagram, ParseError};

match parse_diagram(input) {
    Ok(diagram) => process_diagram(diagram),
    Err(e) => {
        eprintln!("Parse failed: {}", e);
        return;
    }
}
```

### Detailed Error Information

```rust
fn handle_parse_error(error: &ParseError) {
    match error {
        ParseError::EmptyInput => {
            println!("No diagram content found. Please provide a valid Mermaid diagram.");
        }
        ParseError::SyntaxError { message, line, column, expected, .. } => {
            println!("Syntax error at line {}, column {}: {}", line, column, message);
            if !expected.is_empty() {
                println!("Expected one of: {}", expected.join(", "));
            }
        }
        ParseError::SemanticError { message, context } => {
            println!("Semantic error: {}", message);
            if !context.is_empty() {
                println!("Context: {}", context);
            }
        }
        _ => println!("Parse error: {}", error),
    }
}
```

### User-Friendly Error Messages

```rust
fn user_friendly_error(error: &ParseError) -> String {
    match error {
        ParseError::EmptyInput => {
            "The diagram appears to be empty. Please add some content.".to_string()
        }
        ParseError::SyntaxError { line, column, expected, .. } => {
            format!(
                "There's a syntax error on line {} at position {}. Expected: {}",
                line, column, expected.join(" or ")
            )
        }
        ParseError::UnknownDiagramType(diagram_type) => {
            format!(
                "Unknown diagram type '{}'. Supported types include: flowchart, sequence, class, etc.",
                diagram_type
            )
        }
        _ => format!("Parse error: {}", error),
    }
}
```

## Error Recovery

The parser includes intelligent error recovery strategies:

- **Fallback parsing**: Unknown diagram types are handled by the `misc` parser
- **Context preservation**: All errors include location information for debugging
- **Suggestion system**: Enhanced errors provide actionable fix suggestions
- **Graceful degradation**: Partial parsing when possible

## Testing Error Conditions

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_input_error() {
        assert!(matches!(parse_diagram(""), Err(ParseError::EmptyInput)));
    }

    #[test]
    fn test_syntax_error_location() {
        match parse_diagram("flowchart TD\n    A => B") {
            Err(ParseError::SyntaxError { line, column, .. }) => {
                assert_eq!(line, 2);
                assert!(column > 0);
            }
            _ => panic!("Expected syntax error"),
        }
    }
}
