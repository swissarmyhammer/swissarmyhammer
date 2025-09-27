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

## Proposed Solution

Based on examining the existing CLI structure and patterns, I will implement the agent CLI infrastructure following the exact pattern used by the `flow` command:

### 1. CLI Structure Updates
- Add `Agent { subcommand: AgentSubcommand }` to `Commands` enum in `cli.rs`
- Create `AgentSubcommand` enum with `List { format: Option<OutputFormat> }` and `Use { agent_name: String }` variants
- Follow the existing help text patterns with long descriptions

### 2. Module Structure  
- Create `swissarmyhammer-cli/src/commands/agent/` directory
- Create `mod.rs` with routing handler following flow command pattern
- Create `list.rs` and `use_command.rs` for command implementations
- Add `description.md` for help text (following MCP tool pattern)

### 3. Integration
- Add `pub mod agent;` to `commands/mod.rs`
- Add routing case to main CLI dispatcher
- Use same error handling and return code pattern as other commands

### 4. Implementation Notes
- Keep implementations as stubs that return success for now
- Follow exact patterns from `flow` command structure
- Use proper module documentation and DESCRIPTION constant
- Maintain consistency with existing CLI infrastructure


## Implementation Complete

Successfully implemented all CLI infrastructure for the `sah agent` command following the established patterns.

### What Was Implemented

1. **CLI Structure Updates** ✅
   - Added `Agent { subcommand: AgentSubcommand }` to `Commands` enum in `cli.rs`
   - Created `AgentSubcommand` enum with `List { format: Option<OutputFormat> }` and `Use { agent_name: String }` variants
   - Added comprehensive help text and long descriptions following existing patterns

2. **Module Structure** ✅
   - Created `swissarmyhammer-cli/src/commands/agent/` directory
   - Created `mod.rs` with routing handler following flow command pattern
   - Created `list.rs` and `use_command.rs` with stub implementations
   - Added `description.md` for help text

3. **Dynamic CLI Integration** ✅
   - Added `build_agent_command()` method to dynamic CLI builder
   - Integrated agent command into `add_static_commands()` method
   - Added routing in `handle_agent_command()` function in main.rs

4. **Command Routing** ✅
   - Added `pub mod agent;` to `commands/mod.rs`
   - Added routing case in main CLI dispatcher
   - Used same error handling and return code pattern as other commands

### Testing Results

- **Build**: ✅ Clean compilation (only harmless unused DESCRIPTION warning)
- **Help System**: ✅ All help commands work correctly:
  - `sah agent --help` shows main agent help
  - `sah agent list --help` shows list subcommand help  
  - `sah agent use --help` shows use subcommand help
- **Stub Implementation**: ✅ Both commands execute successfully:
  - `sah agent list` returns expected stub output
  - `sah agent use test-agent` returns expected stub output
- **Test Suite**: ✅ All 2803 tests pass

### File Structure Created

```
swissarmyhammer-cli/src/commands/agent/
├── mod.rs              # Main routing and error handling
├── description.md      # Help text content
├── list.rs            # List command stub implementation  
└── use_command.rs     # Use command stub implementation
```

### Next Steps

The CLI infrastructure is now complete and ready for:
1. Actual agent listing functionality in `list.rs`
2. Actual agent use functionality in `use_command.rs` 
3. Integration with the agent management system

The implementation follows all existing patterns and provides a solid foundation for the actual agent functionality.