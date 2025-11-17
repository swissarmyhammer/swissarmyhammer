# Rule Violation: code-quality/code-duplication

**File**: swissarmyhammer-cli/src/context.rs
**Severity**: ERROR

## Violation Message

VIOLATION
Rule: code-quality/code-duplication
File: swissarmyhammer-cli/src/context.rs
Line: 126-144
Severity: warning
Message: Duplicate error handling pattern for serialization failures. The `display` method contains nearly identical error handling blocks for JSON and YAML serialization, differing only in the serialization function called and the format name in the error message.
Suggestion: Extract a generic serialization helper function that accepts a closure for the serialization logic. For example:
```rust
fn serialize_and_display<T, F>(&self, items: &[T], format_name: &str, serializer: F) -> Result<()>
where
    T: serde::Serialize,
    F: FnOnce(&[T]) -> std::result::Result<String, Box<dyn std::error::Error>>,
{
    let output = serializer(items).map_err(|e| {
        swissarmyhammer_common::SwissArmyHammerError::Other {
            message: format!("Failed to serialize to {}: {}", format_name, e),
        }
    })?;
    println!("{}", output);
    Ok(())
}
```
Then use it in the match arms:
```rust
OutputFormat::Json => self.serialize_and_display(&items, "JSON", |i| {
    serde_json::to_string_pretty(i).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
}),
OutputFormat::Yaml => self.serialize_and_display(&items, "YAML", |i| {
    serde_yaml::to_string(i).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
}),
```

VIOLATION
Rule: code-quality/code-duplication
File: swissarmyhammer-cli/src/context.rs
Line: 150-158
Severity: info
Message: The `display_prompts` and `display_rules` methods contain identical logic, only differing in the type name used. Both methods simply match on a `DisplayRows` enum and delegate to the `display` method.
Suggestion: Create a generic method to handle both cases:
```rust
fn display_rows<T>(&self, rows: impl IntoDisplayRows<T>) -> Result<()>
where
    T: serde::Serialize + tabled::Tabled,
{
    match rows.into_display_rows() {
        DisplayRowsVariant::Standard(items) => self.display(items),
        DisplayRowsVariant::Verbose(items) => self.display(items),
    }
}
```
Or use a trait-based approach where `DisplayRows` enum implements a common trait that provides the match logic, eliminating the need for separate methods.

VIOLATION
Rule: code-quality/code-duplication
File: swissarmyhammer-cli/src/context.rs
Line: 277-371
Severity: info
Message: Test methods contain significant code duplication in table validation logic. Multiple tests (`test_table_alignment_with_emojis`, `test_table_with_long_content`, `test_table_with_special_characters`) share the same pattern of: rendering a table, checking for emoji presence, extracting data rows, and verifying column separators.
Suggestion: Extract shared test helper functions:
```rust
fn render_test_table(rows: &[TestRow]) -> String {
    tabled::Table::new(rows)
        .with(tabled::settings::Style::modern())
        .to_string()
}

fn verify_column_separators(table: &str, expected_emojis: &[&str]) {
    let lines: Vec<&str> = table.lines().collect();
    let data_rows: Vec<&str> = lines
        .iter()
        .filter(|line| expected_emojis.iter().any(|emoji| line.contains(emoji)))
        .copied()
        .collect();
    
    for row in &data_rows {
        assert!(
            row.contains('│'),
            "Row should contain column separators: {}",
            row
        );
    }
}
```
Then simplify each test to focus on its unique assertion logic while using the shared helpers for common operations.

---
*This issue was automatically created by `sah rule check --create-todos`*
