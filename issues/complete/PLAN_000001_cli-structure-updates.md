# PLAN_000001: CLI Structure Updates

**Refer to ./specification/plan.md**

## Goal

Add the new `Plan` command to the CLI enum structure in `swissarmyhammer-cli/src/cli.rs` with proper documentation and parameter definitions.

## Background

The swissarmyhammer CLI currently supports various subcommands like `serve`, `prompt`, `flow`, `issue`, etc. We need to add a new `plan` subcommand that accepts a single file path parameter to execute planning workflows on specific specification files.

## Requirements

1. Add `Plan` command variant to the `Commands` enum
2. Include comprehensive documentation following existing patterns
3. Define `plan_filename` parameter with proper validation
4. Follow existing CLI documentation style and examples
5. Ensure proper integration with clap parsing

## Implementation Details

### CLI Enum Addition

Add to `Commands` enum in `swissarmyhammer-cli/src/cli.rs`:

```rust
/// Plan a specific specification file
#[command(long_about = "
Execute planning workflow for a specific specification file.
Takes a path to a markdown specification file and generates implementation steps.

Basic usage:
  swissarmyhammer plan <plan_filename>    # Plan specific file

The planning workflow will:
- Read the specified plan file
- Generate step-by-step implementation issues
- Create numbered issue files in ./issues directory

Examples:
  swissarmyhammer plan ./specification/new-feature.md
  swissarmyhammer plan /path/to/custom-plan.md
  swissarmyhammer plan plans/database-migration.md
")]
Plan {
    /// Path to the plan file to process
    plan_filename: String,
},
```

### Implementation Steps

1. Open `swissarmyhammer-cli/src/cli.rs`
2. Locate the `Commands` enum (around line 92)
3. Add the new `Plan` command variant after existing commands
4. Follow the exact documentation pattern used by other commands
5. Ensure the parameter name matches the specification: `plan_filename`
6. Include comprehensive help text with usage examples

## Acceptance Criteria

- [ ] `Plan` command added to `Commands` enum
- [ ] Parameter named `plan_filename` of type `String`
- [ ] Comprehensive help documentation following existing patterns
- [ ] Examples included in help text
- [ ] CLI parsing works for the new command: `swissarmyhammer plan <file>`

## Testing

- Verify CLI parsing accepts the new command
- Confirm help text displays correctly: `swissarmyhammer plan --help`
- Ensure parameter is captured properly

## Dependencies

- None - this is foundational work

## Proposed Solution

After examining the current CLI structure in swissarmyhammer-cli/src/cli.rs, I will:

1. Add the `Plan` command variant to the `Commands` enum (around line 362, after the Config command)
2. Follow the exact documentation pattern used by existing commands like `Issue`, `Memo`, etc.
3. Include comprehensive help text with proper examples and formatting
4. Ensure the parameter name `plan_filename` matches the specification exactly
5. Test the CLI parsing to ensure it works correctly

The implementation will be added to the Commands enum with proper clap attributes and documentation following the established patterns in the codebase.

## Notes

- This step only adds the CLI structure
- Command handler implementation comes in later steps
- Follow existing documentation patterns exactly
- The long_about text should be comprehensive and helpful