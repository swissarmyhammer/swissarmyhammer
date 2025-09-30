# Implement Rule Check Command Core Logic

Refer to ideas/rules.md

## Goal

Implement the core logic for `sah rule check [glob...]` command.

## Context

The check command is the main entry point for running rules. It validates rules, loads files, and executes checks with fail-fast behavior.

## Implementation

1. In `check.rs`, define `CheckCommand` struct:
```rust
pub struct CheckCommand {
    pub patterns: Vec<String>,         // Glob patterns
    pub rule: Option<Vec<String>>,     // Filter by rule names
    pub severity: Option<Severity>,    // Filter by severity
    pub category: Option<String>,      // Filter by category
}
```

2. Implement `execute_check_command()`:
   - Load all rules via RuleResolver
   - Validate all rules first (fail if any invalid)
   - Apply filters (rule names, severity, category)
   - Display what will be checked
   
3. Add validation phase before checking:
```rust
println!("Validating rules...");
for rule in &rules {
    rule.validate()?;
}
println!("✓ All {} rules are valid\n", rules.len());
```

## Testing

- Test with no rules
- Test with filters
- Test validation phase
- Test with invalid rules

## Success Criteria

- [ ] CheckCommand struct defined
- [ ] Command parsing works
- [ ] Rule loading and filtering works
- [ ] Validation phase implemented
- [ ] Tests passing
