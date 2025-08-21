# Implement Shell Action Parser

Refer to ./specification/shell.md

## Overview

Implement the parser for shell actions by adding `parse_shell_action` method to the `ActionParser` struct. This parser will handle the shell action syntax as specified in the specification.

## Objective

Create a robust parser that can parse shell action descriptions and convert them into `ShellAction` instances, following the established parser patterns in the codebase.

## Tasks

### 1. Implement parse_shell_action Method

Add to `ActionParser` in `/Users/wballard/github/swissarmyhammer/swissarmyhammer/src/workflow/action_parser.rs`:

```rust
/// Parse a shell action from description
/// Format: Shell "command" [with timeout=N] [result="variable"] [working_dir="path"] [env={"KEY": "value"}]
pub fn parse_shell_action(&self, description: &str) -> ActionResult<Option<ShellAction>> {
    // Implementation following the specification syntax
}
```

### 2. Support Required Syntax Patterns

According to the specification, support these formats:
- `Shell "command"` - Basic shell command
- `shell "command"` - Case-insensitive version
- `Shell "command" with timeout=30` - With timeout
- `Shell "command" with result="output_variable"` - With result capture
- `Shell "command" with timeout=30 result="output"` - Combined parameters

### 3. Implement Parameter Parsing

Support parsing of optional parameters:
- `timeout=N` - Parse integer timeout value
- `result="variable_name"` - Parse result variable name  
- `working_dir="/path/to/dir"` - Parse working directory
- `env={"KEY": "value", "KEY2": "value2"}` - Parse environment variables (JSON object)

### 4. Follow Existing Parser Patterns

Use the same patterns as other action parsers:
- Use `case_insensitive` helper for the "Shell" keyword
- Use `quoted_string` for command parsing
- Use `argument_key` and parameter parsing patterns
- Return `Ok(None)` if parsing fails (not this action type)
- Return `Ok(Some(action))` if parsing succeeds
- Return `Err(ActionError)` for validation errors

### 5. Validation

Add validation for:
- Command string is not empty
- Timeout values are positive integers
- Result variable names are valid identifiers
- Working directory paths are reasonable
- Environment variable JSON is properly formatted

## Implementation Approach

Follow the pattern from `parse_prompt_action` and other existing parsers:

```rust
pub fn parse_shell_action(&self, description: &str) -> ActionResult<Option<ShellAction>> {
    let parser = Self::case_insensitive("shell")
        .then_ignore(Self::whitespace())
        .ignore_then(Self::quoted_string())
        .then(
            Self::whitespace()
                .ignore_then(Self::case_insensitive("with"))
                .ignore_then(Self::whitespace())
                .ignore_then(/* parameter parsing */)
                .or_not()
        );
    
    match parser.parse(description.trim()).into_result() {
        Ok((command, params)) => {
            // Build ShellAction from parsed components
            Ok(Some(ShellAction::new(command)/* .with_params(...) */))
        }
        Err(_) => Ok(None),
    }
}
```

## Success Criteria

- [ ] Parser method implemented and compiles
- [ ] All syntax formats from specification are supported
- [ ] Case-insensitive parsing works
- [ ] Parameter parsing works for all optional parameters
- [ ] Validation prevents invalid inputs
- [ ] Parser follows existing code patterns
- [ ] Comprehensive unit tests written

## Testing

Write unit tests in the `#[cfg(test)]` section:
- Test basic shell command parsing
- Test case-insensitive parsing
- Test parameter parsing (timeout, result, working_dir, env)
- Test combined parameters
- Test invalid syntax returns None
- Test validation errors

## Integration Point

The parser method will be called from the dispatch system in the next step.

## Next Steps

After completing this step, proceed to integrating the shell action with the dispatch system.

## Proposed Solution

Based on the existing parser patterns and ShellAction structure, I will implement the `parse_shell_action` method following these steps:

### 1. Parser Implementation Strategy

Follow the established pattern from `parse_prompt_action` and other existing parsers:
- Use `case_insensitive("shell")` for keyword matching
- Use `quoted_string()` for command parsing
- Use parameter parsing patterns for optional parameters
- Return `Ok(None)` if not a shell action, `Ok(Some(action))` if parsing succeeds

### 2. Syntax Support

The parser will support these formats from the specification:
- `Shell "command"` - Basic shell command  
- `shell "command"` - Case-insensitive version
- `Shell "command" with timeout=30` - With timeout
- `Shell "command" with result="output_variable"` - With result capture
- `Shell "command" with timeout=30 result="output"` - Combined parameters
- `Shell "command" with working_dir="/path"` - With working directory
- `Shell "command" with env={"KEY": "value"}` - With environment variables

### 3. Parameter Parsing Implementation

Build a parameter parser that handles:
- `timeout=N` - Parse integer timeout value (seconds)
- `result="variable_name"` - Parse result variable name
- `working_dir="/path/to/dir"` - Parse working directory path
- `env={"KEY": "value", "KEY2": "value2"}` - Parse JSON environment variables

### 4. Validation Rules

Implement validation to ensure:
- Command string is not empty
- Timeout values are positive integers  
- Result variable names are valid identifiers (using `is_valid_variable_name`)
- Working directory paths are non-empty strings
- Environment variable JSON parses correctly as HashMap<String, String>

### 5. Parser Method Signature

```rust
pub fn parse_shell_action(&self, description: &str) -> ActionResult<Option<ShellAction>> {
    // Implementation follows existing patterns
}
```

### 6. Integration Points

- Add `ShellAction` import to action_parser.rs
- Call `parse_shell_action` from `parse_action_from_description` function
- Follow existing error handling patterns

### 7. Testing Strategy

Write comprehensive unit tests covering:
- Basic shell command parsing (`Shell "echo hello"`)
- Case-insensitive parsing (`shell "pwd"`)
- Parameter parsing for each type (timeout, result, working_dir, env)
- Combined parameter parsing (`Shell "ls" with timeout=30 result="files"`)
- Invalid syntax returning None
- Validation error cases
- Edge cases and error conditions

This approach ensures consistency with existing parser patterns while fully supporting the shell action specification requirements.