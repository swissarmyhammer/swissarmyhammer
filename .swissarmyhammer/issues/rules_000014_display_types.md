# Implement Display Types for Rule Lists

Refer to ideas/rules.md

## Goal

Implement `RuleRow` and `VerboseRuleRow` display types with emoji-based source indicators.

## Context

These types provide consistent display formatting for rule lists, matching the pattern used for prompts and flows.

## Implementation

1. In `display.rs`, define display structs:
```rust
#[derive(Tabled, Serialize, Deserialize, Debug, Clone)]
pub struct RuleRow {
    pub name: String,
    pub title: String,
    pub severity: String,
    pub source: String,  // Emoji-based
}

#[derive(Tabled, Serialize, Deserialize, Debug, Clone)]
pub struct VerboseRuleRow {
    pub name: String,
    pub title: String,
    pub description: String,
    pub severity: String,
    pub category: String,
    pub source: String,  // Emoji-based
}
```

2. Implement emoji mapping (MUST MATCH prompts/flows):
   - 📦 Built-in
   - 📁 Project
   - 👤 User

3. Implement conversion functions:
   - `file_source_to_emoji()`
   - `rules_to_display_rows_with_sources()`

4. Add DisplayRows enum for different formats

## Testing

- Unit tests for display row creation
- Test emoji mapping
- Test conversion functions

## Success Criteria

- [ ] RuleRow and VerboseRuleRow defined
- [ ] Emoji mapping consistent with prompts
- [ ] Conversion functions implemented
- [ ] Unit tests passing
