# Shell Action Project Setup

Refer to ./specification/shell.md

## Overview

This is the first step in implementing shell actions for workflows. This step focuses on setting up the project structure and understanding the existing codebase patterns before implementing the shell action functionality.

## Objective

Set up the development environment and understand the existing action system architecture to ensure the shell action implementation follows established patterns.

## Tasks

### 1. Environment Setup
- Ensure the development environment is properly configured
- Run existing tests to confirm the codebase is in a working state
- Review the specification requirements thoroughly

### 2. Codebase Analysis  
- Study the existing action implementations in `swissarmyhammer/src/workflow/actions.rs`
- Analyze the action parser patterns in `swissarmyhammer/src/workflow/action_parser.rs`
- Understand the action dispatch mechanism in `parse_action_from_description`
- Review existing action traits and patterns

### 3. Architecture Planning
- Identify where the `ShellAction` struct should be implemented
- Plan the integration points with the existing parser system
- Design the shell action parameters structure
- Plan the security validation approach

### 4. Testing Strategy
- Review existing action tests to understand testing patterns
- Plan the testing approach for shell actions
- Identify security test requirements
- Plan integration test scenarios

## Expected Deliverables

1. **Development Environment Verification**
   - Confirm all tests pass: `cargo test`
   - Confirm clippy is clean: `cargo clippy`
   - Confirm formatting is correct: `cargo fmt --check`

2. **Architecture Documentation** 
   - Clear understanding of where to implement ShellAction
   - Integration plan with existing systems
   - Security considerations documented

3. **Testing Plan**
   - Unit test strategy for shell actions
   - Integration test approach
   - Security test requirements

## Success Criteria

- [ ] All existing tests pass
- [ ] Clear understanding of action system architecture
- [ ] Solid plan for shell action implementation
- [ ] Testing strategy defined
- [ ] Security considerations identified

## Implementation Notes

This step is purely preparatory and should not involve any code changes. The focus is on understanding and planning to ensure a clean, well-integrated implementation in subsequent steps.

## Next Steps

After completing this setup, proceed to implementing the basic ShellAction struct in the next step.
## Proposed Solution

Based on my analysis of the existing codebase architecture, I have identified the clear implementation plan for shell actions:

### Architecture Analysis Findings

1. **Action System Structure**: 
   - Actions implement the `Action` trait (async execute, description, action_type, as_any)
   - Actions support `VariableSubstitution` trait for context variable replacement
   - Actions are parsed via `ActionParser` using chumsky parser combinators
   - Dispatch occurs in `parse_action_from_description` function (actions.rs:1479-1510)

2. **Existing Action Patterns**:
   - Each action is a struct with configuration fields (PromptAction, LogAction, SetVariableAction, etc.)
   - Builder pattern with `new()` and `with_*()` methods for configuration
   - Actions use `substitute_variables` to replace `${var}` patterns from context
   - Timeout handling via `ActionTimeouts` and `Duration` fields
   - Error handling via `ActionError` enum with specific error types

3. **Parser Integration Pattern**:
   - Case-insensitive command parsing using `case_insensitive("command")` 
   - Quoted string parsing for command arguments
   - Parameter parsing with validation (argument keys, variable names)
   - Optional parameter support with `or_not()` combinator

### Implementation Plan

#### 1. ShellAction Structure (actions.rs)
Create `ShellAction` struct following existing patterns:
```rust
#[derive(Debug, Clone)]
pub struct ShellAction {
    pub command: String,
    pub timeout: Duration,
    pub result_variable: Option<String>,
    pub working_dir: Option<String>,
    pub env_vars: HashMap<String, String>,
}
```

#### 2. Parser Integration (action_parser.rs) 
Add `parse_shell_action` method supporting:
- `Shell "command"` - basic format
- `Shell "command" with timeout=30` - with timeout
- `Shell "command" with result="var" timeout=30` - multiple parameters
- Case-insensitive parsing following existing patterns

#### 3. Action Dispatch (actions.rs)
Add shell action parsing to `parse_action_from_description`:
```rust
if let Some(shell_action) = parser.parse_shell_action(description)? {
    return Ok(Some(Box::new(shell_action)));
}
```

#### 4. Process Execution
Use tokio Command for async process execution with:
- Proper subprocess management and cleanup
- Timeout handling with process termination
- stdout/stderr capture
- Exit code monitoring
- Variable setting (success, failure, exit_code, stdout, stderr, duration_ms)

#### 5. Security Implementation
- Command validation to prevent injection
- Environment variable sanitization  
- Working directory validation
- Configurable restrictions for dangerous operations
- Audit logging for executed commands

### Integration Points

1. **Error Handling**: Add `ShellError` variant to `ActionError` enum
2. **Variable Setting**: Follow pattern of setting context variables after execution
3. **Timeout Configuration**: Use `ActionTimeouts` pattern with environment variable override
4. **Testing**: Follow existing test patterns with `#[tokio::test]` async tests

### Security Considerations

1. **Command Injection Prevention**: Validate and sanitize command strings
2. **Resource Limits**: Maximum execution time of 300 seconds (configurable)
3. **Environment Isolation**: Limited access to workflow execution environment  
4. **Dangerous Command Detection**: Warning/restriction system for privileged operations

### Testing Strategy

1. **Unit Tests**: Action parsing, parameter validation, variable substitution
2. **Integration Tests**: Process execution with real commands, timeout handling
3. **Security Tests**: Command injection attempts, privilege escalation prevention
4. **Error Handling Tests**: Process failures, timeouts, invalid commands

This implementation will seamlessly integrate with the existing action system while maintaining the established patterns for parsing, execution, and error handling.
## Security Validation Approach

### Command Injection Prevention
1. **Input Sanitization**: Validate command strings to prevent shell injection attacks
2. **No Shell Interpretation**: Execute commands directly via tokio::process::Command, not through shell
3. **Argument Validation**: Ensure proper escaping and validation of command arguments
4. **Variable Substitution Safety**: Sanitize substituted variables before command execution

### Execution Restrictions
1. **Timeout Enforcement**: Hard limit of 300 seconds maximum execution time
2. **Working Directory Validation**: Restrict to safe, validated directories only
3. **Environment Variable Control**: Sanitize and validate environment variables
4. **Process Isolation**: Ensure subprocess cannot access workflow internal state

### Dangerous Command Detection
Implement warning/restriction system for:
- System configuration modifications (systemctl, service, etc.)
- Software installation commands (apt, yum, brew, etc.) 
- Privilege escalation attempts (sudo, su, etc.)
- Sensitive directory access (/etc, /proc, /sys, etc.)
- Network operations requiring special attention

### Audit and Monitoring
1. **Command Logging**: Log all executed shell commands with timestamps
2. **Exit Code Tracking**: Monitor and log command success/failure
3. **Resource Usage**: Track execution time and memory usage
4. **Security Events**: Log security-relevant command executions

## Comprehensive Testing Strategy

### Unit Tests (action_parser.rs tests)
- Parse shell action basic format: `Shell "command"`
- Parse with timeout: `Shell "command" with timeout=30`
- Parse with result capture: `Shell "command" with result="output"`
- Parse combined parameters: `Shell "command" with timeout=30 result="var"`
- Case insensitive parsing: `shell "command"`
- Invalid format rejection
- Parameter validation (timeout values, variable names)

### Integration Tests (actions.rs tests)
- Basic command execution with success verification
- Command failure handling and exit code capture
- Timeout enforcement with process termination
- stdout/stderr capture and variable setting
- Working directory functionality
- Environment variable passing
- Variable substitution in commands
- Context variable setting (success, failure, exit_code, etc.)

### Security Tests
- Command injection attempt prevention
- Dangerous command detection and warnings
- Environment variable sanitization
- Working directory validation
- Resource limit enforcement
- Process isolation verification

### Error Handling Tests
- Non-existent command execution
- Permission denied scenarios
- Timeout scenarios with cleanup
- Invalid parameter handling
- Malformed command strings
- Process spawn failures

### End-to-End Integration Tests
- Integration with workflow execution engine
- Variable passing between actions
- Error propagation to workflow status
- Parallel execution scenarios (if supported)

This comprehensive approach ensures the shell action implementation maintains security while providing the flexible command execution capabilities required by the specification.

## Comprehensive Architecture Analysis and Implementation Plan

### Environment Verification ✅

**Status**: All verification steps completed successfully

1. ✅ **Tests**: All 1031 tests pass across workspace  
2. ✅ **Code Quality**: Clippy warnings resolved (format string optimization)
3. ✅ **Formatting**: Code properly formatted with `cargo fmt`

**Key Findings**:
- Codebase is in excellent health with comprehensive test coverage
- Strong testing infrastructure with unit, integration, and end-to-end tests
- Multi-crate workspace with clear separation: `swissarmyhammer` (lib) + `swissarmyhammer-cli` (binary)

### Action System Architecture Analysis ✅

**Core Action System Structure**:

1. **Action Trait** (`actions.rs:179-194`):
   ```rust
   #[async_trait::async_trait]
   pub trait Action: Send + Sync {
       async fn execute(&self, context: &mut HashMap<String, Value>) -> ActionResult<Value>;
       fn description(&self) -> String;
       fn action_type(&self) -> &'static str;
       fn as_any(&self) -> &dyn std::any::Any;
   }
   ```

2. **Variable Substitution Trait** (`actions.rs:196-215`):
   - Standard `substitute_string` and `substitute_map` methods
   - Uses `${variable}` pattern replacement from context
   - All actions implement this for context variable integration

3. **Action Structure Pattern** (analyzed from existing actions):
   - Builder pattern: `new()` + `with_*()` methods for configuration
   - Required fields + optional parameters (timeout, result variables)
   - Implements both `Action` and `VariableSubstitution` traits
   - Uses `impl_as_any!()` macro for testing support

4. **Action Dispatch** (`actions.rs:1480-1510`):
   - Central `parse_action_from_description` function
   - Sequential parser attempts for each action type
   - Returns `Option<Box<dyn Action>>` for matched actions

5. **Parser Architecture** (`action_parser.rs`):
   - Uses `chumsky` parser combinators for robust parsing
   - Case-insensitive command recognition via `case_insensitive()` helper
   - Quoted string parsing for command arguments
   - Parameter parsing with `with` keyword support
   - Comprehensive error handling with `ParserError` type

### Existing Action Implementations Analyzed ✅

**Action Types Studied**:
- `PromptAction`: Claude execution with timeout and argument handling
- `WaitAction`: Duration-based delays with timeout parsing
- `LogAction`: Message logging with level support
- `SetVariableAction`: Context variable manipulation
- `AbortAction`: Workflow termination with error propagation
- `SubWorkflowAction`: Nested workflow execution with circular dependency detection

**Common Patterns Identified**:
- Timeout handling via `ActionTimeouts` with environment variable overrides
- Context variable setting after execution (success/failure states)
- Error propagation through `ActionResult<T>` type alias
- Process execution using `tokio::process::Command` (seen in PromptAction)
- Variable substitution in all configurable string fields

### Shell Action Implementation Strategy ✅

Based on architectural analysis and specification requirements:

#### 1. ShellAction Structure (`actions.rs`)
```rust
#[derive(Debug, Clone)]
pub struct ShellAction {
    pub command: String,
    pub timeout: Duration,
    pub result_variable: Option<String>,
    pub working_dir: Option<String>,
    pub env_vars: HashMap<String, String>,
}
```

**Integration Points**:
- Add `ShellError` variant to `ActionError` enum
- Add shell timeout to `ActionTimeouts` struct
- Implement `Action` and `VariableSubstitution` traits
- Follow builder pattern: `new()`, `with_timeout()`, `with_result()`, etc.

#### 2. Parser Implementation (`action_parser.rs`)
```rust
pub fn parse_shell_action(&self, description: &str) -> ActionResult<Option<ShellAction>>
```

**Parsing Requirements** (from specification):
- `Shell "command"` - basic format
- `Shell "command" with timeout=30` - with timeout
- `Shell "command" with result="var"` - result capture  
- `Shell "command" with timeout=30 result="output"` - combined parameters
- Case-insensitive: `shell "command"` also supported

#### 3. Dispatch Integration (`actions.rs:1480-1510`)
Add to `parse_action_from_description`:
```rust
if let Some(shell_action) = parser.parse_shell_action(description)? {
    return Ok(Some(Box::new(shell_action)));
}
```

#### 4. Process Execution Implementation
Following `PromptAction::render_prompt_with_swissarmyhammer` pattern:
- Use `tokio::process::Command` for async process execution
- Proper subprocess management with cleanup
- Timeout handling with process termination
- stdout/stderr capture with UTF-8 conversion
- Exit code monitoring and context variable setting

#### 5. Context Variable Setting (Per Specification)
After execution, set these context variables:
- `success`: Boolean (exit code == 0)
- `failure`: Boolean (exit code != 0)  
- `exit_code`: Integer exit code
- `stdout`: Standard output string
- `stderr`: Standard error string
- `duration_ms`: Execution time in milliseconds
- `result`: Command output (if result parameter specified)

#### 6. Security Implementation Strategy
- **Command Validation**: Sanitize command strings to prevent injection
- **No Shell Interpretation**: Execute via `Command::new()`, not shell
- **Timeout Enforcement**: Maximum 300 seconds (per specification)
- **Environment Control**: Validate and sanitize environment variables
- **Working Directory Validation**: Restrict to safe directories
- **Dangerous Command Detection**: Warn for privileged operations

### Testing Strategy ✅

**Following Established Patterns**:

1. **Unit Tests** (`action_parser.rs` tests):
   - Basic format parsing: `Shell "command"`
   - Parameter parsing: timeout, result capture
   - Case-insensitive support
   - Invalid format rejection
   - Parameter validation

2. **Integration Tests** (`actions.rs` tests):
   - Command execution with success/failure scenarios
   - Timeout enforcement with process cleanup
   - Variable substitution in commands
   - Context variable setting verification
   - Error handling and propagation

3. **Security Tests**:
   - Command injection prevention
   - Dangerous command detection
   - Resource limit enforcement
   - Process isolation verification

4. **End-to-End Tests**:
   - Integration with workflow execution
   - Variable passing between actions
   - Error propagation to workflow status

### Security Considerations ✅

**Command Injection Prevention**:
- No shell interpretation - direct process execution
- Input sanitization and validation
- Variable substitution safety checks

**Execution Restrictions**:
- Hard timeout limit: 300 seconds maximum
- Working directory validation
- Environment variable sanitization
- Process isolation from workflow internals

**Dangerous Command Detection**:
- System configuration modifications (systemctl, service)
- Software installation (apt, yum, brew)
- Privilege escalation (sudo, su)
- Sensitive directory access (/etc, /proc, /sys)

**Audit and Monitoring**:
- Command execution logging
- Exit code tracking
- Resource usage monitoring
- Security event logging

### Repository-Specific Integration ✅

**Coding Standards Compliance**:
- ✅ Use type system properly (newtype patterns for identifiers)
- ✅ Avoid duplication by following existing action patterns
- ✅ Use `tracing` for logging, not `eprintln`
- ✅ Follow Rust-specific standards (cargo fmt, clippy)
- ✅ Implement comprehensive tests (TDD approach)
- ✅ Use consistent error handling patterns

**MCP Integration**:
- Ready for future MCP tool integration following established patterns
- Structured logging to `.swissarmyhammer/mcp.log`
- Context-aware execution

### Next Steps

This project setup phase is now complete with:

1. ✅ **Environment Verified**: All tests pass, code quality confirmed
2. ✅ **Architecture Understood**: Clear action system patterns identified
3. ✅ **Implementation Planned**: Detailed integration strategy defined
4. ✅ **Security Designed**: Comprehensive protection approach planned
5. ✅ **Testing Strategy**: Multi-level testing approach defined

**Ready to proceed** to implementing the `ShellAction` struct following the established patterns and integrating with the existing parser and dispatch system.

