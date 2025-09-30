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



## Proposed Solution

I will implement the Rule struct following the pattern from the specification while keeping it consistent with the existing severity module.

### Steps:

1. **Create rules.rs module** with the Rule struct containing:
   - Shared fields: name, template, description, category, tags, source, metadata
   - Rule-specific fields: severity, auto_fix
   - No parameters field (unlike Prompt)

2. **Implement basic methods**:
   - `new()` constructor with validation
   - `is_partial()` check for partial templates
   - `validate()` for comprehensive rule validation

3. **Add proper derives**: Debug, Clone, PartialEq, Eq, Serialize, Deserialize

4. **Write comprehensive unit tests**:
   - Test Rule creation with valid and invalid data
   - Test validation logic
   - Test partial detection
   - Test serialization/deserialization

### Design Decisions:

- Using `PathBuf` for source field to match common patterns
- Using `HashMap<String, serde_json::Value>` for flexible metadata
- Following existing severity module pattern for consistency
- auto_fix defaults to false for safety
- Validation checks for: non-empty name/template, valid severity, proper partial syntax


## Implementation Complete

Successfully implemented the Rule struct and all required functionality:

### What Was Built:

1. **Created `/swissarmyhammer-rules/src/rules.rs`** with:
   - Rule struct with all specified fields (name, template, description, category, tags, source, metadata, severity, auto_fix)
   - NO parameters field (correctly following spec that rules don't take arguments)
   - Proper derives: Debug, Clone, PartialEq, Eq, Serialize, Deserialize
   - Source field marked with `#[serde(skip)]` to avoid serialization

2. **Implemented core methods**:
   - `new()` - constructor with minimum required fields
   - `is_partial()` - detects partial templates using `{% partial %}` marker
   - `validate()` - comprehensive validation checking:
     - Non-empty name
     - Non-empty template
     - Proper partial syntax if present
   - `builder()` - returns RuleBuilder for fluent construction

3. **RuleBuilder pattern**:
   - Fluent API for constructing rules with optional fields
   - Methods: description(), category(), tag(), source(), metadata_value(), auto_fix()
   - build() returns constructed Rule

4. **Updated Cargo.toml**:
   - Added serde_json dependency for metadata HashMap values

5. **Updated lib.rs**:
   - Added rules module declaration
   - Re-exported Rule and RuleBuilder

### Testing:

✅ All 18 tests passing:
- Rule creation tests
- Partial detection tests  
- Validation tests (success and failure cases)
- Builder pattern tests
- Serialization tests
- Metadata tests

✅ Clippy: No warnings
✅ Format: All code formatted

### Key Design Decisions:

- Used `SwissArmyHammerError::Other` for validation errors (no Validation variant exists)
- Metadata uses `HashMap<String, serde_json::Value>` for flexible extensibility
- auto_fix defaults to false for safety
- Source field skipped in serialization (runtime-only tracking)
- Followed existing pattern from severity.rs for consistency