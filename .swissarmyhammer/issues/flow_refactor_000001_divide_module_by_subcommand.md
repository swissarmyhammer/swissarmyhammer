# Divide Flow Command Module by Subcommand

## Problem

The flow command module is too large at **1263 lines** and contains business logic for multiple subcommands in a single file. This violates the pattern established with prompt commands and makes the code hard to maintain.

## Current Structure Issues

**Single large file**: `swissarmyhammer-cli/src/commands/flow/mod.rs` (1263 lines)

**Contains logic for multiple subcommands**:
- `flow run` - Workflow execution logic
- `flow resume` - Resume paused workflow logic  
- `flow list` - List available workflows logic
- `flow status` - Check workflow run status logic
- `flow logs` - View workflow logs logic
- `flow metrics` - View workflow metrics logic
- `flow visualize` - Generate execution visualization logic
- `flow test` - Test workflow logic

**Problems**:
- Business logic mixed with routing in single file
- Hard to find specific subcommand implementation
- Difficult to test individual subcommands in isolation
- Violates single responsibility principle
- Harder to maintain and modify specific subcommand behavior

## Target Structure

Following the prompt command pattern:

```
src/commands/flow/
├── mod.rs              # ONLY routing - no business logic
├── cli.rs              # Command definitions and parsing (if needed)
├── display.rs          # Shared display objects with Tabled derives
├── run.rs              # flow run subcommand implementation
├── resume.rs           # flow resume subcommand implementation
├── list.rs             # flow list subcommand implementation
├── status.rs           # flow status subcommand implementation
├── logs.rs             # flow logs subcommand implementation
├── metrics.rs          # flow metrics subcommand implementation
├── visualize.rs        # flow visualize subcommand implementation
├── test.rs             # flow test subcommand implementation
├── description.md      # Main flow command help (existing)
└── shared.rs           # Shared utilities used by multiple subcommands
```

## Implementation Steps

### 1. Create Subcommand Modules

**Extract each subcommand to dedicated module**:

**File**: `src/commands/flow/run.rs`
```rust
use crate::context::CliContext;
use super::shared::{load_workflow, create_executor};
use super::display::WorkflowRunResult;

pub async fn execute_run_command(
    workflow: String,
    vars: Vec<String>,
    interactive: bool,
    dry_run: bool,
    timeout: Option<String>, 
    quiet: bool,
    cli_context: &CliContext,
) -> Result<()> {
    // Move run logic from mod.rs here
    // Use cli_context for all output formatting
    // Convert any manual output to display objects
}
```

**File**: `src/commands/flow/list.rs`
```rust
use crate::context::CliContext;
use super::display::{WorkflowInfo, VerboseWorkflowInfo};

pub async fn execute_list_command(
    source: Option<PromptSource>,
    cli_context: &CliContext,
) -> Result<()> {
    let workflows = load_workflows(source)?;
    
    // Convert to display objects based on verbose flag
    if cli_context.verbose {
        let verbose_workflows: Vec<VerboseWorkflowInfo> = workflows
            .iter()
            .map(|w| w.into())
            .collect();
        cli_context.display(verbose_workflows)?;
    } else {
        let workflow_info: Vec<WorkflowInfo> = workflows
            .iter()
            .map(|w| w.into())
            .collect();
        cli_context.display(workflow_info)?;
    }
    
    Ok(())
}
```

**Similar files for**: `status.rs`, `logs.rs`, `metrics.rs`, `visualize.rs`, `test.rs`, `resume.rs`

### 2. Create Shared Utilities

**File**: `src/commands/flow/shared.rs`
```rust
// Common functions used by multiple subcommands
// Move shared utility functions from mod.rs here
pub fn load_workflows(source: Option<PromptSource>) -> Result<Vec<Workflow>> { ... }
pub fn create_executor() -> Result<WorkflowExecutor> { ... }
pub fn parse_timeout(timeout_str: &str) -> Result<Duration> { ... }
```

### 3. Create Display Objects

**File**: `src/commands/flow/display.rs`
```rust
use tabled::Tabled;
use serde::Serialize;

#[derive(Tabled, Serialize)]
pub struct WorkflowInfo {
    #[tabled(rename = "Workflow")]
    pub name: String,
    #[tabled(rename = "Description")]
    pub description: String,
}

#[derive(Tabled, Serialize)]
pub struct VerboseWorkflowInfo {
    #[tabled(rename = "Workflow")]
    pub name: String,
    #[tabled(rename = "Title")]
    pub title: String,
    #[tabled(rename = "Description")]
    pub description: String,
    #[tabled(rename = "Actions")]
    pub action_count: String,
}

#[derive(Tabled, Serialize)]
pub struct WorkflowRunStatus {
    #[tabled(rename = "Run ID")]
    pub run_id: String,
    #[tabled(rename = "Workflow")]
    pub workflow: String,
    #[tabled(rename = "Status")]
    pub status: String,
    #[tabled(rename = "Started")]
    pub started: String,
}

// Additional display objects for metrics, logs, etc.
```

### 4. Simplify mod.rs to Pure Routing

**File**: `src/commands/flow/mod.rs`

```rust
pub mod cli;
pub mod display;
pub mod run;
pub mod resume;
pub mod list;
pub mod status;
pub mod logs;
pub mod metrics;
pub mod visualize;
pub mod test;
pub mod shared;

use crate::context::CliContext;
use crate::cli::FlowSubcommand;
use crate::exit_codes::EXIT_SUCCESS;

/// Handle flow command - PURE ROUTING ONLY
pub async fn handle_command(subcommand: FlowSubcommand, context: &CliContext) -> i32 {
    let result = match subcommand {
        FlowSubcommand::Run { workflow, vars, interactive, dry_run, timeout, quiet } => {
            run::execute_run_command(workflow, vars, interactive, dry_run, timeout, quiet, context).await
        }
        FlowSubcommand::Resume { run_id, interactive } => {
            resume::execute_resume_command(run_id, interactive, context).await
        }
        FlowSubcommand::List { source } => {
            list::execute_list_command(source, context).await
        }
        FlowSubcommand::Status { run_id } => {
            status::execute_status_command(run_id, context).await
        }
        FlowSubcommand::Logs { run_id, follow } => {
            logs::execute_logs_command(run_id, follow, context).await
        }
        FlowSubcommand::Metrics { format } => {
            metrics::execute_metrics_command(context).await
        }
        FlowSubcommand::Visualize { run_id, format, output } => {
            visualize::execute_visualize_command(run_id, format, output, context).await
        }
        FlowSubcommand::Test { workflow, vars } => {
            test::execute_test_command(workflow, vars, context).await
        }
    };

    match result {
        Ok(_) => EXIT_SUCCESS,
        Err(e) => {
            eprintln!("Flow command failed: {}", e);
            1
        }
    }
}

// NO business logic here - only routing and error handling
```

## Benefits

### For Developers
- **Single Responsibility**: Each module handles one subcommand
- **Easier Navigation**: Find list logic in list.rs, run logic in run.rs
- **Better Testing**: Test subcommands independently
- **Simpler Maintenance**: Changes isolated to specific modules
- **Consistent Pattern**: Same structure as cleaned-up prompt commands

### For Architecture  
- **Proper Separation**: Business logic separate from routing
- **Reusable Components**: Shared utilities in dedicated module
- **Scalable Design**: Easy to add new subcommands
- **Consistent Structure**: All command modules follow same pattern

### For Code Quality
- **Readable Code**: Smaller, focused files easier to understand
- **Maintainable**: Changes don't require navigating huge files
- **Testable**: Unit tests for specific subcommand logic
- **Organized**: Clear file organization reflects command structure

## Success Criteria

1. ✅ mod.rs contains only routing logic (< 100 lines)
2. ✅ Each subcommand has dedicated module with implementation
3. ✅ Shared utilities extracted to shared.rs
4. ✅ Display objects provide structured output for all subcommands
5. ✅ All existing functionality preserved
6. ✅ Global arguments work with all subcommands
7. ✅ Consistent output formatting across all flow operations

## Files Created

- `src/commands/flow/run.rs` - Run subcommand implementation
- `src/commands/flow/resume.rs` - Resume subcommand implementation  
- `src/commands/flow/list.rs` - List subcommand implementation
- `src/commands/flow/status.rs` - Status subcommand implementation
- `src/commands/flow/logs.rs` - Logs subcommand implementation
- `src/commands/flow/metrics.rs` - Metrics subcommand implementation
- `src/commands/flow/visualize.rs` - Visualize subcommand implementation
- `src/commands/flow/test.rs` - Test subcommand implementation
- `src/commands/flow/shared.rs` - Shared utilities
- `src/commands/flow/display.rs` - Display objects

## Files Modified

- `src/commands/flow/mod.rs` - Remove business logic, keep routing only

---

**Priority**: Medium - Code organization and maintainability
**Estimated Effort**: Large (significant refactoring of 1263 lines)
**Dependencies**: None (refactoring existing code)
**Benefits**: Better organization, easier maintenance, consistent patterns