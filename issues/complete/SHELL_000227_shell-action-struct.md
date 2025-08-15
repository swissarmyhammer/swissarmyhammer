# Implement ShellAction Struct

Refer to ./specification/shell.md

## Overview

Implement the core `ShellAction` struct following the established patterns in the codebase. This struct will hold all the parameters needed for shell command execution as specified in the shell action specification.

## Objective

Create a `ShellAction` struct that can store shell command parameters and implement the basic structure needed for shell action execution.

## Tasks

### 1. Define ShellAction Struct
Following the pattern of other action structs like `PromptAction`, implement:

```rust
pub struct ShellAction {
    /// The shell command to execute
    pub command: String,
    /// Optional timeout for command execution
    pub timeout: Option<Duration>,
    /// Optional variable name to store command output
    pub result_variable: Option<String>,
    /// Optional working directory for command execution
    pub working_dir: Option<String>,
    /// Optional environment variables for the command
    pub environment: HashMap<String, String>,
}
```

### 2. Implement Constructor and Builder Methods
- `new(command: String)` - Basic constructor
- `with_timeout(timeout: Duration)` - Fluent builder for timeout
- `with_result_variable(variable: String)` - Fluent builder for result capture
- `with_working_dir(dir: String)` - Fluent builder for working directory
- `with_environment(env: HashMap<String, String>)` - Fluent builder for environment

### 3. Implement VariableSubstitution Trait
- Follow the pattern from other actions
- Enable variable substitution in command strings
- Enable variable substitution in working directory paths
- Enable variable substitution in environment variable values

### 4. Implement Action Trait Stub
- Add basic `Action` trait implementation with placeholder execute method
- Implement `description()` method that returns a meaningful description
- Implement `action_type()` method returning "shell"
- Add `impl_as_any!()` macro usage

## Implementation Location

Add the implementation to `/Users/wballard/github/swissarmyhammer/swissarmyhammer/src/workflow/actions.rs`

## Expected Code Structure

```rust
/// Shell action for executing shell commands in workflows
#[derive(Debug, Clone)]
pub struct ShellAction {
    pub command: String,
    pub timeout: Option<Duration>,
    pub result_variable: Option<String>,
    pub working_dir: Option<String>,
    pub environment: HashMap<String, String>,
}

impl ShellAction {
    pub fn new(command: String) -> Self { ... }
    pub fn with_timeout(mut self, timeout: Duration) -> Self { ... }
    // ... other builder methods
}

impl VariableSubstitution for ShellAction { ... }

#[async_trait::async_trait]
impl Action for ShellAction {
    async fn execute(&self, context: &mut HashMap<String, Value>) -> ActionResult<Value> {
        // TODO: Implement in next step
        unimplemented!("Shell action execution to be implemented")
    }
    
    fn description(&self) -> String { ... }
    fn action_type(&self) -> &'static str { "shell" }
    impl_as_any!();
}
```

## Success Criteria

- [ ] ShellAction struct defined with all required fields
- [ ] Constructor and builder methods implemented
- [ ] VariableSubstitution trait implemented
- [ ] Action trait implemented with placeholder execute method
- [ ] Code compiles without errors
- [ ] Code follows existing patterns and conventions

## Testing

Write basic unit tests to verify:
- Struct construction works correctly
- Builder methods chain properly
- Variable substitution works for command strings
- Description method returns appropriate text

## Next Steps

After completing this step, proceed to implementing the shell command parser integration.
## Proposed Solution

After analyzing the existing codebase patterns and the shell specification, I will implement the `ShellAction` struct by following these established patterns:

### Implementation Steps

1. **Define the ShellAction struct** following the pattern of `PromptAction`:
   - All required and optional fields from the specification
   - Use `Duration` for timeout (matching existing patterns)
   - Use `HashMap<String, String>` for environment variables
   - Use `Option<String>` for optional fields

2. **Implement constructor and builder methods**:
   - `new(command: String)` - Basic constructor with sensible defaults
   - Fluent builder methods matching existing patterns:
     - `with_timeout()`, `with_result_variable()`, `with_working_dir()`, `with_environment()`

3. **Implement VariableSubstitution trait**:
   - Enable variable substitution in command strings, working directory, and environment values
   - Follow the exact pattern used by other actions

4. **Implement Action trait**:
   - Stub implementation with `unimplemented!()` for the execute method
   - Proper `description()` and `action_type()` methods
   - Use `impl_as_any!()` macro

5. **Add comprehensive unit tests**:
   - Test struct construction and builder methods
   - Test variable substitution functionality
   - Test description and action type methods

### Code Structure

```rust
/// Shell action for executing shell commands in workflows
#[derive(Debug, Clone)]
pub struct ShellAction {
    /// The shell command to execute
    pub command: String,
    /// Optional timeout for command execution (default: no timeout)
    pub timeout: Option<Duration>,
    /// Optional variable name to store command output
    pub result_variable: Option<String>,
    /// Optional working directory for command execution
    pub working_dir: Option<String>,
    /// Optional environment variables for the command
    pub environment: HashMap<String, String>,
}
```

This structure matches the specification requirements and follows the established codebase patterns exactly.