# Rule Violation: code-quality/code-duplication

**File**: swissarmyhammer-cli/src/schema_validation.rs
**Severity**: ERROR

## Violation Message

VIOLATION
Rule: code-quality/code-duplication
File: swissarmyhammer-cli/src/schema_validation.rs
Line: 147
Severity: warning
Message: Duplicated validation logic between `validate_properties` and `validate_properties_comprehensive`. Both functions iterate over properties and call the same validation methods (`validate_parameter_name` and `validate_property_schema`), differing only in error handling strategy (fail-fast vs. collect-all).
Suggestion: Extract the common validation logic into a shared helper method that accepts a closure or enum to determine error handling strategy. For example:
```rust
fn validate_properties_with_strategy<F>(
    properties: &Map<String, Value>,
    mut error_handler: F
) -> Result<Vec<ValidationError>, ValidationError>
where F: FnMut(ValidationError) -> Result<(), ValidationError>
{
    let mut errors = Vec::new();
    for (prop_name, prop_schema) in properties {
        if let Err(e) = Self::validate_parameter_name(prop_name) {
            error_handler(e.clone())?;
            errors.push(e);
        }
        if let Err(e) = Self::validate_property_schema(prop_name, prop_schema) {
            error_handler(e.clone())?;
            errors.push(e);
        }
    }
    Ok(errors)
}
```
Then implement both public methods as thin wrappers with different error handling strategies.

---

VIOLATION
Rule: code-quality/code-duplication
File: swissarmyhammer-cli/src/schema_validation.rs
Line: 100
Severity: warning
Message: Duplicated schema validation logic between `validate_schema` and `validate_schema_comprehensive`. Both methods perform the same sequence of validations (structure, properties, property consistency, required fields) but differ only in error handling (fail-fast vs. collect-all).
Suggestion: Refactor to use a common internal method with an error collection strategy parameter. For example:
```rust
fn validate_schema_internal(
    schema: &Value,
    collect_all: bool
) -> Result<Vec<ValidationError>, ValidationError> {
    let mut errors = Vec::new();
    
    macro_rules! handle_result {
        ($result:expr) => {
            match $result {
                Err(e) => {
                    if collect_all {
                        errors.push(e);
                    } else {
                        return Err(e);
                    }
                }
                Ok(_) => {}
            }
        };
    }
    
    handle_result!(Self::validate_schema_structure(schema));
    // ... rest of validation logic
    
    if collect_all {
        Ok(errors)
    } else {
        Ok(vec![])
    }
}
```
Then implement both public methods as wrappers calling this internal method.

---
*This issue was automatically created by `sah rule check --create-todos`*
