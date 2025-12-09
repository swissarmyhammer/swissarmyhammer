# Error Handling

The mermaid-parser provides comprehensive error handling through the `ParseError` enum and `Result<T>` type alias.

## Error Types

### Basic Errors

- **`EmptyInput`**: The input contains no valid diagram content
- **`UnknownDiagramType`**: Diagram type not recognized by any parser
- **`UnsupportedDiagramType`**: Diagram type recognized but not yet implemented
- **`IoError`**: File system or input/output related errors

### Parse-Time Errors

- **`LexError`**: Problems during lexical analysis (tokenization)
- **`SyntaxError`**: Invalid syntax according to grammar rules
- **`EnhancedSyntaxError`**: Detailed syntax errors with context and suggestions
- **`SemanticError`**: Valid syntax but semantically incorrect

## Error Information

Each error provides contextual information:

```rust
use mermaid_parser::{ParseError, parse_diagram};

match parse_diagram("invalid input") {
    Ok(diagram) => println!("Success!"),
    Err(ParseError::SyntaxError { message, line, column, expected, found }) => {
        println!("Syntax error at {}:{}: {}", line, column, message);
        println!("Expected: {:?}, Found: {}", expected, found);
    }
    Err(e) => println!("Error: {}", e),
}
```

## Common Error Scenarios

### Empty or Invalid Input

```rust
use mermaid_parser::parse_diagram;

// Empty input
let result = parse_diagram("");
assert!(matches!(result, Err(ParseError::EmptyInput)));

// Only comments
let result = parse_diagram("// Just a comment\n# Another comment");
assert!(matches!(result, Err(ParseError::EmptyInput)));
```

### Unknown Diagram Types

```rust
use mermaid_parser::parse_diagram;

let result = parse_diagram("unknownDiagram\nsome content");
// Will be handled by the misc parser as fallback
assert!(result.is_ok());
```

### Syntax Errors

```rust
use mermaid_parser::parse_diagram;

// Invalid flowchart syntax
let result = parse_diagram("flowchart TD\n    A => B");  // Wrong arrow
match result {
    Err(ParseError::SyntaxError { line, column, .. }) => {
        println!("Syntax error at line {}, column {}", line, column);
    }
    _ => println!("Unexpected result"),
}
```

## Enhanced Error Messages

The parser provides detailed error messages with location information:

```rust
use mermaid_parser::error::format_error_snippet;

let input = "flowchart TD\n    A => B\n    B --> C";
let snippet = format_error_snippet(input, 2, 7, 9);
println!("{}", snippet);
// Output:
// 2 |     A => B
//         ^^ expected
```

## Error Recovery

The parser attempts graceful error recovery when possible:

- **Fallback Parsing**: Unknown diagram types are handled by the misc parser
- **Context Preservation**: Errors include line/column information for debugging
- **Suggestion System**: Enhanced errors provide fix suggestions

## Best Practices

### 1. Handle All Error Cases

```rust
use mermaid_parser::{parse_diagram, ParseError};

fn safe_parse(input: &str) -> Result<String, String> {
    match parse_diagram(input) {
        Ok(diagram) => Ok(format!("Parsed: {:?}", diagram)),
        Err(ParseError::EmptyInput) => Err("Please provide diagram content".to_string()),
        Err(ParseError::SyntaxError { message, line, column, .. }) => {
            Err(format!("Syntax error at {}:{}: {}", line, column, message))
        }
        Err(e) => Err(format!("Parse failed: {}", e)),
    }
}
```

### 2. User-Friendly Error Messages

```rust
fn user_friendly_error(err: &ParseError) -> String {
    match err {
        ParseError::EmptyInput => "The diagram appears to be empty. Please add some content.".to_string(),
        ParseError::SyntaxError { message, line, column, expected, .. } => {
            format!(
                "There's a syntax error on line {} at position {}:\n{}\n\nTip: Try using one of: {}",
                line, column, message, expected.join(", ")
            )
        }
        _ => format!("Something went wrong: {}", err),
    }
}
```

### 3. Validation Before Parsing

```rust
use mermaid_parser::parse_diagram;

fn validate_and_parse(input: &str) -> Result<(), String> {
    if input.trim().is_empty() {
        return Err("Input cannot be empty".to_string());
    }
    
    if input.lines().all(|line| line.trim().starts_with("//") || line.trim().is_empty()) {
        return Err("Input contains only comments".to_string());
    }
    
    parse_diagram(input)
        .map(|_| ())
        .map_err(|e| format!("Parse error: {}", e))
}
```
