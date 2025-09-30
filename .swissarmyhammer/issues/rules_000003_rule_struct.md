# Define Rule Struct and Core Types

Refer to ideas/rules.md

## Goal

Define the `Rule` struct with all necessary fields, following the pattern from prompts but adapted for rules.

## Context

The Rule struct represents a validation rule. It's similar to Prompt but has NO parameters (rules don't take arguments) and has a severity field.

## Implementation

1. In `src/rules.rs`, define `Rule` struct:
```rust
pub struct Rule {
    // Shared fields (similar to Prompt but NO parameters)
    pub name: String,
    pub template: String,              // The rule content (checking instructions)
    pub description: Option<String>,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub source: Option<PathBuf>,
    pub metadata: HashMap<String, serde_json::Value>,
    
    // Rule-specific fields
    pub severity: Severity,
    pub auto_fix: bool,                // Future: whether rule can auto-fix
}
```

2. Implement basic methods:
   - `new()` constructor
   - `is_partial()` check
   - `validate()` for rule validation

3. Add serde derives for serialization

## Testing

- Unit tests for Rule creation
- Unit tests for validation

## Success Criteria

- [ ] Rule struct fully defined
- [ ] Basic methods implemented
- [ ] Unit tests passing
- [ ] Compiles cleanly
