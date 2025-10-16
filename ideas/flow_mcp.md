# Dynamic Workflow Execution via MCP

## Overview

Transform workflow execution from hardcoded CLI commands (`implement`, `plan`) into a unified dynamic MCP tool system where:

1. A single `flow` MCP tool handles both execution and discovery
2. The required `flow_name` parameter specifies the workflow or "list" for discovery
3. Required workflow parameters are positional, optional use `--param key=value`
4. Dynamic shortcuts provide top-level commands for each workflow

## Current State

### Hardcoded Command Wrappers

```rust
// swissarmyhammer-cli/src/commands/implement/mod.rs
pub async fn handle_command(context: &CliContext) -> i32 {
    let subcommand = FlowSubcommand::Run {
        workflow: "implement".to_string(),
        vars: vec![],
        interactive: false,
        dry_run: false,
        quiet: context.quiet,
    };
    crate::commands::flow::handle_command(subcommand, context).await
}

// swissarmyhammer-cli/src/commands/plan/mod.rs
pub async fn handle_command(plan_filename: String, context: &CliContext) -> i32 {
    let subcommand = FlowSubcommand::Run {
        workflow: "plan".to_string(),
        vars: vec![format!("plan_filename={}", plan_filename)],
        interactive: false,
        dry_run: false,
        quiet: context.quiet,
    };
    crate::commands::flow::handle_command(subcommand, context).await
}
```

### Current Flow Command

```bash
sah flow run <workflow> --var key=value --interactive --dry-run --quiet
sah flow list --format json --verbose --source builtin
```

### Problems

1. **Hardcoded Wrappers**: Each workflow needs a custom command wrapper
2. **Non-Standard Parameters**: Using `--var key=value` is unconventional
3. **Discovery Gap**: MCP clients can't easily discover workflows
4. **Inconsistent Patterns**: Some commands are hardcoded (`implement`), others require `flow run`
5. **Parameter Mismatch**: Workflow parameter definitions don't map to CLI naturally

## Proposed Solution

### Single MCP Tool: `flow`

One `flow` tool handles both workflow execution and discovery.

**Long-Running Operations**: Workflow execution sends MCP notifications to track progress:

```json
{
  "name": "flow",
  "description": "Execute or list workflows",
  "inputSchema": {
    "type": "object",
    "properties": {
      "flow_name": {
        "type": "string",
        "description": "Name of the workflow to execute, or 'list' to show all workflows",
        "enum": ["list", "implement", "plan", "code_review"]
      },
      "parameters": {
        "type": "object",
        "description": "Workflow-specific parameters as key-value pairs (ignored when flow_name='list')",
        "additionalProperties": true
      },
      "format": {
        "type": "string",
        "description": "Output format when flow_name='list'",
        "enum": ["json", "yaml", "table"],
        "default": "json"
      },
      "verbose": {
        "type": "boolean",
        "description": "Include detailed parameter information when flow_name='list'",
        "default": false
      },
      "interactive": {
        "type": "boolean",
        "description": "Enable interactive mode for prompts (workflow execution only)",
        "default": false
      },
      "dry_run": {
        "type": "boolean",
        "description": "Show execution plan without running (workflow execution only)",
        "default": false
      },
      "quiet": {
        "type": "boolean",
        "description": "Suppress progress output (workflow execution only)",
        "default": false
      }
    },
    "required": ["flow_name"]
  }
}
```

**Special Case**: When `flow_name="list"`, return workflow metadata:

```json
{
  "workflows": [
    {
      "name": "implement",
      "description": "Execute the implement workflow for autonomous issue resolution",
      "source": "builtin",
      "parameters": []
    },
    {
      "name": "plan",
      "description": "Execute planning workflow for specific specification files",
      "source": "builtin",
      "parameters": [
        {
          "name": "plan_filename",
          "type": "string",
          "description": "Path to the specification file to process",
          "required": true
        }
      ]
    }
  ]
}
```

### CLI Command Structure

#### Parameter Convention

**IMPORTANT**: Required workflow parameters are **positional**, not switches.

**Required Parameters**: Positional arguments in order
```bash
# plan workflow has required parameter 'plan_filename'
sah flow run plan spec.md

# code_review workflow has required params 'target_branch' and 'source_branch'  
sah flow run code-review main feature-x
```

**Optional Parameters**: Using `--param` for named optional params
```bash
# Optional workflow-specific parameters
sah flow run custom-workflow --param author=alice --param priority=high
```

**Common Options**: Direct switches
```bash
sah flow run plan spec.md --interactive --quiet --dry-run
```

**Special Case**: List workflows
```bash
sah flow list --format json --verbose
```

#### Full Form Examples

```bash
# Execute workflow with required positional param
sah flow run plan spec.md --interactive

# Execute workflow with no required params
sah flow run implement --quiet

# Execute workflow with multiple required params
sah flow run code-review main feature-x --param reviewer=bob

# List workflows
sah flow list --verbose
```

#### Shortcut Form (Dynamic)

Each workflow gets a top-level command with the same parameter conventions:

```bash
# Shortcut form (automatically generated)
sah plan spec.md --interactive
sah implement --quiet
sah code-review main feature-x --param reviewer=bob
```

### Dynamic Shortcuts - Name Conflicts

**Name Conflict Resolution**:

Only `list` and `run` are reserved for flow subcommands. Also check against top-level commands. If a workflow name conflicts, prefix with underscore:

```bash
# Workflow named "list" conflicts with flow list
sah _list required-arg

# Workflow named "run" conflicts with flow run
sah _run required-arg

# Workflow named "serve" conflicts with top-level command
sah _serve required-arg
```

Also check against top-level commands:

```bash
# If workflow named "serve" conflicts with top-level command
sah _serve --param port=8080

# If workflow named "flow" conflicts with the flow command itself
sah _flow --param input=value
```

### Migration Path

**Old Way (Deprecated)**:
```bash
sah implement
sah plan spec.md
sah flow run custom_workflow --var input=value
```

**New Way**:
```bash
# Shortcut (recommended)
sah implement --quiet
sah plan --param plan_filename=spec.md

# Full form (explicit)
sah flow implement --quiet
sah flow plan --param plan_filename=spec.md
```

### MCP Notifications

Since workflows are long-running operations, the `flow` tool sends progress notifications:

**Notification Types**:

1. **Flow Start**:
```json
{
  "method": "notifications/progress",
  "params": {
    "token": "<workflow_run_id>",
    "progress": 0,
    "message": "Starting workflow: plan",
    "metadata": {
      "flow_name": "plan",
      "parameters": {
        "plan_filename": "spec.md"
      },
      "initial_state": "parse_spec"
    }
  }
}
```

2. **State Start**:
```json
{
  "method": "notifications/progress",
  "params": {
    "token": "<workflow_run_id>",
    "progress": 25,
    "message": "Entering state: parse_spec",
    "metadata": {
      "flow_name": "plan",
      "state_id": "parse_spec",
      "state_description": "Parse specification file"
    }
  }
}
```

3. **State Complete**:
```json
{
  "method": "notifications/progress",
  "params": {
    "token": "<workflow_run_id>",
    "progress": 50,
    "message": "Completed state: parse_spec",
    "metadata": {
      "flow_name": "plan",
      "state_id": "parse_spec",
      "next_state": "generate_plan"
    }
  }
}
```

4. **Flow Complete**:
```json
{
  "method": "notifications/progress",
  "params": {
    "token": "<workflow_run_id>",
    "progress": 100,
    "message": "Completed workflow: plan",
    "metadata": {
      "flow_name": "plan",
      "status": "completed",
      "final_state": "done"
    }
  }
}
```

5. **Flow Error**:
```json
{
  "method": "notifications/progress",
  "params": {
    "token": "<workflow_run_id>",
    "progress": null,
    "message": "Workflow failed: plan",
    "metadata": {
      "flow_name": "plan",
      "status": "failed",
      "error_state": "generate_plan",
      "error": "Failed to generate plan: invalid specification"
    }
  }
}
```

**Implementation Notes**:
- Notifications are less verbose than logging (don't include every action)
- Each notification includes the workflow run ID as token for tracking
- Progress percentage estimated from state count/position
- State info includes enough context for monitoring dashboards
- Notifications sent asynchronously, don't block workflow execution

## Implementation Plan

### Phase 1: Single MCP Tool

Create one MCP tool that handles both execution and discovery:

```rust
// swissarmyhammer-tools/src/mcp/tools/flow/mod.rs

pub struct FlowTool {
    workflow_storage: Arc<WorkflowStorage>,
}

#[async_trait]
impl McpTool for FlowTool {
    fn name(&self) -> &'static str {
        "flow"
    }

    fn description(&self) -> &'static str {
        "Execute or list workflows"
    }

    fn schema(&self) -> serde_json::Value {
        let workflows = self.workflow_storage.list_workflows()
            .unwrap_or_default();
        
        let mut workflow_names: Vec<String> = workflows
            .iter()
            .map(|w| w.name.to_string())
            .collect();
        
        // Add "list" as special case
        workflow_names.insert(0, "list".to_string());

        serde_json::json!({
            "type": "object",
            "properties": {
                "flow_name": {
                    "type": "string",
                    "description": "Name of the workflow to execute, or 'list' to show all workflows",
                    "enum": workflow_names
                },
                "parameters": {
                    "type": "object",
                    "description": "Workflow-specific parameters as key-value pairs",
                    "additionalProperties": true
                },
                "format": {
                    "type": "string",
                    "description": "Output format when workflow='list'",
                    "enum": ["json", "yaml", "table"],
                    "default": "json"
                },
                "verbose": {
                    "type": "boolean",
                    "description": "Include detailed parameter information when workflow='list'",
                    "default": false
                },
                "interactive": {
                    "type": "boolean",
                    "description": "Enable interactive mode for prompts",
                    "default": false
                },
                "dry_run": {
                    "type": "boolean",
                    "description": "Show execution plan without running",
                    "default": false
                },
                "quiet": {
                    "type": "boolean",
                    "description": "Suppress progress output",
                    "default": false
                }
            },
            "required": ["flow_name"]
        })
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> Result<CallToolResult, McpError> {
        let flow_name = arguments.get("flow_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::invalid_params("Missing flow_name parameter"))?;

        // Special case: list workflows
        if flow_name == "list" {
            return self.list_workflows(arguments).await;
        }

        // Regular case: execute workflow
        self.execute_workflow(flow_name, arguments, context).await
    }

    fn cli_category(&self) -> Option<&'static str> {
        None  // Top-level dynamic command
    }

    fn cli_name(&self) -> &'static str {
        "flow"
    }
}

impl FlowTool {
    async fn list_workflows(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
    ) -> Result<CallToolResult, McpError> {
        let format = arguments.get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("json");

        let verbose = arguments.get("verbose")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let workflows = self.workflow_storage.list_workflows()
            .map_err(|e| McpError::internal_error(format!("Failed to list workflows: {}", e)))?;

        let response = build_workflow_list(&workflows, verbose, format)?;
        Ok(BaseToolImpl::create_success_response(response))
    }

    async fn execute_workflow(
        &self,
        flow_name: &str,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> Result<CallToolResult, McpError> {
        let workflow = self.workflow_storage
            .get_workflow(&WorkflowName::new(flow_name))
            .map_err(|e| McpError::invalid_params(format!("Workflow not found: {}", e)))?;

        let parameters = arguments.get("parameters")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();

        let interactive = arguments.get("interactive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let dry_run = arguments.get("dry_run")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let quiet = arguments.get("quiet")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Create workflow executor with notification support
        let mut executor = WorkflowExecutor::new_with_notifications(context.notification_sender.clone());
        
        // Send flow start notification
        executor.notify_flow_start(flow_name, &parameters, &workflow.initial_state).await?;

        // Execute workflow with progress notifications
        let result = executor.execute_with_notifications(
            workflow, 
            parameters, 
            interactive, 
            dry_run, 
            quiet,
            context
        ).await;

        // Send completion notification
        match &result {
            Ok(_) => executor.notify_flow_complete(flow_name).await?,
            Err(e) => executor.notify_flow_error(flow_name, e).await?,
        }

        result
    }
}
```

### Phase 2: CLI Updates

Update CLI to handle new parameter syntax:

```rust
fn generate_flow_cli() -> Command {
    Command::new("flow")
        .about("Execute or list workflows")
        .arg(
            Arg::new("workflow")
                .help("Name of the workflow to execute")
                .required(true)
                .value_name("WORKFLOW")
        )
        .arg(
            Arg::new("param")
                .long("param")
                .short('p')
                .help("Workflow-specific parameter (key=value)")
                .action(ArgAction::Append)
                .value_name("KEY=VALUE")
        )
        .arg(
            Arg::new("interactive")
                .long("interactive")
                .short('i')
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new("dry_run")
                .long("dry-run")
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new("quiet")
                .long("quiet")
                .short('q')
                .action(ArgAction::SetTrue)
        )
}
```

### Phase 3: Dynamic Shortcuts

Generate top-level commands for each workflow:

```rust
fn generate_workflow_shortcuts(
    workflow_storage: &WorkflowStorage,
) -> Vec<Command> {
    let mut shortcuts = Vec::new();
    let workflows = workflow_storage.list_workflows().unwrap_or_default();

    // Reserved names: special case + top-level commands
    let reserved: HashSet<&str> = [
        "list",  // Special case for workflow discovery
        "flow", "agent", "prompt", "serve", "doctor", "rule", "validate"  // Top-level
    ].iter().copied().collect();

    for workflow in workflows {
        let workflow_name = workflow.name.to_string();
        
        let command_name = if reserved.contains(workflow_name.as_str()) {
            format!("_{}", workflow_name)
        } else {
            workflow_name.clone()
        };

        let cmd = Command::new(command_name)
            .about(format!("{} (shortcut for 'flow {}')", 
                          workflow.description, workflow_name))
            .arg(
                Arg::new("param")
                    .long("param")
                    .short('p')
                    .action(ArgAction::Append)
                    .value_name("KEY=VALUE")
            )
            .arg(Arg::new("interactive").long("interactive").short('i').action(ArgAction::SetTrue))
            .arg(Arg::new("dry_run").long("dry-run").action(ArgAction::SetTrue))
            .arg(Arg::new("quiet").long("quiet").short('q').action(ArgAction::SetTrue));

        shortcuts.push(cmd);
    }

    shortcuts
}
```

### Phase 4: Tool Registry

```rust
pub fn register_workflow_tools(
    registry: &mut ToolRegistry,
    workflow_storage: Arc<WorkflowStorage>,
) -> Result<()> {
    registry.register(FlowTool {
        workflow_storage,
    });
    
    Ok(())
}
```

## Benefits

1. **Unified Interface**: Single MCP tool for both execution and discovery
2. **Dynamic Discovery**: `workflow="list"` special case enables workflow discovery
3. **Conventional CLI**: `--param key=value` is more standard than `--var`
4. **Shortcuts**: Top-level commands for convenient workflow execution
5. **No Hardcoded Wrappers**: All workflows work through same mechanism
6. **Minimal Conflict Resolution**: Only `list` is special, plus top-level commands
7. **Simple Implementation**: One tool, one enum, special case for "list"

## Extended Application

Same pattern applies to other dynamic categories:

### Agents
```bash
sah agent code-reviewer --param input=value
sah agent list --verbose  # Special case
# Shortcut:
sah code-reviewer --param input=value
```

### Prompts
```bash
sah prompt my-prompt --param input=value
sah prompt list --format json  # Special case
# Shortcut:
sah my-prompt --param input=value
```

## Open Questions

1. **Backward Compatibility**: Support `--var` during transition?
   - **Solution**: Accept both `--var` and `--param`, warn on `--var` usage

2. **Type Validation**: How to validate parameter types from CLI?
   - **Solution**: Parse and validate using workflow parameter definitions
   
3. **Complex Parameters**: JSON objects or arrays?
   - **Solution**: Support JSON string values: `--param config='{"key":"value"}'`

4. **Shortcut Discovery**: How users learn about shortcuts?
   - **Solution**: Help text shows shortcuts with note "(shortcut for 'flow run X')"

5. **Reserved Name List**: Which names are reserved?
   - **Solution**: `list` (special case) + all top-level commands (flow, agent, prompt, etc.)

## Implementation Checklist

- [ ] Create `FlowTool` implementing `McpTool`
- [ ] Implement special case handling when `flow_name="list"`
- [ ] Add "list" to workflow enum in schema
- [ ] Register tool in tool registry
- [ ] Implement MCP notification support:
  - [ ] Send flow start notification with flow_name and parameters
  - [ ] Send state start notification when entering each state
  - [ ] Send state complete notification when exiting each state
  - [ ] Send flow complete notification on successful completion
  - [ ] Send flow error notification on failure
  - [ ] Include workflow run ID as notification token
  - [ ] Calculate progress percentage from state position
- [ ] Update `sah flow` CLI to accept workflow as first positional
- [ ] Handle `sah flow list` as special case
- [ ] Update CLI to parse positional required parameters
- [ ] Update CLI to use `--param` for optional parameters
- [ ] Implement dynamic shortcut generation with positional args
- [ ] Add name conflict detection (only `list` and top-level commands)
- [ ] Support `--var` with deprecation warning during transition
- [ ] Remove old flow subcommands (resume, status, logs, test)
- [ ] Update documentation
- [ ] Add deprecation warnings to `implement` and `plan` wrapper commands
- [ ] Create integration tests for workflow execution
- [ ] Create integration tests for MCP notifications during execution
- [ ] Create integration tests for `flow_name="list"` special case
- [ ] Create integration tests for shortcuts with positional parameters
- [ ] Extend pattern to agents and prompts (single `agent`, `prompt` tools with notifications)

## Success Criteria

1. Single `flow` tool appears in MCP `tools/list`
2. `flow` tool has required parameter `flow_name` with enum including "list" + all available workflows
3. `sah flow list` returns workflow metadata (special case when `flow_name="list"`)
4. `sah flow [workflow]` executes workflows (when `flow_name="<workflow>"`)
5. CLI accepts `sah flow [workflow] [required_args...]` with positional required params
6. Dynamic shortcuts work: `sah plan spec.md`
7. Name conflicts resolved: `sah _list` for workflow named "list"
8. Both forms work: `sah plan spec.md` and `sah flow plan spec.md`
9. No hardcoded workflow wrappers needed
10. Pattern consistent across flow, agent, prompt
11. Backward compatibility via `--var` deprecation
12. Clean removal of unused flow subcommands
13. MCP clients can call `flow` with `flow_name="list"` for discovery
14. MCP clients can call `flow` with `flow_name="<name>"` for execution
15. Required workflow parameters are positional, optional use `--param`
16. MCP notifications sent during workflow execution:
    - Flow start notification with flow_name and parameters
    - State start/complete notifications for each state transition
    - Flow complete notification on success
    - Flow error notification on failure
    - All notifications include workflow run ID and progress info
17. Notifications provide sufficient context for monitoring (flow name, state, parameters)
18. Notifications are less verbose than logging (key events only)
