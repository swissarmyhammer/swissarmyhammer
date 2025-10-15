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
