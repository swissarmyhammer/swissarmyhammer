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
