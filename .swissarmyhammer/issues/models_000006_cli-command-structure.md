# Step 6: CLI Command Structure for Agent Management

Refer to ideas/models.md

## Objective

Add the CLI infrastructure for the `sah agent` command, including command enums and routing.

## Tasks

### 1. Add Agent Command to Main CLI Enum
- Add `Agent { subcommand: AgentSubcommand }` to `Commands` enum in `swissarmyhammer-cli/src/cli.rs`
- Include appropriate help text following the pattern of existing commands
- Add long description for the agent command

### 2. Add Agent Subcommand Enum
- Create `AgentSubcommand` enum in `swissarmyhammer-cli/src/cli.rs`
- Add `List { format: Option<OutputFormat> }` variant
- Add `Use { agent_name: String }` variant
- Follow existing patterns from other subcommands

### 3. Create Agent Command Module Structure
- Create `swissarmyhammer-cli/src/commands/agent/` directory
- Create `mod.rs` with module structure and main handler
- Create `list.rs` for list command implementation
- Create `use_command.rs` for use command implementation

### 4. Add Command Routing
- Add agent command handling to main CLI router
- Route to agent module's main handler
- Pass through CLI context and subcommand

### 5. Add Basic Module Infrastructure
- Add module exports in `swissarmyhammer-cli/src/commands/mod.rs`
- Create stub implementations for list and use commands
- Add basic error handling and return types

## Implementation Notes

- Follow the exact pattern used by `prompt` and `flow` commands
- Use existing CLI infrastructure and patterns
- Keep stub implementations simple - just return success
- Add proper module documentation

## Acceptance Criteria

- `sah agent --help` shows appropriate help text
- `sah agent list --help` and `sah agent use --help` work
- Command parsing routes correctly to agent module
- Stub implementations compile and run without errors
- No actual functionality yet - just CLI structure

## Files to Modify

- `swissarmyhammer-cli/src/cli.rs`
- `swissarmyhammer-cli/src/commands/mod.rs`
- `swissarmyhammer-cli/src/commands/agent/mod.rs` (new)
- `swissarmyhammer-cli/src/commands/agent/list.rs` (new)
- `swissarmyhammer-cli/src/commands/agent/use_command.rs` (new)