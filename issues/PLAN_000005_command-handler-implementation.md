# PLAN_000005: Command Handler Implementation

**Refer to ./specification/plan.md**

## Goal

Implement the command handler for the new `Plan` command, integrating it with the workflow execution system to properly execute the plan workflow with the specified filename parameter.

## Background

Based on the analysis in PLAN_000004, implement the actual command handling logic that connects the CLI command to the workflow execution system, passing the plan_filename parameter correctly.

## Requirements

1. Add Plan command handler to the main command dispatcher
2. Implement proper parameter passing to workflow execution
3. Add appropriate error handling for file validation
4. Follow existing patterns from other command handlers
5. Ensure async execution works correctly
6. Add basic file existence validation
7. Provide clear error messages for common issues

## Implementation Details

### Expected Handler Implementation

Based on the specification and typical patterns:

```rust
Commands::Plan { plan_filename } => {
    // Validate file exists and is readable
    if !std::path::Path::new(&plan_filename).exists() {
        return Err(Error::PlanFileNotFound(plan_filename));
    }
    
    let vars = vec![
        ("plan_filename".to_string(), plan_filename.clone())
    ];
    
    // Execute plan workflow with the filename parameter
    execute_workflow("plan", vars, Vec::new(), false, false, false, None, false).await?;
}
```

### Integration Requirements

1. **File Path Validation**
   - Check if file exists before execution
   - Validate file is readable
   - Support both relative and absolute paths
   - Provide clear error messages

2. **Parameter Passing**
   - Pass `plan_filename` as workflow variable
   - Follow existing variable passing patterns
   - Ensure proper string handling and cloning

3. **Error Handling**
   - Handle file not found errors
   - Handle workflow execution errors
   - Provide user-friendly error messages
   - Follow existing error handling patterns

4. **Async Execution**
   - Use proper async/await patterns
   - Handle async errors correctly
   - Follow existing async command patterns

## Implementation Steps

1. Locate the main command dispatcher (likely in `main.rs` or similar)
2. Find the match statement handling `Commands` enum
3. Add the `Plan` command case following existing patterns
4. Implement file existence validation
5. Set up variable vector for parameter passing
6. Call workflow execution function with correct parameters
7. Add appropriate error handling
8. Test with sample plan file

## File Structure Investigation

Need to locate and modify:
- Main command handling logic (probably `main.rs`)
- Import any necessary modules for file validation
- Ensure proper error type usage
- Follow existing logging patterns

## Acceptance Criteria

- [ ] Plan command handler added to main dispatcher
- [ ] File existence validation implemented
- [ ] Parameter passed correctly to workflow execution
- [ ] Error handling for file not found
- [ ] Error handling for workflow execution failures
- [ ] Async execution works correctly
- [ ] Command executes plan workflow with specified file
- [ ] Clear error messages for common issues

## Testing

- Test with existing plan files
- Test with non-existent files (should error clearly)
- Test with relative and absolute paths
- Verify workflow receives parameter correctly
- Confirm end-to-end execution works

## Dependencies

- Requires CLI structure from PLAN_000001
- Requires workflow updates from PLAN_000002
- Requires prompt updates from PLAN_000003
- Requires analysis from PLAN_000004

## Notes

- Follow exact patterns from existing command handlers
- Use appropriate error types from the existing system
- Ensure proper async handling
- Add logging if other commands do
- Test thoroughly with various file paths
- The handler should be simple and focused - just parameter passing and validation