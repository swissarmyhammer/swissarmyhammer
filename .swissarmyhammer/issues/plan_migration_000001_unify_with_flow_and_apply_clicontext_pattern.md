# Migrate Plan Command to Use FlowSubcommand::Run and CliContext Pattern

## Overview

Migrate the `sah plan` command to follow the established CliContext pattern and eliminate code duplication by using `FlowSubcommand::Run` like the `implement` command does.

## Current Plan Command Issues

### 1. Uses TemplateContext Instead of CliContext
**Current signature**:
```rust
pub async fn handle_command(
    plan_filename: String,
    _template_context: &TemplateContext,
) -> i32
```

**Problems**:
- No support for global `--verbose` and `--format` arguments
- Inconsistent with other migrated commands (doctor, implement, flow)
- Cannot use CliContext display methods

### 2. Duplicates Flow Command Logic
**Current implementation**:
- Manual workflow loading from `FileSystemWorkflowStorage`
- Manual `WorkflowExecutor` creation and configuration
- Manual variable setting and workflow execution
- Manual error handling and exit code management

**This logic is identical to what `FlowSubcommand::Run` already does!**

### 3. Inconsistent Architecture
- `implement` command uses `FlowSubcommand::Run` (correct pattern)
- `plan` command duplicates the same workflow execution logic (wrong)
- Same underlying operation implemented twice

## Solution: Follow Implement Command Pattern

### Current Implement Command (Good Pattern)
```rust
pub async fn handle_command(context: &CliContext) -> i32 {
    // Execute the implement workflow - equivalent to 'flow run implement'
    let subcommand = FlowSubcommand::Run {
        workflow: "implement".to_string(),
        vars: vec![],
        interactive: false,
        dry_run: false,
        timeout: None,
        quiet: context.quiet,
    };

    crate::commands::flow::handle_command(subcommand, context).await
}
```

### Target Plan Command (Apply Same Pattern)
```rust
pub async fn handle_command(
    plan_filename: String,
    cli_context: &CliContext,
) -> i32 {
    // Validate plan file first (existing logic)
    let validated_file = match validate_plan_file_comprehensive(&plan_filename, None) {
        Ok(file) => file,
        Err(e) => {
            eprintln!("Plan file validation failed: {}", e);
            return EXIT_ERROR;
        }
    };

    if cli_context.verbose {
        println!("Executing plan workflow with file: {}", validated_file.path.display());
    }

    // Execute the plan workflow using FlowSubcommand::Run
    let subcommand = FlowSubcommand::Run {
        workflow: "plan".to_string(),
        vars: vec![format!("plan_filename={}", validated_file.path.display())],
        interactive: false,
        dry_run: false,
        timeout: None,
        quiet: !cli_context.verbose,  // Use global verbose setting
    };

    crate::commands::flow::handle_command(subcommand, cli_context).await
}
```

## Implementation Steps

### 1. Update Command Signature

**File**: `swissarmyhammer-cli/src/commands/plan/mod.rs`

**Changes**:
- Replace `TemplateContext` parameter with `CliContext`
- Keep `plan_filename: String` parameter (plan-specific)
- Remove `_template_context` prefix since we'll use cli_context

### 2. Replace Workflow Execution Logic

**Remove duplicate code**:
- Delete manual `FileSystemWorkflowStorage` creation
- Delete manual `WorkflowExecutor` setup and execution
- Delete manual variable setting and error handling
- Delete all workflow-related imports

**Replace with FlowSubcommand::Run**:
- Use `FlowSubcommand::Run` with workflow name "plan"
- Pass `plan_filename` as a workflow variable
- Delegate to existing `flow::handle_command()`

### 3. Preserve Plan-Specific Logic

**Keep plan validation**:
- Preserve `validate_plan_file_comprehensive()` call
- Plan file validation is plan-specific and should remain
- Only the workflow execution should be delegated

**Add CliContext integration**:
- Use `cli_context.verbose` for debug output
- Plan command will automatically support global `--format` for any output

### 4. Update Main.rs Integration

**File**: `swissarmyhammer-cli/src/main.rs`

**Change from**:
```rust
commands::plan::handle_command(plan_filename, template_context).await
```

**Change to**:
```rust
commands::plan::handle_command(plan_filename, &cli_context).await
```

## Benefits

### For Code Quality
- **Eliminate Duplication**: No more duplicate workflow execution logic
- **Consistent Architecture**: Same CliContext pattern as other commands
- **Reuse Flow Logic**: Leverage existing, tested workflow execution code
- **Simpler Maintenance**: Changes to workflow execution only need to be made in flow command

### For Users  
- **Global Arguments**: `sah --verbose plan myfile.md` shows detailed execution
- **Consistent Output**: Same workflow execution output as `sah flow run plan`
- **Better Error Handling**: Inherits robust error handling from flow command
- **Unified Experience**: Plan behaves like other workflow commands

### For Architecture
- **Pattern Completion**: Fourth command using CliContext pattern
- **Code Reuse**: Demonstrates proper delegation to avoid duplication
- **Consistency**: All workflow-related commands use same execution path

## Expected Behavior

### Before and After Commands
```bash
# Current behavior (preserved)
sah plan specification/my-plan.md

# New global argument support
sah --verbose plan specification/my-plan.md    # Shows detailed workflow execution
sah --format=json plan specification/my-plan.md # JSON output if workflow produces any
```

### Workflow Execution
- Plan file validation happens first (plan-specific)
- Workflow execution delegated to flow command (shared logic)
- Plan filename passed as workflow variable
- Same error handling and exit codes as `sah flow run plan`

## Success Criteria

1. ✅ `sah plan myfile.md` works exactly as before
2. ✅ `sah --verbose plan myfile.md` shows detailed workflow execution
3. ✅ Global `--format` argument works if plan workflow produces output
4. ✅ No duplicate workflow execution logic in plan command
5. ✅ Plan file validation preserved and working
6. ✅ Same error handling and exit codes as current implementation
7. ✅ Uses CliContext pattern consistently with other commands

## Files Modified

- `swissarmyhammer-cli/src/commands/plan/mod.rs` - CliContext integration, remove duplicate logic
- `swissarmyhammer-cli/src/main.rs` - Pass CliContext instead of TemplateContext

## Removed Code

- Manual `FileSystemWorkflowStorage` creation
- Manual `WorkflowExecutor` setup and execution  
- Manual variable setting and workflow management
- Duplicate error handling logic
- Workflow-related imports

---

**Priority**: Medium - Eliminates code duplication and completes CliContext pattern
**Estimated Effort**: Small (mostly deletion + delegation)
**Dependencies**: cli_prompt_000001_add_global_format_argument (for CliContext)
**Benefits**: Code reuse, consistency, global argument support