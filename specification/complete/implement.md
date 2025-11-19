# OBSOLETE: sah implement Command

**Status**: This specification is obsolete. The `implement` workflow has been removed.

**Replacement**: Use `sah do` to work through todos created by `sah plan`.

**Migration**: The issue-based workflow system was replaced by the rules + todos system:
- Issues → Todos (ephemeral tasks)
- No separate tracking needed → Rules (permanent acceptance criteria)

---

# Original Specification

_The content below describes the removed `sah implement` command for historical reference._

## Overview

Add a top-level `sah implement` command that provides a convenient shortcut to run the implement workflow, similar to how `sah plan` is a shortcut for `sah flow run plan`.

## Current State

Currently, users must run:
```bash
sah flow run implement
```

## Desired State

Users should be able to run:
```bash
sah implement
```

Which should be equivalent to running `sah flow run implement`.

## Command Design

### Basic Usage
```bash
sah implement
```

## Implementation Pattern

Follow the same pattern as the existing `sah plan` command:

1. **CLI Definition**: Add `Implement` variant to the `Commands` enum in `swissarmyhammer-cli/src/cli.rs`
2. **Command Handler**: Add `run_implement()` function in `swissarmyhammer-cli/src/main.rs`
3. **Workflow Integration**: The handler should create a `FlowSubcommand::Run` with workflow name "implement"
4. **Validation**: Ensure the implement workflow exists before attempting to run it

## File Changes Required

### 1. CLI Structure (`swissarmyhammer-cli/src/cli.rs`)
```rust
pub enum Commands {
    // ... existing commands
    Plan { plan_filename: String },
    Implement,  // Add this
    // ... rest of commands
}
```

### 2. Main Handler (`swissarmyhammer-cli/src/main.rs`)
```rust
async fn run_implement() -> i32 {
    use cli::FlowSubcommand;
    use flow;
    
    let subcommand = FlowSubcommand::Run {
        workflow: "implement".to_string(),
        vars: Vec::new(),
        set: Vec::new(),
        interactive: false,
        dry_run: false,
        test: false,
        timeout: None,
        quiet: false,
    };
    
    match flow::run_flow_command(subcommand).await {
        Ok(_) => EXIT_SUCCESS,
        Err(e) => {
            // Handle errors similar to run_plan()
            tracing::error!("Implement workflow error: {}", e);
            EXIT_ERROR
        }
    }
}
```

### 3. Command Routing
Add to the main command match in `main()`:
```rust
Some(Commands::Implement) => {
    tracing::info!("Running implement command");
    run_implement().await
}
```

## Workflow Requirements

The implementation assumes that an "implement" workflow exists in the builtin workflows. If it doesn't exist yet, it needs to be created at `builtin/workflows/implement.md`.

## User Experience

This command provides:
- **Consistency**: Matches the pattern of `sah plan`
- **Convenience**: Shorter command for common workflow
- **Discoverability**: Top-level command is easier to find in help output
- **Future Extensibility**: Can add implement-specific options later if needed

## Testing Considerations

1. Verify the command appears in `sah --help`
2. Test that `sah implement` runs the implement workflow
3. Ensure error handling matches other workflow commands
4. Test that the command fails gracefully if the implement workflow doesn't exist

## Documentation Updates

- Update CLI help text
- Add to command reference documentation
- Include in examples/tutorials if appropriate

## Notes

- This follows the established pattern from the `sah plan` command
- The implementation should be minimal and delegate to the existing flow infrastructure
- Consider whether implement-specific validation or preprocessing is needed (like plan does with file validation)