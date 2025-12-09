# Improve error message quality and context

## Description
Enhance parser error messages to provide clear, actionable feedback for invalid syntax. This is listed as a functional goal in PROJECT_STATUS.md.

## Current State
- Basic error handling implemented
- ParseError type exists
- Limited context in error messages

## Requirements
1. Add line and column information to errors
2. Include context snippet showing error location
3. Provide helpful suggestions for common mistakes
4. Create error recovery strategies
5. Implement similar errors to Rust compiler quality

## Error Message Improvements
```rust
// Current
ParseError("Expected arrow")

// Improved
ParseError {
    message: "Expected arrow '->' or '-->' after node",
    location: Location { line: 3, column: 15 },
    snippet: r#"
3 |     A => B
  |       ^^ expected '->' or '-->', found '=>'
  |
  = help: use '-->' for solid arrows or '-.->' for dotted arrows
    "#,
    suggestions: vec![
        "Did you mean '-->'?",
        "See https://mermaid.js.org/syntax/flowchart.html#links-between-nodes"
    ],
}
```

## Implementation Areas
- Enhance parser combinators to track position
- Create error formatting utilities
- Add contextual help for each diagram type
- Implement error recovery to continue parsing
- Create comprehensive error documentation

## Success Criteria
- Errors include line/column information
- Context snippets highlight problem areas
- Helpful suggestions for common mistakes
- Error messages guide users to solutions
- Similar quality to Rust compiler errors