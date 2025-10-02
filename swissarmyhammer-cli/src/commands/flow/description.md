Automate complex development workflows with powerful, resumable state machines.

Workflows orchestrate multi-step processes including code reviews, deployments,
testing, and AI-powered operations. Define once, run reliably, resume anywhere.

WORKFLOW POWER

State Machine Architecture:
• Define complex processes as declarative state machines
• Execute actions, tools, and AI commands in sequence or parallel
• Handle conditional logic, loops, and error recovery
• Resume interrupted workflows exactly where they stopped
• Track execution state, variables, and progress

Built for Reliability:
• Pause and resume workflows without losing state
• Interactive mode for step-by-step control and debugging
• Dry-run mode to preview execution without side effects
• Comprehensive logging and status tracking
• Automatic error handling and recovery options

AI Integration:
• Execute Claude commands within workflow steps
• Pass variables and context between AI operations
• Chain AI-powered analysis, planning, and implementation
• Combine automated and AI-assisted tasks seamlessly

COMMANDS

The flow system provides comprehensive workflow management:

• run - Start a new workflow execution with variables
• resume - Continue a paused or interrupted workflow run
• list - Display all available workflows from all sources
• status - Check execution state and progress of a run
• logs - View detailed execution logs and step history

WORKFLOW DISCOVERY

Workflows are loaded from multiple sources:
• Built-in workflows - Standard development workflows included
• User workflows (~/.swissarmyhammer/workflows/) - Personal automations
• Project workflows (./workflows/) - Project-specific processes

COMMON WORKFLOWS

Start a workflow with parameters:
  swissarmyhammer flow run code-review --vars file=main.rs

Preview execution without running:
  swissarmyhammer flow run deploy --dry-run

Resume after interruption:
  swissarmyhammer flow resume a1b2c3d4

Interactive step-through debugging:
  swissarmyhammer flow resume a1b2c3d4 --interactive

Monitor workflow status:
  swissarmyhammer flow status a1b2c3d4 --watch

View execution history:
  swissarmyhammer flow logs a1b2c3d4

List available workflows:
  swissarmyhammer flow list --format json

EXECUTION OPTIONS

Pass variables to workflows:
  --vars key=value              # Single variable
  --vars file=main.rs --vars author=jane  # Multiple variables

Control execution:
  --interactive                 # Step-by-step confirmation
  --dry-run                     # Show plan without executing

EXAMPLES

Run code review workflow:
  swissarmyhammer flow run code-review --vars file=main.rs --vars severity=high

Test deployment workflow:
  swissarmyhammer flow run deploy --dry-run

Resume interrupted workflow:
  swissarmyhammer flow resume a1b2c3d4 --interactive

Check workflow status:
  swissarmyhammer flow status a1b2c3d4

View execution logs:
  swissarmyhammer flow logs a1b2c3d4 --format json

List available workflows:
  swissarmyhammer flow list

Workflows bring automation, reliability, and AI-powered intelligence to your
development processes. Define complex operations once and execute them
consistently every time.