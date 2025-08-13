# PLAN_000007: Unit Tests for CLI Parsing

**Refer to ./specification/plan.md**

## Goal

Add comprehensive unit tests for the new `Plan` command CLI parsing, following the existing test patterns in `swissarmyhammer-cli/src/cli.rs` to ensure the command is parsed correctly with various argument formats.

## Background

The CLI module already has extensive unit tests (around 1000+ lines of tests). We need to add similar tests for the new Plan command to ensure it integrates properly with the clap parsing system and handles various input scenarios correctly.

## Requirements

1. Add unit tests following existing patterns in the file
2. Test basic plan command parsing
3. Test parameter extraction
4. Test error scenarios (missing parameters)
5. Test help text functionality
6. Test various file path formats
7. Ensure comprehensive coverage of the new command

## Implementation Details

### Test Structure

Following the existing pattern, add these tests to the `#[cfg(test)]` section:

```rust
#[test]
fn test_cli_plan_command_basic() {
    let result = Cli::try_parse_from_args(["swissarmyhammer", "plan", "specification/plan.md"]);
    assert!(result.is_ok());

    let cli = result.unwrap();
    if let Some(Commands::Plan { plan_filename }) = cli.command {
        assert_eq!(plan_filename, "specification/plan.md");
    } else {
        panic!("Expected Plan command");
    }
}

#[test]
fn test_cli_plan_command_absolute_path() {
    let result = Cli::try_parse_from_args(["swissarmyhammer", "plan", "/full/path/to/plan.md"]);
    assert!(result.is_ok());

    let cli = result.unwrap();
    if let Some(Commands::Plan { plan_filename }) = cli.command {
        assert_eq!(plan_filename, "/full/path/to/plan.md");
    } else {
        panic!("Expected Plan command");
    }
}

#[test]
fn test_cli_plan_command_relative_path() {
    let result = Cli::try_parse_from_args(["swissarmyhammer", "plan", "./plans/feature.md"]);
    assert!(result.is_ok());

    let cli = result.unwrap();
    if let Some(Commands::Plan { plan_filename }) = cli.command {
        assert_eq!(plan_filename, "./plans/feature.md");
    } else {
        panic!("Expected Plan command");
    }
}

#[test]
fn test_cli_plan_command_missing_parameter() {
    let result = Cli::try_parse_from_args(["swissarmyhammer", "plan"]);
    assert!(result.is_err());

    let error = result.unwrap_err();
    assert_eq!(error.kind(), clap::error::ErrorKind::MissingRequiredArgument);
}

#[test]
fn test_cli_plan_command_help() {
    let result = Cli::try_parse_from_args(["swissarmyhammer", "plan", "--help"]);
    assert!(result.is_err()); // Help exits with error but that's expected

    let error = result.unwrap_err();
    assert_eq!(error.kind(), clap::error::ErrorKind::DisplayHelp);
}

#[test]
fn test_cli_plan_command_with_global_flags() {
    let result = Cli::try_parse_from_args([
        "swissarmyhammer", 
        "--verbose", 
        "plan", 
        "test-plan.md"
    ]);
    assert!(result.is_ok());

    let cli = result.unwrap();
    assert!(cli.verbose);
    if let Some(Commands::Plan { plan_filename }) = cli.command {
        assert_eq!(plan_filename, "test-plan.md");
    } else {
        panic!("Expected Plan command");
    }
}

#[test]
fn test_cli_plan_command_with_debug_flag() {
    let result = Cli::try_parse_from_args([
        "swissarmyhammer", 
        "--debug", 
        "plan", 
        "debug-plan.md"
    ]);
    assert!(result.is_ok());

    let cli = result.unwrap();
    assert!(cli.debug);
    if let Some(Commands::Plan { plan_filename }) = cli.command {
        assert_eq!(plan_filename, "debug-plan.md");
    } else {
        panic!("Expected Plan command");
    }
}

#[test]
fn test_cli_plan_command_file_with_spaces() {
    let result = Cli::try_parse_from_args([
        "swissarmyhammer", 
        "plan", 
        "plan with spaces.md"
    ]);
    assert!(result.is_ok());

    let cli = result.unwrap();
    if let Some(Commands::Plan { plan_filename }) = cli.command {
        assert_eq!(plan_filename, "plan with spaces.md");
    } else {
        panic!("Expected Plan command");
    }
}

#[test]
fn test_cli_plan_command_complex_path() {
    let result = Cli::try_parse_from_args([
        "swissarmyhammer", 
        "plan", 
        "./specifications/features/advanced-feature-plan.md"
    ]);
    assert!(result.is_ok());

    let cli = result.unwrap();
    if let Some(Commands::Plan { plan_filename }) = cli.command {
        assert_eq!(plan_filename, "./specifications/features/advanced-feature-plan.md");
    } else {
        panic!("Expected Plan command");
    }
}
```

## Test Categories

### 1. Basic Functionality Tests
- Simple plan command with filename
- Parameter extraction verification
- Command enum matching

### 2. Path Format Tests
- Absolute paths
- Relative paths with `./`
- Simple filenames
- Complex directory structures
- Files with spaces in names

### 3. Integration Tests
- Plan command with global flags (`--verbose`, `--debug`, `--quiet`)
- Combined flag scenarios
- Order independence testing

### 4. Error Handling Tests
- Missing filename parameter
- Invalid argument combinations
- Help text display

### 5. Edge Cases
- Very long file paths
- Special characters in filenames
- Unicode filenames
- Empty strings (should error)

## Implementation Steps

1. Locate the test module in `swissarmyhammer-cli/src/cli.rs` (around line 931)
2. Add the new test functions following existing patterns
3. Use the same naming conventions: `test_cli_plan_command_*`
4. Follow the same assertion patterns used by other tests
5. Test all documented examples from the help text
6. Ensure comprehensive coverage of parameter handling
7. Run tests to verify they pass
8. Add any additional edge cases discovered

## Acceptance Criteria

- [ ] All basic plan command parsing tests pass
- [ ] Parameter extraction tests work correctly
- [ ] Path format tests cover all scenarios
- [ ] Global flag integration tests pass
- [ ] Error handling tests work as expected
- [ ] Help text tests function correctly
- [ ] Edge cases are covered
- [ ] Tests follow existing code patterns exactly
- [ ] All tests pass consistently

## Testing Commands

```bash
# Run all CLI tests
cargo test --package swissarmyhammer-cli cli

# Run only plan command tests
cargo test --package swissarmyhammer-cli test_cli_plan_command

# Run tests with output
cargo test --package swissarmyhammer-cli cli -- --nocapture
```

## Dependencies

- Requires CLI structure from PLAN_000001
- Must follow patterns established in existing tests
- Should integrate with existing test framework

## Notes

- Follow the exact patterns used by existing tests in the file
- Use the same assertion styles and error checking approaches
- Test names should be descriptive and consistent
- Include edge cases that might not be obvious
- Ensure tests are deterministic and don't rely on external files
- The test examples should match the help text documentation