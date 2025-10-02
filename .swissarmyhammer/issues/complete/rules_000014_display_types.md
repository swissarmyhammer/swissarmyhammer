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



## Proposed Solution

After analyzing the existing code, I found that the display types are **already implemented** in `/Users/wballard/github/sah/swissarmyhammer-cli/src/commands/rule/display.rs`. The file contains:

1. ✅ `RuleRow` struct with name, title, and source fields
2. ✅ `VerboseRuleRow` struct with name, title, description, source, and language fields  
3. ✅ Emoji mapping constants (BUILTIN_EMOJI, PROJECT_EMOJI, USER_EMOJI)
4. ✅ `file_source_to_emoji()` function matching the pattern from prompts
5. ✅ `rules_to_display_rows_with_sources()` conversion function
6. ✅ `DisplayRows` enum for different formats
7. ✅ Comprehensive unit tests

**However**, I notice the implementation has a minor discrepancy with the issue specification:

### Discrepancy Found

The issue specification requests these fields for `RuleRow`:
- name
- title
- **severity**
- source

But the current implementation has:
- name
- title
- source (missing severity)

The issue also requests these fields for `VerboseRuleRow`:
- name
- title
- description
- **severity**
- **category**
- source

But the current implementation has:
- name
- title
- description
- source
- **language** (instead of category)
- (missing severity)

### Required Changes

1. Add `severity` field to `RuleRow`
2. Add `severity` field to `VerboseRuleRow`
3. Change `language` field to `category` in `VerboseRuleRow` (to match specification)
4. Update conversion functions to populate severity
5. Update all unit tests to reflect these changes




## Implementation Complete

Successfully implemented the display types for rule lists with the following changes:

### Changes Made

1. **Updated `RuleRow` struct** - Added `severity` field between title and source
2. **Updated `VerboseRuleRow` struct** - Added `severity` field and changed `language` to `category` to match specification
3. **Updated conversion functions**:
   - `RuleRow::from_rule_with_source()` - Now populates severity using `format!("{:?}", rule.severity).to_lowercase()`
   - `VerboseRuleRow::from_rule_with_source()` - Now populates both severity and category fields correctly
4. **Updated all unit tests** - All 11 tests in the display module now verify the new fields

### Verification

- ✅ Compilation successful with `cargo build`
- ✅ All 3136 tests pass with `cargo nextest run`
- ✅ Emoji mapping consistent with prompts (📦 Built-in, 📁 Project, 👤 User)
- ✅ Display structures match specification exactly

### Files Modified

- `/Users/wballard/github/sah/swissarmyhammer-cli/src/commands/rule/display.rs`

