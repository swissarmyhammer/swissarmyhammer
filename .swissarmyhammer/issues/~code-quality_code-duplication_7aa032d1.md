# Rule Violation: code-quality/code-duplication

**File**: swissarmyhammer-cli/tests/todo_cli_tests.rs
**Severity**: ERROR

## Violation Message

VIOLATION
Rule: code-quality/code-duplication
File: swissarmyhammer-cli/tests/todo_cli_tests.rs
Line: 96
Severity: warning
Message: Duplicated test setup pattern appears in multiple test functions. The pattern of creating a temp directory, initializing a git repo, and creating the .swissarmyhammer directory is repeated across 17+ test functions (lines 96-104, 121-126, 145-150, 169-173, 197-201, 222-226, 256-260, 298-302, 335-339, 368-372, 407-411, 449-453, 492-496, 533-537, 577-581).
Suggestion: Extract this common setup into a helper function that returns the initialized temp directory:

```rust
/// Helper function to create a temp directory with git repo and .swissarmyhammer initialized
fn setup_todo_test_env() -> TempDir {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let temp_path = temp_dir.path();
    
    init_git_repo(temp_path);
    std::fs::create_dir_all(temp_path.join(".swissarmyhammer"))
        .expect("Failed to create .swissarmyhammer dir");
    
    temp_dir
}
```

Then replace the setup code in each test with:
```rust
let temp_dir = setup_todo_test_env();
let temp_path = temp_dir.path();
```

VIOLATION
Rule: code-quality/code-duplication
File: swissarmyhammer-cli/tests/todo_cli_tests.rs
Line: 269
Severity: warning
Message: Duplicated ID extraction logic appears in multiple test functions (lines 269-275, 322-325, 394-397, 469-473, 513-517, 560-564). The pattern of finding `"id":"` in JSON output, calculating start/end positions, and extracting the substring is repeated with identical logic.
Suggestion: Extract this into a helper function:

```rust
/// Helper function to extract todo ID from JSON output
fn extract_todo_id_from_output(output: &str) -> &str {
    let id_start = output
        .find("\"id\":\"")
        .expect("Should find id in output")
        + JSON_ID_PREFIX_LEN;
    let id_end = output[id_start..]
        .find('\"')
        .expect("Should find end of id")
        + id_start;
    &output[id_start..id_end]
}
```

Then replace the extraction code with:
```rust
let todo_id = extract_todo_id_from_output(&create_stdout);
```

VIOLATION
Rule: code-quality/code-duplication
File: swissarmyhammer-cli/tests/todo_cli_tests.rs
Line: 456
Severity: info
Message: Similar assertion pattern for checking JSON numeric fields appears multiple times (lines 456-458, 461-463, 514-516, 519-521, 597-599). The pattern checks for both compact and spaced JSON formatting of the same field.
Suggestion: Create a helper function for this common assertion pattern:

```rust
/// Helper function to assert JSON field contains expected numeric value
fn assert_json_field_value(output: &str, field: &str, value: u32) {
    let compact = format!("\"{}\":{}", field, value);
    let spaced = format!("\"{}\": {}", field, value);
    assert!(
        output.contains(&compact) || output.contains(&spaced),
        "Should show {} {} todos", value, field
    );
}
```

Then replace assertion blocks with:
```rust
assert_json_field_value(&stdout, "total", 0);
assert_json_field_value(&stdout, "pending", 0);
```

---
*This issue was automatically created by `sah rule check --create-todos`*
