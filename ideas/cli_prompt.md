# CLI Prompt Command Cleanup Plan

## Problem Analysis

The `sah prompt` command implementation is currently scattered and inconsistent across multiple files and systems:

### Current Issues

1. **Scattered Implementation**:
   - CLI argument definitions in `src/cli.rs` (PromptSubcommand enum)
   - Dynamic CLI builder in `src/dynamic_cli.rs` (build_prompt_command)
   - Command handlers in `src/commands/prompt/mod.rs`
   - Manual CLI parsing in `src/main.rs` (handle_prompt_command)

2. **Duplicated Logic**:
   - Two separate command definition systems (static enum + dynamic builder)
   - Redundant argument parsing in multiple places
   - Multiple validation paths for the same data

3. **Over-Complicated Interface**:
   - `sah prompt list` has unnecessary filtering options (--source, --category)
   - Basic list command should be simple and just show all prompts
   - Filtering adds complexity without clear user benefit

4. **Inconsistent Architecture**:
   - Some commands use the modern dynamic CLI system
   - Prompt commands still use the legacy static enum system
   - Mixed approaches make the codebase harder to understand and maintain

5. **Maintenance Burden**:
   - Changes require updates in multiple files
   - Easy to introduce inconsistencies between CLI definition and handler
   - Testing requires covering multiple code paths

## Proposed Solution

**IMPORTANT**: This plan focuses **ONLY** on the prompt command. We are establishing a pattern that can be applied to other commands later, but we are explicitly **NOT** changing any other commands at this time.

### Phase 0: Add Global Arguments for Prompt Pattern

**Goal**: Add `verbose` and `format` as root-level CLI arguments and establish `CliContext` pattern for prompt commands only

#### 0.1 Update Root CLI Arguments
- Move `--verbose` and `--format` from individual subcommands to root level
- Update `CliContext` to include these global options
- Ensure all commands receive and use `CliContext` instead of just `TemplateContext`

#### 0.2 Update Prompt Command Signature Only
- **Current**: `handle_prompt_command(matches: &ArgMatches, template_context: &TemplateContext)`
- **New**: `handle_prompt_command(matches: &ArgMatches, cli_context: &CliContext)`
- **CliContext Contents**:
  ```rust
  pub struct CliContext {
      pub template_context: TemplateContext,
      pub verbose: bool,
      pub format: OutputFormat,
      // Future: dry_run, config_path, etc.
  }
  
  impl CliContext {
      pub fn display<T>(&self, items: Vec<T>) -> Result<(), DisplayError> 
      where
          T: Tabled + Serialize
      {
          // Handles table/json/yaml output based on self.format
      }
  }
  ```
- **Other commands remain unchanged** - still receive `TemplateContext`

#### 0.3 Benefits of Global Arguments (For Prompt Commands)
- **Pattern Establishment**: Creates template for future command migrations
- **User Experience**: `sah --verbose prompt list` instead of `sah prompt list --verbose`
- **Reduced Duplication**: No need to define these args in prompt subcommands
- **Architecture Demonstration**: Shows how global output controls should work
- **Proper State Management**: Context flows through prompt command call chain

### Phase 1: Consolidate to Commands Module

**Goal**: Move all prompt command logic into the dedicated `src/commands/prompt/` module

#### 1.1 Create Unified Command Structure
- **File**: `src/commands/prompt/cli.rs`
- **Purpose**: Single source of truth for prompt command definitions
- **Content**: 
  - Simple command definitions using clap builders (excluding global args)
  - Help text loaded from markdown files using `include_str!`
  - `list` subcommand with no filtering options - just lists all prompts
  - `test` subcommand with only necessary arguments
  - Clean argument parsing and validation

#### 1.2 Refactor Handler Logic
- **File**: `src/commands/prompt/mod.rs` (existing)
- **Changes**:
  - Remove dependency on `crate::cli::PromptSubcommand`
  - Accept `CliContext` instead of just `TemplateContext`
  - Use global `verbose` and `format` from `CliContext`
  - Create internal data structures for prompt-specific parameters only

#### 1.3 Create Subcommand Modules
```
src/commands/prompt/
├── mod.rs              # Main handler and module exports
├── cli.rs              # Command definitions and parsing
├── list.rs             # List command implementation
├── test.rs             # Test command implementation
├── display.rs          # Table formatting with tabled derives
├── description.md      # Main prompt command help (existing)
├── list_help.md        # Help text for list subcommand
└── test_help.md        # Help text for test subcommand
```

### Phase 2: Remove Legacy Static Definitions

**Goal**: Eliminate duplicate command definitions and parsing logic

#### 2.1 Remove from cli.rs
- Delete `PromptSubcommand` enum
- Remove prompt-related imports and dependencies
- Clean up any prompt-specific argument types

#### 2.2 Update dynamic_cli.rs
- Remove `build_prompt_command()` function
- Update main CLI builder to delegate to commands module
- Ensure consistent behavior with other command categories

#### 2.3 Simplify main.rs
- Remove `handle_prompt_command()` function
- Remove manual argument parsing logic
- Route prompt commands through standard command dispatch

### Phase 3: Implement Modern Command Pattern

**Goal**: Make prompt commands consistent with the rest of the CLI architecture

#### 3.1 Standard Command Interface
```rust
pub async fn handle_command(
    matches: &clap::ArgMatches,
    cli_context: &CliContext,
) -> i32
```

Where `CliContext` contains:
- `template_context: &TemplateContext`
- `verbose: bool` (from root CLI args)
- `format: OutputFormat` (from root CLI args)
- Other global CLI state

#### 3.2 Internal Command Types
```rust
pub struct ListCommand {
    // No filtering - just list all available prompts
    // Uses global verbose/format from CliContext
}

pub struct TestCommand {
    pub prompt_name: String,
    pub vars: Vec<String>,
    // ... other fields
}
```

#### 3.3 Parsing and Validation
- Parse clap matches into strongly-typed command structs
- Validate parameters early with clear error messages
- Separate parsing logic from business logic

### Phase 4: Improve Command Organization

**Goal**: Make each command self-contained and easier to test

#### 4.1 Separate Business Logic and Output Handling
- Extract simplified prompt listing logic: just load prompts and return data
- Remove filtering complexity from list command
- Create clean display objects with both `Tabled` and `Serialize` derives:
  ```rust
  #[derive(Tabled, Serialize)]
  struct PromptRow {
      name: String,
      title: String,
  }
  
  #[derive(Tabled, Serialize)]  
  struct VerbosePromptRow {
      name: String,
      title: String,
      description: String,
      source: String,
  }
  ```
- **No printing in handlers**: Use `cli_context.display(rows)` instead
- **Clean separation**: Handlers return data or errors, `CliContext` handles output
- Extract prompt testing logic into reusable functions
- Use `CliContext` for all output formatting decisions
- Create clear interfaces between CLI parsing and core functionality

#### 4.2 Enhanced Testing
- Unit tests for command parsing
- Integration tests for full command execution
- Separate tests for business logic vs CLI integration

#### 4.3 Better Error Handling
- Consistent error types and messages
- Proper exit codes for different failure scenarios
- User-friendly error formatting

## Implementation Strategy

### Step 0: Add Global Arguments for Prompt Command (Targeted)
1. **Update root CLI** to include `--verbose` and `--format` global arguments
2. **Expand `CliContext`** to carry these global settings alongside `TemplateContext`
3. **Update main.rs** to:
   - Parse global `--verbose` and `--format` arguments
   - Construct `CliContext` with these values
   - Pass `CliContext` to `handle_prompt_command()` instead of just `TemplateContext`
4. **Leave other commands unchanged** - they still receive `TemplateContext`
5. Test that global arguments work for prompt command only

**Scope**: This is targeted work that only affects prompt command routing. Other commands remain unchanged for now.

### Step 1: Create New Prompt Structure (Non-Breaking)
1. Create `src/commands/prompt/cli.rs` with new prompt command definitions (no global args)
2. Create help markdown files: `list_help.md` and `test_help.md`
3. Create `src/commands/prompt/display.rs` with `PromptRow` and `VerbosePromptRow` structs
4. Create `src/commands/prompt/list.rs` and `src/commands/prompt/test.rs` using `CliContext`
5. Update `src/commands/prompt/mod.rs` to use new structure and accept `CliContext`
6. Add comprehensive tests for new prompt implementation

### Step 2: Switch Prompt Commands Over (Breaking Changes)
1. Update main.rs to route prompt commands through new commands module
2. Remove prompt-specific legacy code from cli.rs and dynamic_cli.rs
3. Update prompt integration tests that depend on old structure
4. **Leave other commands unchanged** - only prompt commands use new architecture

### Step 3: Polish Prompt Commands
1. Add comprehensive error handling for prompt commands
2. Improve prompt command user experience with better help text in markdown files
3. Ensure all help text is sourced from markdown, not hardcoded strings
4. Add any missing prompt functionality that was lost during refactor
5. Validate that prompt commands work with global `--verbose` and `--format`

## Expected Benefits

### For Developers
- **Pattern Establishment**: Creates reusable template for other commands (future work)
- **Single Source of Truth**: All prompt command logic in one place
- **Easier Testing**: Clear separation of concerns and testable units
- **Proof of Concept**: Demonstrates how CLI commands should be structured
- **Reduced Duplication**: No more maintaining multiple prompt command definitions

### For Users
- **Simpler Interface**: `sah prompt list` just works - no confusing filter options
- **Better Table Formatting**: Clean, properly aligned tables using `tabled` crate
- **Consistent Global Options**: `sah --verbose --format=json prompt list` works like other commands
- **Better UX**: Global output controls available everywhere, not just specific subcommands  
- **Consistent Experience**: Prompt commands work like all other CLI commands
- **Better Error Messages**: More specific and actionable error feedback
- **Improved Help**: Comprehensive and consistent help text
- **Reliable Behavior**: Less chance of inconsistencies between different code paths

### For Maintenance
- **Pattern Documentation**: Establishes template for future command refactors
- **Simpler Updates**: Prompt command changes only need to be made in one place
- **Clear Architecture**: Easy to understand where prompt command logic lives
- **Better Testing**: More focused and comprehensive test coverage for prompt commands
- **Future-Proof**: Ready for new prompt commands or applying pattern to other commands

## Risks and Mitigation

### Risk: Breaking Existing Workflows
- **Mitigation**: Careful testing and gradual rollout
- **Mitigation**: Maintain backward compatibility where possible
- **Mitigation**: Clear documentation of any changes

### Risk: Scope Creep to Other Commands  
- **Reality**: It's tempting to "fix everything" while we're at it
- **Mitigation**: Stay focused on prompt commands only
- **Mitigation**: Document the pattern for future command migrations
- **Mitigation**: Resist urge to refactor unrelated commands

### Risk: Introducing Bugs During Refactor
- **Mitigation**: Comprehensive test coverage before changes
- **Mitigation**: Step-by-step implementation with validation at each step
- **Mitigation**: Keep old code until new code is fully validated

### Risk: Incomplete Migration
- **Mitigation**: Create checklist of all prompt command touchpoints
- **Mitigation**: Use compiler errors to find missing updates
- **Mitigation**: Integration tests to verify complete functionality

## Success Criteria

1. **Prompt commands use `CliContext` with global `--verbose` and `--format` support**
2. **All prompt commands work identically to current behavior (except simplified list)**
3. **No duplication between cli.rs, dynamic_cli.rs, and commands/prompt/ for prompt commands**
4. **Clear, single path from CLI argument to prompt command execution**
5. **Comprehensive test coverage for all prompt functionality**
6. **Consistent error handling and user experience for prompt commands**
7. **Global arguments work for prompt commands: `sah --verbose --format=json prompt list`**
8. **Other commands remain unchanged and continue to work**
9. **Documentation updated to reflect new prompt command architecture**

This plan will result in a cleaner, more maintainable architecture for prompt commands while preserving all existing functionality. **Importantly, it establishes a proven pattern that can be applied to other commands in future work, but we are explicitly only changing prompt commands in this effort.**

## Future Work (Explicitly Out of Scope)

Once the prompt command pattern is proven and working:
- Apply similar refactoring to `doctor` command
- Apply similar refactoring to `flow` command  
- Apply similar refactoring to other commands
- Fully standardize all commands on `CliContext` pattern

But for now, we focus solely on prompt commands to establish and validate the pattern.