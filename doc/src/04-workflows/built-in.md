# Built-in Workflows

SwissArmyHammer includes ready-to-use workflows for common development scenarios. These workflows demonstrate state machine patterns and tool orchestration.

## Example: Hello World Workflow

A simple three-state workflow demonstrating the basic workflow lifecycle:

```markdown
{{#include ../../../builtin/workflows/hello-world.md}}
```

### Key Features Demonstrated

- **State diagram** using Mermaid syntax
- **Action mapping** with simple state → action definitions
- **Prompt execution** with result capture
- **Variable substitution** in log messages
- **Linear state progression** from start to completion

### Usage

```bash
# Run the workflow
sah flow run hello-world

# Check workflow status
sah flow status hello-world

# View workflow logs
sah flow logs hello-world
```

## Example: Implementation Workflow

For a more complex example, see the `implement` workflow which demonstrates:
- Conditional branching based on issue states
- Tool orchestration across multiple MCP tools
- Error handling and recovery
- Multi-step autonomous execution

## Other Built-in Workflows

- `plan` - Convert specifications into implementation issues
- `implement` - Autonomously resolve all pending issues
- `tdd` - Test-driven development loop
- `code_issue` - Implement solutions for specific issues
- `document` - Generate project documentation
- `example-actions` - Comprehensive action type demonstration

All built-in workflows are located in the `builtin/workflows/` directory and can be viewed with:

```bash
sah flow list --category builtin
```