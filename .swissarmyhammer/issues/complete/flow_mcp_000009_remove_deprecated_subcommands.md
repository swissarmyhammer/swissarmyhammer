# Step 9: Remove Deprecated Flow Subcommands

Refer to ideas/flow_mcp.md

## Objective

Remove unused flow subcommands (resume, status, logs, test) and ensure flow command only handles workflow execution and listing.

## Context

The spec indicates that resume, status, logs, and test subcommands should be removed. The new design focuses on flow taking workflow name directly as first positional, with "list" as a special case.

## Tasks

### 1. Remove Subcommand Handlers

Delete the following files:
- `swissarmyhammer-cli/src/commands/flow/resume.rs`
- `swissarmyhammer-cli/src/commands/flow/status.rs`
- `swissarmyhammer-cli/src/commands/flow/logs.rs`
- `swissarmyhammer-cli/src/commands/flow/test.rs`

### 2. Simplify Flow Command Structure

Update `swissarmyhammer-cli/src/cli.rs` to remove subcommand enum entirely:

```rust
// FlowSubcommand enum is REMOVED
// Flow command now takes workflow name directly as first positional

// Flow command is parsed into FlowCommand struct directly
#[derive(Debug, Clone)]
pub struct FlowCommand {
    pub workflow_name: String,
    pub positional_args: Vec<String>,
    pub params: Vec<String>,
    pub vars: Vec<String>,
    pub format: Option<String>,
    pub verbose: bool,
    pub source: Option<String>,
    pub interactive: bool,
    pub dry_run: bool,
    pub quiet: bool,
}
```

### 3. Update Flow Command Handler

Update `swissarmyhammer-cli/src/commands/flow/mod.rs`:

```rust
pub mod display;
pub mod list;
pub mod shared;

// REMOVED: resume, status, logs, test modules
// REMOVED: run module (functionality merged into handler)

use crate::cli::FlowCommand;
use crate::context::CliContext;

pub async fn handle_command(cmd: FlowCommand, context: &CliContext) -> i32 {
    let result = if cmd.workflow_name == "list" {
        // Special case: list workflows
        list::execute_list_command(
            cmd.format.unwrap_or_else(|| "json".to_string()),
            cmd.verbose,
            cmd.source,
            context,
        ).await
    } else {
        // Regular case: execute workflow
        execute_workflow(cmd, context).await
    };

    match result {
        Ok(_) => EXIT_SUCCESS,
        Err(e) => {
            eprintln!("Flow command failed: {}", e);
            EXIT_ERROR
        }
    }
}

async fn execute_workflow(
    cmd: FlowCommand,
    context: &CliContext,
) -> Result<()> {
    // Get workflow definition
    let workflow_def = context.workflow_storage
        .get_workflow(&WorkflowName::new(&cmd.workflow_name))?;
    
    // Map positional args to required parameters
    let mut all_params = map_positional_to_params(
        &workflow_def,
        cmd.positional_args,
    )?;
    
    // Add --param and --var values
    for param in cmd.params {
        let (key, value) = parse_key_value(&param)?;
        all_params.insert(key, value);
    }
    
    for var in cmd.vars {
        let (key, value) = parse_key_value(&var)?;
        all_params.insert(key, value);
    }
    
    // Execute workflow
    execute_workflow_impl(
        &cmd.workflow_name,
        all_params,
        cmd.interactive,
        cmd.dry_run,
        cmd.quiet,
        context,
    ).await
}
```

### 4. Remove Subcommands from CLI Parser

The CLI parser is already updated in issue 000005 to not have subcommands.

### 5. Update Tests

Remove or update tests that depend on deleted subcommands:
- Remove tests for resume command
- Remove tests for status command
- Remove tests for logs command
- Remove tests for test command
- Update integration tests that may have used these commands

### 6. Update Documentation

Update any documentation references to removed subcommands:
- Update command help text
- Update README or docs if they mention these commands
- Add notes about removal in changelog

## Files to Delete

- `swissarmyhammer-cli/src/commands/flow/resume.rs`
- `swissarmyhammer-cli/src/commands/flow/status.rs`
- `swissarmyhammer-cli/src/commands/flow/logs.rs`
- `swissarmyhammer-cli/src/commands/flow/test.rs`
- `swissarmyhammer-cli/src/commands/flow/run.rs` (merged into handler)

## Files to Modify

- `swissarmyhammer-cli/src/cli.rs`
- `swissarmyhammer-cli/src/commands/flow/mod.rs`
- `swissarmyhammer-cli/src/main.rs` (CLI parser)
- `swissarmyhammer-cli/tests/*` (remove related tests)

## Acceptance Criteria

- [ ] Resume, status, logs, test subcommands removed
- [ ] FlowSubcommand enum removed
- [ ] Flow command takes workflow name directly (no "run" subcommand)
- [ ] `sah flow plan spec.md` works
- [ ] `sah flow list` works
- [ ] Tests updated or removed
- [ ] Code compiles without warnings
- [ ] No broken references to removed commands

## Estimated Changes

~-300 lines of code (deletions)
~50 lines of code (updates)



## Proposed Solution

This refactoring will simplify the flow command structure by removing deprecated subcommands (resume, status, logs, test) and keeping only the core workflow execution and listing capabilities.

### Analysis

The current code has:
- **FlowSubcommand enum** with 6 variants: Execute, Resume, List, Status, Logs, Test
- **4 deprecated command files**: resume.rs, status.rs, logs.rs, test.rs
- **Complex parsing logic** in parse_flow_args() that handles special cases for each deprecated command
- **Tests** for deprecated functionality in cli.rs

According to the spec (ideas/flow_mcp.md), the new design should:
- Remove resume, status, logs, and test subcommands entirely
- Keep only Execute (for direct workflow execution) and List (for workflow discovery)
- Simplify the CLI to: `sah flow <workflow> [args...] [--param k=v]` or `sah flow list`

### Implementation Steps

#### Step 1: Delete Deprecated Command Files
Remove the following files:
- `swissarmyhammer-cli/src/commands/flow/resume.rs`
- `swissarmyhammer-cli/src/commands/flow/status.rs`
- `swissarmyhammer-cli/src/commands/flow/logs.rs`
- `swissarmyhammer-cli/src/commands/flow/test.rs`

#### Step 2: Update FlowSubcommand Enum in cli.rs
Remove the deprecated variants from the enum, keeping only:
```rust
#[derive(Subcommand, Debug)]
pub enum FlowSubcommand {
    Execute { ... },
    List { ... },
}
```

Remove these variants:
- Resume
- Status
- Logs
- Test

#### Step 3: Update flow/mod.rs Module Declarations
Remove module declarations for deprecated commands:
```rust
// Remove these lines:
pub mod logs;
pub mod resume;
pub mod status;
pub mod test;
```

Update the handle_command function to only handle Execute and List variants.

#### Step 4: Update parse_flow_args Function
Remove the special case parsing for:
- "resume" command
- "status" command
- "logs" command
- "test" command

Keep only:
- "list" special case (returns List variant)
- Default case (returns Execute variant for any other workflow name)

This simplifies the parser significantly - any non-"list" first argument is treated as a workflow name.

#### Step 5: Clean Up Tests
Remove all tests in cli.rs that reference deprecated subcommands:
- test_cli_flow_test_subcommand
- test_cli_flow_test_subcommand_with_options
- Any other tests that might reference resume, status, or logs

#### Step 6: Verification
- Run `cargo build` to ensure compilation succeeds
- Run `cargo nextest run` to ensure all tests pass
- Run `cargo fmt` to format code
- Run `cargo clippy` to check for warnings

### Benefits

1. **Simpler Code**: Removes ~300 lines of unused code
2. **Clearer Intent**: The flow command now clearly does two things: execute workflows or list them
3. **Easier Maintenance**: Fewer moving parts to maintain
4. **Aligned with Spec**: Matches the design in ideas/flow_mcp.md for the unified flow tool approach

### Risks and Mitigations

**Risk**: Breaking changes for users who rely on these commands
**Mitigation**: These commands were marked for removal in the spec, indicating they're not part of the final design. If users need this functionality, it should be rebuilt properly in the future.

**Risk**: Tests might fail due to missing functionality
**Mitigation**: Review test failures carefully and update or remove tests as appropriate. The goal is to ensure the remaining Execute and List functionality works correctly.

### Success Criteria

- [ ] All 4 deprecated command files deleted
- [ ] FlowSubcommand enum reduced to 2 variants (Execute, List)
- [ ] parse_flow_args simplified to handle only "list" special case
- [ ] handle_command only routes Execute and List
- [ ] Code compiles without errors
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] `sah flow <workflow>` executes workflows
- [ ] `sah flow list` lists workflows

