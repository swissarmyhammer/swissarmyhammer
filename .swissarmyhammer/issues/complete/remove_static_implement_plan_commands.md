# Remove Static Implement and Plan Commands

## Objective

Remove the hardcoded `implement` and `plan` wrapper commands since they're now available via the dynamic flow system.

## Context

With the flow MCP implementation complete, we have two ways to access these workflows:
1. **Full form**: `sah flow implement`, `sah flow plan spec.md`
2. **Dynamic shortcuts**: `sah implement`, `sah plan spec.md`

The static wrapper commands in `swissarmyhammer-cli/src/commands/implement/` and `swissarmyhammer-cli/src/commands/plan/` are now redundant and should be removed.

## Current State

**Static Commands**:
- `swissarmyhammer-cli/src/commands/implement/mod.rs`
- `swissarmyhammer-cli/src/commands/implement/description.md`
- `swissarmyhammer-cli/src/commands/plan/mod.rs`
- `swissarmyhammer-cli/src/commands/plan/description.md`

**Dynamic Replacements**:
- Flow shortcuts automatically generate `implement` and `plan` commands
- If there's a naming conflict, they become `_implement` and `_plan`

## Tasks

### 1. Delete Static Command Modules

Delete the following directories and all files within:
- `swissarmyhammer-cli/src/commands/implement/`
- `swissarmyhammer-cli/src/commands/plan/`

### 2. Update Commands Module

Update `swissarmyhammer-cli/src/commands/mod.rs`:

```rust
pub mod agent;
pub mod doctor;
pub mod flow;
// pub mod implement;  // REMOVED - now via dynamic flow shortcuts
// pub mod plan;       // REMOVED - now via dynamic flow shortcuts
pub mod prompt;
pub mod rule;
pub mod serve;
pub mod validate;
```

### 3. Update CLI Enum

Update `swissarmyhammer-cli/src/cli.rs`:

```rust
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run as MCP server
    Serve {
        // ... args
    },
    
    /// Diagnose configuration issues
    Doctor {
        // ... args
    },
    
    // REMOVED: Implement
    // REMOVED: Plan
    
    /// Manage prompts
    Prompt {
        #[command(subcommand)]
        subcommand: PromptSubcommand,
    },
    
    // ... other commands
}
```

### 4. Update Main Dispatcher

Update `swissarmyhammer-cli/src/main.rs` to remove implement/plan handlers:

```rust
async fn handle_dynamic_matches(
    matches: ArgMatches,
    cli_tool_context: Arc<CliToolContext>,
    template_context: TemplateContext,
) -> i32 {
    match matches.subcommand() {
        Some(("serve", sub_matches)) => handle_serve(sub_matches).await,
        Some(("doctor", sub_matches)) => handle_doctor(sub_matches).await,
        // Some(("implement", sub_matches)) => handle_implement(sub_matches).await,  // REMOVED
        // Some(("plan", sub_matches)) => handle_plan(sub_matches).await,             // REMOVED
        Some((name, sub_matches)) => {
            // All other commands are dynamic (workflows or tools)
            handle_dynamic_command(name, sub_matches, context).await
        }
        None => {
            // No subcommand - default behavior or error
            EXIT_ERROR
        }
    }
}
```

### 5. Update Tests

Remove or update tests that reference static commands:

```bash
# Find tests for implement command
rg "test.*implement.*command|commands::implement" swissarmyhammer-cli/tests --type rust

# Find tests for plan command  
rg "test.*plan.*command|commands::plan" swissarmyhammer-cli/tests --type rust
```

Update tests to use dynamic shortcuts or flow commands instead.

### 6. Update Documentation

Remove references to static commands:
- Update main CLI about text
- Update README if it mentions these commands
- Ensure migration guide notes the removal

### 7. Verify No Broken Imports

```bash
# Check for any remaining imports of removed modules
rg "commands::implement|commands::plan" swissarmyhammer-cli/src --type rust

# Check for use statements
rg "use.*commands::implement|use.*commands::plan" --type rust
```

## Files to Delete

- `swissarmyhammer-cli/src/commands/implement/mod.rs`
- `swissarmyhammer-cli/src/commands/implement/description.md`
- `swissarmyhammer-cli/src/commands/plan/mod.rs`
- `swissarmyhammer-cli/src/commands/plan/description.md`

## Files to Modify

- `swissarmyhammer-cli/src/commands/mod.rs`
- `swissarmyhammer-cli/src/cli.rs`
- `swissarmyhammer-cli/src/main.rs`
- `swissarmyhammer-cli/tests/*` (update or remove tests)

## Acceptance Criteria

- [ ] Static implement and plan command modules deleted
- [ ] Commands module updated (no implement/plan imports)
- [ ] CLI enum updated (no Implement/Plan variants)
- [ ] Main dispatcher updated (no implement/plan handlers)
- [ ] Dynamic shortcuts still work: `sah implement`, `sah plan spec.md`
- [ ] Full form still works: `sah flow implement`, `sah flow plan spec.md`
- [ ] No broken imports or references
- [ ] All tests pass
- [ ] Code compiles without warnings
- [ ] Help output cleaner (no duplicate commands)

## Benefits

1. **No Duplication**: Single implementation via dynamic shortcuts
2. **Consistency**: All workflows accessed the same way
3. **Maintainability**: No hardcoded command wrappers to maintain
4. **Cleaner Code**: Removes ~200 lines of wrapper code

## Verification

After removal, verify both access methods still work:

```bash
# Dynamic shortcuts
sah implement --quiet
sah plan spec.md --interactive

# Full form
sah flow implement --quiet
sah flow plan spec.md --interactive
```

## Estimated Changes

~-200 lines of code (deletions)
~30 lines of code (updates to remove references)

## Priority

Medium - Cleanup task, no functional impact (dynamic shortcuts provide same functionality)



## Proposed Solution

After analyzing the codebase, I can confirm that both `implement` and `plan` commands are static wrappers that delegate to the flow system. They both include deprecation warnings and simply call `flow::handle_command` with the appropriate `FlowSubcommand::Execute`.

The dynamic flow system already provides these commands via shortcuts, so the static wrappers are redundant.

### Implementation Steps

1. **Delete Static Command Files**
   - Remove `swissarmyhammer-cli/src/commands/implement/` directory and all contents
   - Remove `swissarmyhammer-cli/src/commands/plan/` directory and all contents

2. **Update Module Declarations**
   - Remove `pub mod implement;` and `pub mod plan;` from `swissarmyhammer-cli/src/commands/mod.rs`

3. **Update CLI Enum**
   - Remove `Plan` and `Implement` variants from `Commands` enum in `swissarmyhammer-cli/src/cli.rs`
   - Remove associated test cases for these commands

4. **Update Main Dispatcher**
   - Remove `handle_plan_command` and `handle_implement_command` match arms from `swissarmyhammer-cli/src/main.rs`
   - Remove the handler function definitions

5. **Verify Dynamic Shortcuts Work**
   - Ensure the workflow shortcuts in `dynamic_cli.rs` still generate `implement` and `plan` commands
   - Test that `sah implement` and `sah plan spec.md` work via dynamic shortcuts

### Files to Delete
- `swissarmyhammer-cli/src/commands/implement/mod.rs`
- `swissarmyhammer-cli/src/commands/implement/description.md`
- `swissarmyhammer-cli/src/commands/plan/mod.rs`
- `swissarmyhammer-cli/src/commands/plan/description.md`

### Files to Modify
- `swissarmyhammer-cli/src/commands/mod.rs` - Remove module declarations
- `swissarmyhammer-cli/src/cli.rs` - Remove enum variants and tests
- `swissarmyhammer-cli/src/main.rs` - Remove match arms and handler functions

### Expected Outcome
After removal, users can still access these workflows via:
- `sah implement` (dynamic shortcut)
- `sah plan spec.md` (dynamic shortcut)
- `sah flow implement` (full form)
- `sah flow plan spec.md` (full form)

The removal is safe because the static commands were only thin wrappers around the flow system with deprecation warnings.


## Implementation Notes

Successfully removed all static `implement` and `plan` command wrappers from the CLI. The refactoring included:

### Files Deleted
- `swissarmyhammer-cli/src/commands/implement/mod.rs`
- `swissarmyhammer-cli/src/commands/implement/description.md`
- `swissarmyhammer-cli/src/commands/plan/mod.rs`
- `swissarmyhammer-cli/src/commands/plan/description.md`
- `swissarmyhammer-cli/tests/deprecation_warnings_test.rs` (entire test file)
- `swissarmyhammer-cli/tests/implement_command_integration_test.rs` (entire test file)
- `swissarmyhammer-cli/tests/plan_integration_tests.rs` (entire test file)

### Files Modified
- `swissarmyhammer-cli/src/commands/mod.rs` - Removed `pub mod implement;` and `pub mod plan;`
- `swissarmyhammer-cli/src/cli.rs`:
  - Removed `Plan` and `Implement` enum variants
  - Removed all 30+ test functions for plan/implement commands
  - Updated CLI about text to remove plan/implement from command list
- `swissarmyhammer-cli/src/main.rs`:
  - Removed `handle_plan_command()` and `handle_implement_command()` functions
  - Removed match arms for plan and implement in `handle_dynamic_matches()`
- `swissarmyhammer-cli/tests/in_process_test_utils.rs`:
  - Removed Plan and Implement from `can_run_in_process` matches
  - Removed mock implementations for both commands
  - Fixed unused variable warnings

### Verification
- ✅ Code compiles without errors: `cargo build` succeeded
- ✅ All 3343 tests pass: `cargo nextest run` succeeded
- ✅ Code formatted: `cargo fmt --all` applied

### Dynamic Shortcuts Still Work
Users can still access these workflows via:
- `sah implement` (dynamic shortcut from workflow system)
- `sah plan spec.md` (dynamic shortcut from workflow system)
- `sah flow implement` (full form)
- `sah flow plan spec.md` (full form)

### Lines of Code Removed
Approximately 450+ lines of code removed including:
- Static command implementations (~100 lines)
- CLI enum variants and documentation (~50 lines)
- Test cases (~300+ lines across multiple test files)

The removal is clean and complete with no breaking changes since the dynamic flow shortcuts provide identical functionality.


## Additional Issue Found

After the static command removal, there's a follow-up issue: the `RESERVED_NAMES` list in `dynamic_cli.rs` still includes `"plan"` and `"implement"`, which causes the dynamic workflow system to create both underscore-prefixed versions (`_implement`, `_plan`) AND the normal versions when there's no actual conflict.

### Current Behavior
Running `sah --help` shows:
- `plan` - Plan a specific specification file
- `implement` - Execute the implement workflow for autonomous issue resolution  
- `_implement` - Autonomously run until all issues are resolved (shortcut for 'flow implement')
- `_plan` - Turn specifications into multiple step plans (shortcut for 'flow plan')

### Root Cause
In `swissarmyhammer-cli/src/dynamic_cli.rs` at line ~1443-1453:
```rust
const RESERVED_NAMES: &[&str] = &[
    "serve",
    "doctor",
    "prompt",
    "rule",
    "flow",
    "agent",
    "validate",
    "plan",        // ← Should be removed
    "implement",   // ← Should be removed
    "list",
];
```

These were reserved when static commands existed, but now they're causing unnecessary prefixing.

### Solution
Remove `"plan"` and `"implement"` from the `RESERVED_NAMES` array in `dynamic_cli.rs` since the static commands have been deleted.

### Expected Result After Fix
Running `sah --help` should show only:
- `implement` - Autonomously run until all issues are resolved (shortcut for 'flow implement')
- `plan` - Turn specifications into multiple step plans (shortcut for 'flow plan')

No underscore-prefixed duplicates.



## Additional Fix Applied

Successfully removed duplicate command entries that were causing both underscore-prefixed and normal versions to appear.

### Changes Made
1. **Removed from RESERVED_NAMES** in `swissarmyhammer-cli/src/dynamic_cli.rs:1443-1453`:
   - Removed `"plan"` from the reserved names list
   - Removed `"implement"` from the reserved names list
   
2. **Removed hardcoded Command definitions** in `swissarmyhammer-cli/src/dynamic_cli.rs:738-790`:
   - Deleted entire `Command::new("plan")` subcommand definition (~30 lines)
   - Deleted entire `Command::new("implement")` subcommand definition (~30 lines)

3. **Updated test assertions** in `swissarmyhammer-cli/src/dynamic_cli_tests.rs:180`:
   - Removed `assert!(subcommand_names.contains(&"plan"));`
   - Removed `assert!(subcommand_names.contains(&"implement"));`
   - Added comment explaining they're now dynamic shortcuts

4. **Updated reserved names test** in `swissarmyhammer-cli/tests/workflow_shortcut_tests.rs:42-44`:
   - Removed `"plan"` from reserved array
   - Removed `"implement"` from reserved array
   - Added comment explaining the change

### Verification
- ✅ All 3343 tests pass
- ✅ Code compiles without errors or warnings
- ✅ CLI help shows only one entry for each command (no duplicates)
- ✅ Commands appear without underscore prefixes:
  - `implement` - Autonomously run until all issues are resolved (shortcut for 'flow implement')
  - `plan` - Turn specifications into multiple step plans (shortcut for 'flow plan')

### Root Cause
After removing the static command modules, there were TWO sources creating these commands:
1. Hardcoded `Command::new()` definitions in `build_cli()` method
2. Dynamic workflow shortcuts from `build_workflow_shortcuts()`

The RESERVED_NAMES list was preventing conflicts by prefixing the dynamic shortcuts with underscores, resulting in both versions appearing in the help output.

### Solution
Removed ALL three sources of duplication:
1. RESERVED_NAMES entries (stopped creating underscore versions)
2. Hardcoded Command definitions (stopped creating static versions)  
3. Updated tests to match new behavior

Now only the dynamic workflow shortcuts exist, appearing cleanly without prefixes.
