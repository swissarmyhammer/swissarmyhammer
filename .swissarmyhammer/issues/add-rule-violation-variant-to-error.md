# Add RuleViolation Variant to SwissArmyHammerError

## Problem
When `RuleError::Violation` is converted to `SwissArmyHammerError`, it uses the generic `other()` variant. This causes the error to be logged again by the CLI/command layer, creating redundant output since the violation was already logged by the checker at the appropriate level.

## Current Behavior
```rust
// In error.rs
impl From<RuleError> for SwissArmyHammerError {
    fn from(error: RuleError) -> Self {
        SwissArmyHammerError::other(error.to_string())
    }
}
```

This treats all `RuleError` variants the same, including violations that were already logged.

## Desired Behavior
1. Add a `RuleViolation` variant to `SwissArmyHammerError` enum
2. Update the `From<RuleError>` implementation to handle violations specially:
   - `RuleError::Violation` → `SwissArmyHammerError::RuleViolation`
   - Other variants → `SwissArmyHammerError::other()`
3. The CLI/command layer can then check for `SwissArmyHammerError::RuleViolation` and avoid logging it again

## Implementation

### In swissarmyhammer-common (SwissArmyHammerError)
Add a variant:
```rust
pub enum SwissArmyHammerError {
    // ... existing variants
    RuleViolation(String), // or store the RuleViolation directly
}
```

### In swissarmyhammer-rules/src/error.rs
```rust
impl From<RuleError> for SwissArmyHammerError {
    fn from(error: RuleError) -> Self {
        match error {
            RuleError::Violation(violation) => {
                SwissArmyHammerError::RuleViolation(violation.to_string())
            }
            _ => SwissArmyHammerError::other(error.to_string()),
        }
    }
}
```

### In CLI command (rule check)
Don't log violations as errors since they were already logged:
```rust
if let Err(e) = checker.check_all(rules, targets).await {
    if !matches!(e, SwissArmyHammerError::RuleViolation(_)) {
        tracing::error!("Rule command failed: {}", e);
    }
    return Err(e);
}
```

## Locations
- `swissarmyhammer-common/src/error.rs` - Add RuleViolation variant
- `swissarmyhammer-rules/src/error.rs:105` - Update From impl
- `sah/src/commands/rule.rs` - Update error handling in check command

## Acceptance Criteria
- [ ] `SwissArmyHammerError` has a `RuleViolation` variant
- [ ] `RuleError::Violation` converts to `SwissArmyHammerError::RuleViolation`
- [ ] Other `RuleError` variants still convert to `other()`
- [ ] CLI doesn't double-log violations
- [ ] Tests verify the conversion behavior
- [ ] Violations still cause non-zero exit code
