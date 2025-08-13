# PLAN_000004: Flow Integration Analysis

**Refer to ./specification/plan.md**

## Goal

Analyze the flow execution system in `swissarmyhammer-cli/src/flow.rs` to understand how to properly integrate the new plan command with the existing workflow execution infrastructure.

## Background

Before implementing the command handler, we need to understand how the existing flow system works, particularly how commands are mapped to workflow executions and how parameters are passed from CLI to workflows.

## Requirements

1. Examine `swissarmyhammer-cli/src/flow.rs` for execution patterns
2. Understand how other CLI commands trigger workflow execution
3. Identify the function signature for workflow execution
4. Document parameter passing mechanisms
5. Understand error handling patterns
6. Identify where to add the Plan command handler

## Investigation Tasks

### 1. Flow Execution Analysis

- Read `swissarmyhammer-cli/src/flow.rs` completely
- Identify main workflow execution function
- Document function signature and parameters
- Understand how variables are passed to workflows

### 2. Existing Command Handler Patterns

- Find examples of CLI commands that execute workflows
- Document the pattern used for parameter passing
- Identify error handling approaches
- Understand return value handling

### 3. Variable Passing System

- Understand how CLI arguments become workflow variables
- Document the format for variable passing (Vec<(String, String)>?)
- Identify any validation or transformation logic
- Check for templating variable support

### 4. Integration Points

- Identify where command handlers are typically placed
- Find the main command router/dispatcher
- Understand the async execution pattern
- Document any common imports or dependencies needed

## Expected Findings

Based on the specification, we expect to find:

```rust
// Expected function signature (to be confirmed)
execute_workflow(
    workflow_name: &str, 
    vars: Vec<(String, String)>, 
    // ... other parameters
) -> Result<(), Error>
```

And usage pattern like:

```rust
Commands::Plan { plan_filename } => {
    let vars = vec![
        ("plan_filename".to_string(), plan_filename.clone())
    ];
    
    execute_workflow("plan", vars, /* other args */).await?;
}
```

## Implementation Steps

1. Read and analyze `swissarmyhammer-cli/src/flow.rs`
2. Look for the main command handling logic (likely in `main.rs` or similar)
3. Document the workflow execution function signature
4. Find examples of other command handlers
5. Create documentation of integration patterns
6. Prepare implementation plan for PLAN_000005

## Deliverables

1. Complete analysis of flow execution system
2. Documented function signatures and patterns  
3. Example code from existing command handlers
4. Clear implementation plan for Plan command handler
5. Understanding of error handling requirements

## Acceptance Criteria

- [ ] Flow execution system fully understood
- [ ] Workflow execution function identified and documented
- [ ] Parameter passing mechanism documented
- [ ] Existing command handler patterns identified
- [ ] Error handling approach understood
- [ ] Clear path forward for implementation identified

## Dependencies

- Requires understanding from previous steps
- Foundation for PLAN_000005 implementation

## Analysis Results

### Key Finding: Plan Command Already Implemented!

During analysis, I discovered that the Plan command has already been fully implemented in the codebase:

#### 1. CLI Structure (swissarmyhammer-cli/src/cli.rs:363-384)

The Plan command is already defined in the Commands enum:

```rust
/// Plan a specific specification file
#[command(long_about = "
Execute planning workflow for a specific specification file.
Takes a path to a markdown specification file and generates implementation steps.

Basic usage:
  swissarmyhammer plan <plan_filename>    # Plan specific file

The planning workflow will:
- Read the specified plan file
- Generate step-by-step implementation issues
- Create numbered issue files in ./issues directory

Examples:
  swissarmyhammer plan ./specification/new-feature.md
  swissarmyhammer plan /path/to/custom-plan.md
  swissarmyhammer plan plans/database-migration.md
")]
Plan {
    /// Path to the plan file to process
    plan_filename: String,
},
```

#### 2. Command Handler Implementation (swissarmyhammer-cli/src/main.rs:169-172, 373-471)

The main.rs file contains both the command routing and the complete implementation:

```rust
// Command routing in main()
Some(Commands::Plan { plan_filename }) => {
    tracing::info!("Running plan command for file: {}", plan_filename);
    run_plan(plan_filename).await
}

// Complete implementation in run_plan()
async fn run_plan(plan_filename: String) -> i32 {
    // File validation
    // Workflow storage creation
    // Workflow execution with template variables
    // Error handling and status reporting
}
```

#### 3. Workflow Execution Pattern

The implementation follows the exact pattern identified in the analysis:

1. **File Validation**: Checks if plan file exists
2. **Workflow Storage**: Creates `WorkflowStorage::file_system()`
3. **Workflow Loading**: Loads the "plan" workflow by name
4. **Executor Creation**: Creates `WorkflowExecutor::new()`
5. **Workflow Run**: Uses `executor.start_workflow()` and `executor.execute_state()`
6. **Parameter Passing**: Uses template variables via `_template_vars` context key
7. **Error Handling**: Returns appropriate exit codes (SUCCESS, ERROR)

#### 4. Parameter Passing Mechanism

The plan_filename is passed as a template variable:

```rust
let mut set_variables = HashMap::new();
set_variables.insert(
    "plan_filename".to_string(),
    serde_json::Value::String(plan_filename),
);

run.context.insert(
    "_template_vars".to_string(),
    serde_json::to_value(set_variables).unwrap_or(serde_json::Value::Object(Default::default())),
);
```

#### 5. Error Handling Pattern

Consistent error handling with:
- File existence validation
- Storage creation error handling  
- Workflow loading error handling
- Execution error handling
- Final status checking with appropriate exit codes

### Implications for Next Steps

Since the Plan command is fully implemented:

1. **PLAN_000001** (CLI structure) - ✅ Already completed
2. **PLAN_000004** (Integration analysis) - ✅ Analysis shows complete implementation
3. **PLAN_000005** (Command handler) - ✅ Already implemented

The remaining steps should focus on:
- Testing the existing implementation
- Validating the "plan" workflow exists
- Documentation updates if needed
- Integration testing

## Notes

- This is an analysis step, not implementation
- Document findings thoroughly for next step
- Look for async/await patterns
- Pay attention to error types and handling
- Identify any logging or debug patterns to follow