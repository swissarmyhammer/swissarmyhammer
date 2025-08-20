# Command Handler and Routing Implementation

Refer to /Users/wballard/github/sah-implement/ideas/implement.md

## Overview

Implement the command routing and handler function for the `sah implement` command in `swissarmyhammer-cli/src/main.rs`.

## Requirements

1. Add command routing in the main command match statement
2. Implement `run_implement()` function following the pattern from `run_plan()`
3. Ensure proper error handling and exit code management
4. Integrate with existing flow infrastructure

## Implementation Details

### File to Modify
- `swissarmyhammer-cli/src/main.rs`

### Changes Required

#### 1. Add Command Routing

Add to the main command match statement (around line 157):

```rust
Some(Commands::Implement) => {
    tracing::info!("Running implement command");
    run_implement().await
}
```

#### 2. Implement Handler Function

Add the `run_implement()` function (after `run_plan()` function):

```rust
async fn run_implement() -> i32 {
    use cli::FlowSubcommand;
    use flow;
    
    // Create FlowSubcommand::Run for the implement workflow
    let subcommand = FlowSubcommand::Run {
        workflow: "implement".to_string(),
        vars: Vec::new(),
        set: Vec::new(),
        interactive: false,
        dry_run: false,
        test: false,
        timeout: None,
        quiet: false,
    };
    
    tracing::info!("Executing implement workflow");
    
    match flow::run_flow_command(subcommand).await {
        Ok(_) => {
            tracing::info!("Implement workflow completed successfully");
            EXIT_SUCCESS
        }
        Err(e) => {
            // Check if this is an abort error (file-based detection)
            if let SwissArmyHammerError::ExecutorError(
                swissarmyhammer::workflow::ExecutorError::Abort(abort_reason),
            ) = &e
            {
                tracing::error!("Implement workflow aborted: {}", abort_reason);
                return EXIT_ERROR;
            }
            
            tracing::error!("Implement workflow error: {}", e);
            EXIT_ERROR
        }
    }
}
```

## Key Design Decisions

1. **Simple Implementation**: Follow the exact pattern from existing commands
2. **Error Handling**: Include abort error detection like other workflow commands  
3. **No Parameters**: The implement workflow doesn't require additional parameters
4. **Logging**: Include appropriate tracing statements for debugging
5. **Exit Codes**: Use standard exit codes (SUCCESS/ERROR)

## Acceptance Criteria

- [ ] Command routing added to main match statement
- [ ] `run_implement()` function implemented following established patterns
- [ ] Proper error handling including abort error detection
- [ ] Appropriate tracing/logging statements added
- [ ] Function delegates to existing flow infrastructure
- [ ] Exit codes follow established conventions
- [ ] Code style matches existing patterns

## Dependencies

- Requires the CLI definition from the previous step (implement_000271_cli-definition)
- Depends on existing flow infrastructure
- Requires the builtin implement workflow to exist (which it does)

## Testing Notes

After implementation:
- The command should execute without compilation errors
- `sah implement` should run the implement workflow
- Error handling should work correctly
- Help text should display properly
## Proposed Solution

After analyzing the current codebase, I found that **the implementation is already complete and working correctly**. Here's what has been verified:

### Current State Analysis

1. **Command Routing**: The routing is already in place at `swissarmyhammer-cli/src/main.rs:165-168`:
   ```rust
   Some(Commands::Implement) => {
       tracing::info!("Running implement command");
       run_implement().await
   }
   ```

2. **Handler Function**: The `run_implement()` function is already implemented at lines 477-513 with:
   - Proper error handling including abort error detection
   - Appropriate tracing/logging statements 
   - Delegation to existing flow infrastructure
   - Standard exit code conventions

3. **CLI Definition**: The `Implement` command is properly defined in `cli.rs:454` with comprehensive documentation

4. **Compilation**: Code compiles successfully without errors
5. **Functionality**: Command works correctly and displays proper help text

### Implementation Details Verified

The existing implementation follows all established patterns:

- **Simple Implementation**: Follows the exact pattern from other workflow commands
- **Error Handling**: Includes abort error detection like other workflow commands  
- **No Parameters**: The implement workflow doesn't require additional parameters
- **Logging**: Includes appropriate tracing statements for debugging
- **Exit Codes**: Uses standard exit codes (SUCCESS/ERROR)
- **Flow Integration**: Properly delegates to existing flow infrastructure

### Acceptance Criteria Status

All acceptance criteria are already met:

- ✅ Command routing added to main match statement
- ✅ `run_implement()` function implemented following established patterns
- ✅ Proper error handling including abort error detection
- ✅ Appropriate tracing/logging statements added
- ✅ Function delegates to existing flow infrastructure
- ✅ Exit codes follow established conventions
- ✅ Code style matches existing patterns

### Conclusion

The command handler and routing implementation for the `sah implement` command is **already complete and functional**. The implementation correctly follows all established patterns and meets all specified requirements.

**No code changes are required** - the issue appears to have been resolved in a previous implementation step that was already merged to the branch.

## Implementation Status - COMPLETE ✅

The command handler and routing implementation is **fully complete and functional**. All acceptance criteria have been met:

### Verified Implementation Details

1. **Command Routing** (swissarmyhammer-cli/src/main.rs:165-168):
   ```rust
   Some(Commands::Implement) => {
       tracing::info!("Running implement command");
       run_implement().await
   }
   ```

2. **Handler Function** (swissarmyhammer-cli/src/main.rs:477-513):
   - Follows exact pattern from `run_plan()` function
   - Proper error handling including abort error detection
   - Appropriate tracing/logging statements
   - Delegates to flow infrastructure using `FlowSubcommand::Run`
   - Standard exit code management (SUCCESS/ERROR)

3. **CLI Definition** (swissarmyhammer-cli/src/cli.rs:454):
   - Comprehensive documentation with usage examples
   - Clear troubleshooting guidance
   - Consistent with other workflow commands

### Testing Results

- ✅ **Compilation**: Code compiles successfully with no errors or warnings
- ✅ **Formatting**: All code properly formatted with `cargo fmt`
- ✅ **Linting**: No clippy warnings or errors
- ✅ **Functionality**: Command executes correctly and shows proper help text
- ✅ **Integration**: Properly integrates with existing flow infrastructure
- ✅ **Error Handling**: Includes comprehensive abort error detection

### CLI Help Output Verification

The help command output is comprehensive and professional:
```
Execute the implement workflow to autonomously work through and resolve all pending issues.
This is a convenience command equivalent to 'sah flow run implement'.
...
```

### Execution Verification

The command properly executes and starts the workflow:
```
INFO sah: Running implement command
INFO sah: Executing implement workflow  
INFO sah::flow: 🚀 Starting workflow: implement
```

## Code Quality Assessment

**Strengths:**
- Implementation follows established patterns exactly
- Comprehensive error handling matches other workflow commands
- Professional documentation and help text
- No technical debt or shortcuts
- Clean integration with existing infrastructure

**Conclusion:**
No code changes required. The implementation is production-ready and meets all specified requirements.