# Step 5: Update CLI Parameter Parsing

Refer to ideas/flow_mcp.md

## Objective

Update CLI to support the new parameter convention: workflow name is first positional, required workflow parameters follow as positional, optional parameters use `--param key=value`.

## Context

The spec requires `sah flow [workflow] [required_args...]` - NO "run" subcommand. Required workflow parameters are positional, optional use `--param key=value`. We need to support both `--param` and deprecated `--var` during transition.

## Tasks

### 1. Update FlowCommand Structure

Update `swissarmyhammer-cli/src/cli.rs`:

```rust
#[derive(Debug, Clone)]
pub struct FlowCommand {
    pub workflow_name: String,           // First positional: workflow name OR "list"
    pub positional_args: Vec<String>,     // Required workflow parameters (positional)
    pub params: Vec<String>,              // Optional parameters: --param key=value
    pub vars: Vec<String>,                // DEPRECATED: --var key=value
    pub format: Option<String>,           // For list: --format
    pub verbose: bool,                    // For list: --verbose
    pub source: Option<String>,           // For list: --source
    pub interactive: bool,
    pub dry_run: bool,
    pub quiet: bool,
}
```

### 2. Update CLI Parser

Update the flow command parser:

```rust
fn parse_flow_command(matches: &ArgMatches) -> FlowCommand {
    let workflow_name = matches.get_one::<String>("workflow")
        .expect("workflow is required")
        .clone();
    
    // Collect positional arguments after workflow name
    let positional_args: Vec<String> = matches
        .get_many::<String>("positional")
        .map(|vals| vals.cloned().collect())
        .unwrap_or_default();
    
    // Collect --param values
    let params: Vec<String> = matches
        .get_many::<String>("param")
        .map(|vals| vals.cloned().collect())
        .unwrap_or_default();
    
    // Collect --var values (deprecated)
    let vars: Vec<String> = matches
        .get_many::<String>("var")
        .map(|vals| vals.cloned().collect())
        .unwrap_or_default();
    
    // Warn if using deprecated --var
    if !vars.is_empty() {
        eprintln!("Warning: --var is deprecated, use --param instead");
    }
    
    FlowCommand {
        workflow_name,
        positional_args,
        params,
        vars,
        format: matches.get_one::<String>("format").cloned(),
        verbose: matches.get_flag("verbose"),
        source: matches.get_one::<String>("source").cloned(),
        interactive: matches.get_flag("interactive"),
        dry_run: matches.get_flag("dry_run"),
        quiet: matches.get_flag("quiet"),
    }
}
```

### 3. Update Command Builder

Update clap command definition (NO "run" subcommand):

```rust
fn build_flow_command() -> Command {
    Command::new("flow")
        .about("Execute or list workflows")
        .arg(Arg::new("workflow")
            .required(true)
            .help("Workflow name or 'list' to show all workflows"))
        .arg(Arg::new("positional")
            .num_args(0..)
            .help("Required workflow parameters (positional)"))
        .arg(Arg::new("param")
            .long("param")
            .short('p')
            .action(ArgAction::Append)
            .value_name("KEY=VALUE")
            .help("Optional workflow parameter"))
        .arg(Arg::new("var")
            .long("var")
            .action(ArgAction::Append)
            .value_name("KEY=VALUE")
            .help("(Deprecated) Use --param instead"))
        .arg(Arg::new("format")
            .long("format")
            .value_name("FORMAT")
            .help("Output format for 'list' (json, yaml, table)"))
        .arg(Arg::new("verbose")
            .long("verbose")
            .short('v')
            .action(ArgAction::SetTrue)
            .help("Verbose output for 'list'"))
        .arg(Arg::new("source")
            .long("source")
            .value_name("SOURCE")
            .help("Filter by source for 'list' (builtin, project, user)"))
        .arg(Arg::new("interactive")
            .long("interactive")
            .short('i')
            .action(ArgAction::SetTrue))
        .arg(Arg::new("dry_run")
            .long("dry-run")
            .action(ArgAction::SetTrue))
        .arg(Arg::new("quiet")
            .long("quiet")
            .short('q')
            .action(ArgAction::SetTrue))
}
```

### 4. Update Flow Handler

Update flow command handler:

```rust
pub async fn handle_flow_command(
    cmd: FlowCommand,
    context: &CliContext,
) -> Result<()> {
    // Special case: list workflows
    if cmd.workflow_name == "list" {
        return execute_list_command(
            cmd.format.unwrap_or_else(|| "json".to_string()),
            cmd.verbose,
            cmd.source,
            context,
        ).await;
    }
    
    // Regular case: execute workflow
    execute_workflow_command(cmd, context).await
}

async fn execute_workflow_command(
    cmd: FlowCommand,
    context: &CliContext,
) -> Result<()> {
    // Get workflow definition to determine parameter mapping
    let workflow_def = context.workflow_storage
        .get_workflow(&WorkflowName::new(&cmd.workflow_name))?;
    
    // Map positional args to required parameters
    let mut all_params = map_positional_to_params(
        &workflow_def,
        cmd.positional_args,
    )?;
    
    // Add --param values
    for param in cmd.params {
        let (key, value) = parse_key_value(&param)?;
        all_params.insert(key, value);
    }
    
    // Add --var values (deprecated, but still supported)
    for var in cmd.vars {
        let (key, value) = parse_key_value(&var)?;
        all_params.insert(key, value);
    }
    
    // Execute workflow with combined parameters
    execute_workflow(
        &cmd.workflow_name,
        all_params,
        cmd.interactive,
        cmd.dry_run,
        cmd.quiet,
        context,
    ).await
}
```

### 5. Add Parameter Mapping Function

```rust
fn map_positional_to_params(
    workflow: &Workflow,
    positional: Vec<String>,
) -> Result<HashMap<String, String>> {
    let required_params: Vec<_> = workflow.parameters
        .iter()
        .filter(|p| p.required)
        .collect();
    
    if positional.len() != required_params.len() {
        return Err(anyhow!(
            "Expected {} positional arguments for required parameters, got {}",
            required_params.len(),
            positional.len()
        ));
    }
    
    let mut params = HashMap::new();
    for (arg, param) in positional.iter().zip(required_params.iter()) {
        params.insert(param.name.clone(), arg.clone());
    }
    
    Ok(params)
}
```

### 6. Add Tests

```rust
#[test]
fn test_parse_flow_without_run_subcommand() {
    // Test flow takes workflow name directly
    let matches = parse_args(&["flow", "plan", "spec.md"]);
    assert_eq!(matches.workflow_name, "plan");
    assert_eq!(matches.positional_args, vec!["spec.md"]);
}

#[test]
fn test_parse_flow_list() {
    // Test flow list special case
    let matches = parse_args(&["flow", "list", "--verbose"]);
    assert_eq!(matches.workflow_name, "list");
    assert!(matches.verbose);
}

#[test]
fn test_parse_positional_args() {
    // Test positional arguments are parsed correctly
}

#[test]
fn test_param_vs_var_precedence() {
    // Test --param takes precedence over --var
}

#[test]
fn test_deprecated_var_warning() {
    // Test warning is shown for --var usage
}
```

## Files to Modify

- `swissarmyhammer-cli/src/cli.rs`
- `swissarmyhammer-cli/src/commands/flow/mod.rs`
- `swissarmyhammer-cli/src/commands/flow/handler.rs` (create or update)
- `swissarmyhammer-cli/tests/flow_parameter_tests.rs` (create)

## Acceptance Criteria

- [ ] Flow command takes workflow name as first positional (NO "run" subcommand)
- [ ] `sah flow list` works for workflow discovery
- [ ] `sah flow plan spec.md` works for execution
- [ ] Positional arguments work for required parameters
- [ ] `--param key=value` works for optional parameters
- [ ] `--var key=value` still works with deprecation warning
- [ ] Correct number of positional args validated
- [ ] Parameters mapped correctly to workflow variables
- [ ] All tests pass
- [ ] Code compiles without warnings

## Estimated Changes

~250 lines of code
