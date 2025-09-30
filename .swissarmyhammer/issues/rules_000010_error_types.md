# Define RuleViolation and RuleError Types

Refer to ideas/rules.md

## Goal

Define error types for the rules system, including RuleViolation for fail-fast behavior.

## Context

The rules system needs clear error types for different failure modes. RuleViolation is special - it's an error that represents a rule violation (used for fail-fast).

## Implementation

1. Create `src/error.rs` module
2. Define `RuleError` enum:
```rust
pub enum RuleError {
    LoadError(String),          // Can't load rule file
    ValidationError(String),    // Rule is invalid
    CheckError(String),         // Error during checking
    AgentError(String),         // LLM agent failed
    LanguageDetectionError(String),
    GlobExpansionError(String),
    Violation(RuleViolation),   // Rule violation found (for fail-fast)
}
```

3. Define `RuleViolation` struct:
```rust
pub struct RuleViolation {
    pub rule_name: String,
    pub file_path: PathBuf,
    pub severity: Severity,
    pub message: String,  // Full LLM response
}
```

4. Implement Display and std::error::Error traits
5. Add conversion methods

## Testing

- Unit tests for error creation
- Unit tests for error display

## Success Criteria

- [ ] RuleError enum defined
- [ ] RuleViolation struct defined
- [ ] Error traits implemented
- [ ] Unit tests passing
