# Add Comprehensive Prompt Integration Tests

## Problem

The current prompt command integration testing is incomplete. Critical command combinations are not tested and are failing in practice.

## Missing Test Coverage

### Basic Commands Not Working
```bash
cargo run -- prompt test say-hello      # FAILING - not tested
cargo run -- prompt                     # Basic help - needs verification
cargo run -- prompt list                # Basic list - needs verification
cargo run -- prompt test --help         # Test help - needs verification
```

### Global Argument Combinations
```bash
cargo run -- --verbose prompt list      # Global verbose
cargo run -- --format=json prompt list  # Global format
cargo run -- --format=yaml prompt list  # Global format YAML
```

### Prompt Test Variations
```bash
cargo run -- prompt test say-hello                    # Test existing prompt
cargo run -- prompt test say-hello --var name=World   # Test with variables
cargo run -- prompt test nonexistent                  # Test error handling
```

## Current State

- Limited smoke tests in existing files
- Missing tests for actual prompt testing functionality
- No validation that commands produce expected output
- No tests for error scenarios

## Goals

Create comprehensive integration tests that verify:
1. All prompt commands execute without errors
2. Commands produce expected output format
3. Error cases are handled gracefully
4. Global arguments work correctly with prompt commands

## Implementation

### 1. Expand Existing Smoke Tests

**File**: `swissarmyhammer-cli/tests/prompt_smoke_tests.rs` (if exists) or create new file

```rust
use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_prompt_command_shows_help() {
    Command::cargo_bin("sah")
        .unwrap()
        .arg("prompt")
        .assert()
        .success()
        .stdout(predicate::str::contains("prompt"));
}

#[test]
fn test_prompt_list_basic() {
    Command::cargo_bin("sah")
        .unwrap()
        .arg("prompt")
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

#[test]
fn test_prompt_test_help() {
    Command::cargo_bin("sah")
        .unwrap()
        .arg("prompt")
        .arg("test")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("test"));
}

#[test]
fn test_prompt_test_say_hello() {
    Command::cargo_bin("sah")
        .unwrap()
        .arg("prompt")
        .arg("test")
        .arg("say-hello")
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

#[test]
fn test_prompt_test_with_variable() {
    Command::cargo_bin("sah")
        .unwrap()
        .arg("prompt")
        .arg("test")
        .arg("say-hello")
        .arg("--var")
        .arg("name=World")
        .assert()
        .success()
        .stdout(predicate::str::contains("World"));
}

#[test]
fn test_prompt_test_nonexistent_prompt() {
    Command::cargo_bin("sah")
        .unwrap()
        .arg("prompt")
        .arg("test")
        .arg("definitely-does-not-exist-12345")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found").or(predicate::str::contains("failed")));
}

#[test]
fn test_global_verbose_with_prompt_list() {
    Command::cargo_bin("sah")
        .unwrap()
        .arg("--verbose")
        .arg("prompt")
        .arg("list")
        .assert()
        .success();
}

#[test]
fn test_global_format_json_with_prompt_list() {
    Command::cargo_bin("sah")
        .unwrap()
        .arg("--format")
        .arg("json")
        .arg("prompt")
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::starts_with("[").or(predicate::str::starts_with("{")));
}

#[test]
fn test_global_format_yaml_with_prompt_list() {
    Command::cargo_bin("sah")
        .unwrap()
        .arg("--format")
        .arg("yaml")
        .arg("prompt")
        .arg("list")
        .assert()
        .success();
}

#[test]
fn test_prompt_list_shows_builtin_prompts() {
    let output = Command::cargo_bin("sah")
        .unwrap()
        .arg("prompt")
        .arg("list")
        .assert()
        .success()
        .get_output();
        
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    // Should show some builtin prompts like say-hello
    assert!(stdout.contains("say-hello") || stdout.contains("Available prompts:"),
        "Should list prompts or show appropriate message. Got: {}", stdout);
}
```

### 2. Create Error Scenario Tests

```rust
#[test]
fn test_invalid_prompt_subcommand() {
    Command::cargo_bin("sah")
        .unwrap()
        .arg("prompt")
        .arg("invalid-subcommand")
        .assert()
        .failure();
}

#[test]
fn test_prompt_test_missing_prompt_name() {
    Command::cargo_bin("sah")
        .unwrap()
        .arg("prompt")
        .arg("test")
        .assert()
        .failure()
        .stderr(predicate::str::contains("required").or(predicate::str::contains("missing")));
}

#[test]
fn test_invalid_global_format() {
    Command::cargo_bin("sah")
        .unwrap()
        .arg("--format")
        .arg("invalid")
        .arg("prompt")
        .arg("list")
        .assert()
        .failure();
}
```

## Success Criteria

1. ✅ All basic prompt commands execute successfully
2. ✅ `cargo run -- prompt test say-hello` works
3. ✅ Commands produce expected output to stdout
4. ✅ Error cases return proper exit codes and error messages
5. ✅ Global arguments work with prompt commands
6. ✅ JSON/YAML output formats work correctly
7. ✅ Tests catch regressions in prompt command functionality

## Files Created

- `swissarmyhammer-cli/tests/prompt_integration_tests.rs` - Comprehensive prompt CLI tests

## Dependencies

- Requires basic prompt commands to be working
- Should be run after architectural cleanup

---

**Priority**: High - Verify functionality works
**Estimated Effort**: Medium
**Dependencies**: cli_prompt_000014_complete_architectural_cleanup

## Proposed Solution

After investigating the failing commands, I discovered and fixed the root cause issue:

### Root Cause Analysis

The problem was in the CLI argument parsing logic. The dynamic CLI builder defined the variable argument as:
```rust
Arg::new("vars").long("var")
```

This means:
- The **argument ID** is `"vars"` 
- The **long flag** is `"--var"`

However, the parsing code in `cli.rs` was trying to access the argument using the wrong identifier:
```rust
// WRONG - was trying to access "var"
.get_many::<String>("var")

// CORRECT - should access "vars" 
.get_many::<String>("vars")
```

### Fix Applied

I updated `swissarmyhammer-cli/src/commands/prompt/cli.rs` line 75 to use the correct argument identifier:

```rust
vars: sub_matches
    .get_many::<String>("vars")  // Fixed: changed from "var" to "vars"
    .map(|vals| vals.cloned().collect())
    .unwrap_or_default(),
```

### Verification

After the fix, all the failing commands now work correctly:

✅ `cargo run -- prompt test say-hello` - **WORKING**  
✅ `cargo run -- prompt` - **WORKING**  
✅ `cargo run -- prompt list` - **WORKING**  
✅ `cargo run -- prompt test --help` - **WORKING**  
✅ `cargo run -- prompt test say-hello --var name=World` - **WORKING**  
✅ `cargo run -- --verbose prompt list` - **WORKING**  
✅ `cargo run -- --format=json prompt list` - **WORKING**  
✅ Error handling: `cargo run -- prompt test nonexistent-prompt` - **WORKING**  

### Next Steps

Now that the core functionality is fixed, I will create comprehensive integration tests to prevent regressions and ensure all command combinations work correctly as specified in the original issue requirements.
