# Step 6: Implement Dynamic Shortcut Generation

Refer to ideas/flow_mcp.md

## Objective

Generate top-level CLI commands dynamically for each workflow, with proper conflict resolution.

## Context

Each workflow should get a convenient top-level shortcut command (e.g., `sah plan spec.md` instead of `sah flow plan spec.md`). We need to detect name conflicts with reserved commands and add underscore prefix.

## Tasks

### 1. Create Shortcut Generator Module

Create `swissarmyhammer-cli/src/shortcuts/flow_shortcuts.rs`:

```rust
use clap::Command;
use std::collections::HashSet;

const RESERVED_NAMES: &[&str] = &[
    "list",  // Special case in flow command
    "flow", "agent", "prompt", "serve", "doctor", "rule", "validate",  // Top-level
];

pub fn generate_workflow_shortcuts(
    workflow_storage: &WorkflowStorage,
) -> Vec<Command> {
    let mut shortcuts = Vec::new();
    let workflows = match workflow_storage.list_workflows() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Warning: Failed to load workflows for shortcuts: {}", e);
            return shortcuts;
        }
    };

    let reserved: HashSet<&str> = RESERVED_NAMES.iter().copied().collect();

    for workflow in workflows {
        let workflow_name = workflow.name.to_string();
        
        // Apply conflict resolution
        let command_name = if reserved.contains(workflow_name.as_str()) {
            format!("_{}", workflow_name)
        } else {
            workflow_name.clone()
        };

        let cmd = build_shortcut_command(
            command_name,
            &workflow_name,
            &workflow,
        );
        
        shortcuts.push(cmd);
    }

    shortcuts
}
```

### 2. Build Shortcut Command

```rust
fn build_shortcut_command(
    command_name: String,
    workflow_name: &str,
    workflow: &Workflow,
) -> Command {
    let mut cmd = Command::new(command_name)
        .about(format!(
            "{} (shortcut for 'flow {}')",
            workflow.description,
            workflow_name
        ));
    
    // Add positional args for required parameters
    let required_params: Vec<_> = workflow.parameters
        .iter()
        .filter(|p| p.required)
        .collect();
    
    if !required_params.is_empty() {
        cmd = cmd.arg(
            Arg::new("positional")
                .num_args(required_params.len())
                .value_names(
                    required_params.iter().map(|p| p.name.as_str())
                )
                .help("Required parameters")
        );
    }
    
    // Add optional parameter flag
    cmd = cmd.arg(
        Arg::new("param")
            .long("param")
            .short('p')
            .action(ArgAction::Append)
            .value_name("KEY=VALUE")
            .help("Optional workflow parameter")
    );
    
    // Add standard flags
    cmd = cmd
        .arg(Arg::new("interactive").long("interactive").short('i').action(ArgAction::SetTrue))
        .arg(Arg::new("dry_run").long("dry-run").action(ArgAction::SetTrue))
        .arg(Arg::new("quiet").long("quiet").short('q').action(ArgAction::SetTrue));
    
    cmd
}
```

### 3. Integrate Shortcuts into CLI

Update `swissarmyhammer-cli/src/main.rs`:

```rust
use shortcuts::flow_shortcuts::generate_workflow_shortcuts;

fn build_cli(context: &CliContext) -> Command {
    let mut app = Command::new("sah")
        .version(VERSION)
        .about("SwissArmyHammer - The only coding assistant you'll ever need")
        // ... standard subcommands
        ;
    
    // Add dynamic workflow shortcuts
    let workflow_shortcuts = generate_workflow_shortcuts(&context.workflow_storage);
    for shortcut in workflow_shortcuts {
        app = app.subcommand(shortcut);
    }
    
    app
}
```

### 4. Add Shortcut Handler

Update command dispatcher to handle shortcuts:

```rust
async fn handle_shortcut_command(
    workflow_name: String,
    matches: &ArgMatches,
    context: &CliContext,
) -> i32 {
    let positional_args: Vec<String> = matches
        .get_many::<String>("positional")
        .map(|vals| vals.cloned().collect())
        .unwrap_or_default();
    
    let params: Vec<String> = matches
        .get_many::<String>("param")
        .map(|vals| vals.cloned().collect())
        .unwrap_or_default();
    
    // Delegate to flow handler
    let cmd = FlowCommand {
        workflow_name,
        positional_args,
        params,
        vars: vec![],  // Shortcuts don't support deprecated --var
        format: None,
        verbose: false,
        source: None,
        interactive: matches.get_flag("interactive"),
        dry_run: matches.get_flag("dry_run"),
        quiet: matches.get_flag("quiet"),
    };
    
    match handle_flow_command(cmd, context).await {
        Ok(_) => EXIT_SUCCESS,
        Err(e) => {
            eprintln!("Workflow failed: {}", e);
            EXIT_ERROR
        }
    }
}
```

### 5. Add Tests

```rust
#[test]
fn test_shortcut_generation() {
    // Test shortcuts are generated for all workflows
}

#[test]
fn test_name_conflict_resolution() {
    // Test reserved names get underscore prefix
}

#[test]
fn test_shortcut_positional_args() {
    // Test positional args work in shortcuts
}

#[test]
fn test_shortcut_execution() {
    // Test shortcut delegates to flow handler correctly
}

#[test]
fn test_shortcut_about_text() {
    // Test about text shows "(shortcut for 'flow <name>')"
}
```

## Files to Create/Modify

- `swissarmyhammer-cli/src/shortcuts/mod.rs` (create)
- `swissarmyhammer-cli/src/shortcuts/flow_shortcuts.rs` (create)
- `swissarmyhammer-cli/src/main.rs` (update)
- `swissarmyhammer-cli/tests/shortcut_tests.rs` (create)

## Acceptance Criteria

- [ ] Shortcuts generated for all workflows
- [ ] Name conflicts resolved with underscore prefix
- [ ] Positional args work in shortcuts
- [ ] Shortcuts delegate to flow handler (not "flow run")
- [ ] Help text shows "(shortcut for 'flow <name>')" not "flow run"
- [ ] All tests pass
- [ ] Code compiles without warnings

## Estimated Changes

~230 lines of code



## Proposed Solution

After analyzing the codebase, I've identified a cleaner approach that integrates better with the existing CLI architecture:

### Analysis of Current State

The codebase already has:
1. **Dynamic CLI Builder** (`dynamic_cli.rs`) - generates MCP tool commands dynamically
2. **Flow Command Parser** (`commands/flow/mod.rs`) - handles `flow` command with `parse_flow_args()` 
3. **CliContext** - shared context with workflow storage access
4. **Static Commands** - added via `add_static_commands()` in `CliBuilder`

The `FlowSubcommand::Execute` enum variant was recently added (see tests) for the new flattened structure.

### Implementation Approach

Instead of creating a separate `shortcuts` module, I'll integrate shortcut generation directly into the existing `CliBuilder` in `dynamic_cli.rs`. This maintains consistency with how other dynamic commands are generated.

**Key Design Decisions:**

1. **Location**: Add shortcut generation to `CliBuilder::add_static_commands()` since shortcuts are workflow-specific top-level commands
2. **Conflict Resolution**: Check against both reserved flow subcommands (`list`) AND top-level commands (`serve`, `doctor`, `prompt`, `rule`, `flow`, `agent`, `validate`, `plan`, `implement`)
3. **Parameter Handling**: Shortcuts accept positional args and `--param` flags, delegating to `FlowSubcommand::Execute`
4. **Help Text**: Show "(shortcut for 'flow <workflow>')" in about text

### Implementation Steps

1. **Add helper method to `CliBuilder`**:
   - `build_workflow_shortcuts(&self, workflow_storage: &WorkflowStorage) -> Vec<Command>`
   - Returns vector of workflow shortcut commands

2. **Integrate into CLI building**:
   - Call from `build_cli()` after static commands
   - Add shortcuts as top-level subcommands

3. **Add dispatcher in main.rs**:
   - Match on shortcut command names
   - Create `FlowSubcommand::Execute` with mapped parameters
   - Delegate to existing `handle_flow_command()`

4. **Conflict detection**:
   - Reserved names: `list`, `serve`, `doctor`, `prompt`, `rule`, `flow`, `agent`, `validate`, `plan`, `implement`
   - Prefix conflicts with underscore: `_{workflow_name}`

### Files to Modify

- `swissarmyhammer-cli/src/dynamic_cli.rs` - Add `build_workflow_shortcuts()` method
- `swissarmyhammer-cli/src/main.rs` - Add shortcut dispatcher in `handle_dynamic_matches()`
- `swissarmyhammer-cli/src/context.rs` - Expose workflow storage if needed
- `swissarmyhammer-cli/tests/integration_tests.rs` or new test file - Add shortcut tests

### Why This Approach is Better

1. **Consistency**: Uses same pattern as dynamic MCP tool commands
2. **Less Code**: Reuses existing infrastructure instead of creating parallel system
3. **Maintainability**: All CLI generation logic in one place (`CliBuilder`)
4. **Testability**: Can test through existing CLI parsing infrastructure

### Trade-offs

- Need access to `WorkflowStorage` during CLI building (requires passing through `CliContext`)
- Shortcuts are generated at startup, so new workflows require CLI restart
- This is acceptable since workflows are typically static configuration

