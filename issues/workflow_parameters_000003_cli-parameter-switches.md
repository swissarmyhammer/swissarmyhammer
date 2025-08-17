# CLI Parameter Switch Generation

**Refer to /Users/wballard/github/sah-parameters/ideas/workflow_parameters.md**

## Objective

Implement CLI parameter switch generation from workflow parameter definitions, allowing users to pass parameters as named command-line arguments (e.g., `--person-name "John" --language "Spanish"`) in addition to the existing `--var` and `--set` approaches.

## Current State

Workflows support parameter passing via:
- `--var key=value` for workflow variables
- `--set key=value` for liquid template variables
- No support for dedicated parameter switches like `--person-name`

## Implementation Tasks

### 1. Dynamic CLI Argument Generation

Extend the flow command to dynamically generate CLI arguments from workflow parameter definitions:

```rust
// In swissarmyhammer-cli/src/cli.rs
#[derive(Parser)]
pub struct FlowRunCommand {
    pub workflow: String,
    
    // Existing parameter support
    #[arg(long = "var", value_name = "KEY=VALUE")]
    pub vars: Vec<String>,
    
    #[arg(long = "set", value_name = "KEY=VALUE")]  
    pub set: Vec<String>,
    
    // Dynamic parameters will be added here
    #[command(flatten)]
    pub workflow_params: WorkflowParams,
}
```

### 2. Parameter Switch Mapping

Create parameter name to CLI switch conversion:

```rust
impl WorkflowParameter {
    pub fn to_cli_arg_name(&self) -> String {
        // Convert parameter name to CLI switch format
        // person_name -> --person-name
        // language -> --language
        format!("--{}", self.name.replace('_', "-"))
    }
    
    pub fn to_short_form(&self) -> Option<char> {
        // Generate short form if first letter is unique
        // --person-name -> -p (if available)
        // --language -> -l (if available)
    }
}
```

### 3. Runtime Parameter Discovery

Implement workflow parameter discovery before CLI parsing:

```rust
pub struct WorkflowParameterDiscovery;

impl WorkflowParameterDiscovery {
    pub fn discover_parameters(&self, workflow_name: &str) -> Result<Vec<WorkflowParameter>> {
        // Load workflow and extract parameter definitions
        // This needs to happen before full CLI parsing
    }
    
    pub fn generate_clap_args(&self, params: &[WorkflowParameter]) -> Vec<clap::Arg> {
        // Convert parameters to clap arguments
        params.iter().map(|p| self.parameter_to_clap_arg(p)).collect()
    }
}
```

### 4. Parameter Value Resolution

Resolve parameters from multiple sources in order of precedence:

1. Dedicated parameter switches (`--person-name`)
2. Variable switches (`--var person_name=value`)
3. Interactive prompting (if missing and required)
4. Default values (if specified)

```rust
pub struct ParameterResolver {
    pub fn resolve_workflow_parameters(
        &self,
        workflow: &Workflow,
        cli_matches: &clap::ArgMatches,
        interactive: bool
    ) -> Result<HashMap<String, serde_json::Value>>;
}
```

### 5. Help Text Generation

Generate comprehensive help text from parameter definitions:

```bash
$ sah flow run greeting --help

Execute a workflow

Usage: sah flow run greeting [OPTIONS]

Workflow Parameters:
  --person-name <PERSON_NAME>    The name of the person to greet
  --language <LANGUAGE>          The language to use for greeting [default: English] [possible values: English, Spanish, French]
  --enthusiastic                 Whether to use enthusiastic greeting

Options:
  --var <KEY=VALUE>             Set workflow variable
  --set <KEY=VALUE>             Set liquid template variable
  --interactive                 Run in interactive mode
  --help                        Print help
```

## Technical Details

### CLI Architecture Changes

The challenge is that clap requires compile-time argument definitions, but workflow parameters are discovered at runtime. Solutions:

1. **Two-Pass Parsing**: Parse workflow name first, load parameters, then reparse with full arguments
2. **Dynamic Subcommands**: Generate subcommands for each workflow
3. **Custom Parser**: Implement custom argument parsing for workflow parameters

**Recommended Approach**: Two-pass parsing for simplicity and compatibility.

### File Locations
- `swissarmyhammer-cli/src/cli.rs` - CLI command definitions
- `swissarmyhammer-cli/src/flow.rs` - Flow command implementation
- `swissarmyhammer/src/common/parameter_cli.rs` - CLI integration helpers

### Integration with Existing System

Maintain backward compatibility:
- Continue supporting `--var` and `--set`
- New parameter switches take precedence
- Migration path for existing workflows

### Testing Requirements

- CLI argument parsing tests
- Parameter precedence resolution tests
- Help text generation tests
- Integration tests with real workflows
- Backward compatibility tests

## Success Criteria

- [ ] Workflow parameters automatically generate CLI switches
- [ ] Parameter switches follow consistent naming conventions
- [ ] Help text includes parameter descriptions and constraints
- [ ] Backward compatibility with existing `--var` and `--set` approaches
- [ ] Parameter validation occurs before workflow execution
- [ ] Clear error messages for invalid parameter values

## Dependencies

- Requires completion of workflow_parameters_000001_frontmatter-parameter-schema
- Requires completion of workflow_parameters_000002_shared-parameter-system

## Next Steps

After completion, enables:
- Interactive parameter prompting for missing parameters
- Enhanced help text with parameter documentation
- Parameter completion support in shells