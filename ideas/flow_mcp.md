# Dynamic Workflow Execution via MCP

## Overview

Transform workflow execution from hardcoded CLI commands (`implement`, `plan`) into a unified dynamic MCP tool system where:

1. Workflows are executed through a single `flow_run` MCP tool
2. Workflows are discovered through a `flow_list` MCP tool
3. Workflow parameters use conventional CLI patterns (`--param key=value`)
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

### Two MCP Tools

#### 1. `flow_run` - Execute Workflows

```json
{
  "name": "flow_run",
  "description": "Execute a workflow with the specified parameters",
  "inputSchema": {
    "type": "object",
    "properties": {
      "workflow": {
        "type": "string",
        "description": "Name of the workflow to execute",
        "enum": ["implement", "plan", "code_review"]
      },
      "parameters": {
        "type": "object",
        "description": "Workflow-specific parameters as key-value pairs",
        "additionalProperties": true
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
    "required": ["workflow"]
  }
}
```

#### 2. `flow_list` - Discover Workflows

```json
{
  "name": "flow_list",
  "description": "List all available workflows with their descriptions and parameters",
  "inputSchema": {
    "type": "object",
    "properties": {
      "format": {
        "type": "string",
        "description": "Output format",
        "enum": ["json", "yaml", "table"],
        "default": "json"
      },
      "verbose": {
        "type": "boolean",
        "description": "Include detailed parameter information",
        "default": false
      },
      "source": {
        "type": "string",
        "description": "Filter by workflow source",
        "enum": ["all", "builtin", "project", "user"],
        "default": "all"
      }
    }
  }
}
```

**Example Response**:
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

#### Full Form

```bash
# Execute workflows
sah flow run [workflow] --param key=value --interactive --quiet

# List workflows
sah flow list --format json --verbose
```

#### Shortcut Form (Dynamic)

```bash
# Direct workflow execution (automatically generated)
sah implement --quiet
sah plan --param plan_filename=spec.md
sah code-review --param target_branch=main
```

### Parameter Convention

**Workflow Name**: First positional argument to `flow run`
```bash
sah flow run plan
```

**Workflow Parameters**: Using `--param` syntax (replaces `--var`)
```bash
sah flow run plan --param plan_filename=spec.md --param interactive=true
```

**Common Options**: Direct switches
```bash
sah flow run plan --interactive --quiet --dry-run
```

### Dynamic Shortcuts

Each workflow automatically gets a top-level command:

```bash
# These are equivalent:
sah plan --param plan_filename=spec.md
sah flow run plan --param plan_filename=spec.md
```

**Name Conflict Resolution**:

Only `list` and `run` are reserved for flow subcommands. If a workflow has either of these names, prefix with underscore:

```bash
# Workflow named "list" conflicts with flow list
sah _list --param input=value

# Workflow named "run" conflicts with flow run  
sah _run --param input=value
```

Also check against top-level commands:

```bash
# If workflow named "serve" conflicts with top-level command
sah _serve --param port=8080
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
sah flow run implement --quiet
sah flow run plan --param plan_filename=spec.md
```

## Implementation Plan

### Phase 1: MCP Tools

Create two MCP tools:

```rust
// swissarmyhammer-tools/src/mcp/tools/flow/run/mod.rs

pub struct FlowRunTool {
    workflow_storage: Arc<WorkflowStorage>,
}

#[async_trait]
impl McpTool for FlowRunTool {
    fn name(&self) -> &'static str {
        "flow_run"
    }

    fn description(&self) -> &'static str {
        "Execute a workflow with the specified parameters"
    }

    fn schema(&self) -> serde_json::Value {
        let workflows = self.workflow_storage.list_workflows()
            .unwrap_or_default();
        
        let workflow_names: Vec<String> = workflows
            .iter()
            .map(|w| w.name.to_string())
            .collect();

        serde_json::json!({
            "type": "object",
            "properties": {
                "workflow": {
                    "type": "string",
                    "description": "Name of the workflow to execute",
                    "enum": workflow_names
                },
                "parameters": {
                    "type": "object",
                    "description": "Workflow-specific parameters as key-value pairs",
                    "additionalProperties": true
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
            "required": ["workflow"]
        })
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> Result<CallToolResult, McpError> {
        let workflow_name = arguments.get("workflow")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::invalid_params("Missing workflow name"))?;

        let workflow = self.workflow_storage
            .get_workflow(&WorkflowName::new(workflow_name))
            .map_err(|e| McpError::invalid_params(format!("Workflow not found: {}", e)))?;

        let parameters = arguments.get("parameters")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();

        // Validate and execute workflow
        execute_workflow(workflow, parameters, context).await
    }

    fn cli_category(&self) -> Option<&'static str> {
        Some("flow")
    }

    fn cli_name(&self) -> &'static str {
        "run"
    }
}
```

```rust
// swissarmyhammer-tools/src/mcp/tools/flow/list/mod.rs

pub struct FlowListTool {
    workflow_storage: Arc<WorkflowStorage>,
}

#[async_trait]
impl McpTool for FlowListTool {
    fn name(&self) -> &'static str {
        "flow_list"
    }

    fn description(&self) -> &'static str {
        "List all available workflows with their descriptions and parameters"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "format": {
                    "type": "string",
                    "description": "Output format",
                    "enum": ["json", "yaml", "table"],
                    "default": "json"
                },
                "verbose": {
                    "type": "boolean",
                    "description": "Include detailed parameter information",
                    "default": false
                },
                "source": {
                    "type": "string",
                    "description": "Filter by workflow source",
                    "enum": ["all", "builtin", "project", "user"],
                    "default": "all"
                }
            }
        })
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> Result<CallToolResult, McpError> {
        let format = arguments.get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("json");

        let verbose = arguments.get("verbose")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let workflows = self.workflow_storage.list_workflows()
            .map_err(|e| McpError::internal_error(format!("Failed to list workflows: {}", e)))?;

        let response = build_workflow_list(workflows, verbose, format)?;
        Ok(BaseToolImpl::create_success_response(response))
    }

    fn cli_category(&self) -> Option<&'static str> {
        Some("flow")
    }

    fn cli_name(&self) -> &'static str {
        "list"
    }
}
```

### Phase 2: CLI Updates

Update CLI to handle new parameter syntax:

```rust
fn generate_flow_run_cli() -> Command {
    Command::new("run")
        .about("Execute a workflow with the specified parameters")
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

    // Reserved names: flow subcommands + top-level commands
    let reserved: HashSet<&str> = [
        "list", "run",  // Flow subcommands
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
            .about(format!("{} (shortcut for 'flow run {}')", 
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
    registry.register(FlowRunTool {
        workflow_storage: workflow_storage.clone(),
    });
    
    registry.register(FlowListTool {
        workflow_storage,
    });
    
    Ok(())
}
```

## Benefits

1. **Unified Interface**: Single MCP tool for all workflow execution
2. **Dynamic Discovery**: `flow_list` enables workflow discovery
3. **Conventional CLI**: `--param key=value` is more standard than `--var`
4. **Shortcuts**: Top-level commands for convenient workflow execution
5. **No Hardcoded Wrappers**: All workflows work through same mechanism
6. **Conflict Resolution**: Simple prefix system for reserved names

## Extended Application

Same pattern applies to other dynamic categories:

### Agents
```bash
sah agent run code-reviewer --param input=value
sah agent list --verbose
# Shortcut:
sah code-reviewer --param input=value
```

### Prompts
```bash
sah prompt run my-prompt --param input=value
sah prompt list --format json
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
   - **Solution**: `list`, `run` (flow subcommands) + all top-level commands

## Implementation Checklist

- [ ] Create `FlowRunTool` implementing `McpTool`
- [ ] Create `FlowListTool` implementing `McpTool`
- [ ] Register both tools in tool registry
- [ ] Update `sah flow run` CLI to use `--param` instead of `--var`
- [ ] Keep `sah flow list` CLI (maps to `flow_list` MCP tool)
- [ ] Implement dynamic shortcut generation
- [ ] Add name conflict detection (only `list`, `run`, and top-level commands)
- [ ] Support `--var` with deprecation warning during transition
- [ ] Remove `sah flow resume`, `sah flow status`, `sah flow logs`, `sah flow test`
- [ ] Update documentation
- [ ] Add deprecation warnings to `implement` and `plan` wrapper commands
- [ ] Create integration tests for both MCP tools
- [ ] Create integration tests for shortcuts
- [ ] Extend to agents and prompts

## Success Criteria

1. `flow_run` and `flow_list` appear in MCP `tools/list`
2. `flow_run` has enum of all available workflows
3. `flow_list` returns workflow metadata with parameters
4. CLI accepts `sah flow run [workflow] --param key=value`
5. `sah flow list` executes `flow_list` MCP tool
6. Dynamic shortcuts work: `sah plan --param x=y`
7. Name conflicts resolved: `sah _list` for workflow named "list"
8. Both forms work: shortcut and full form
9. No hardcoded workflow wrappers needed
10. Pattern consistent across flow, agent, prompt
11. Backward compatibility via `--var` deprecation
12. Clean removal of unused flow subcommands (resume, status, logs, test)
