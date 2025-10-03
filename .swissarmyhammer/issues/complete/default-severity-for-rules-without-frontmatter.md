# Allow Rules Without Front Matter

## Description
Rules should be valid even when they don't have front matter. When a rule is defined without front matter, the severity should default to `Error`.

## Current Behavior
- Rules may require front matter to be valid
- No default severity is applied when front matter is missing

## Desired Behavior
- Rules without front matter should be considered valid
- Default severity should be set to `Error` when not explicitly specified in front matter

## Acceptance Criteria
- [ ] Rules can be parsed and processed without front matter
- [ ] When no front matter exists, severity defaults to `Error`
- [ ] Existing rules with front matter continue to work as expected



## Proposed Solution

After analyzing the codebase, I've identified the exact location where the default severity is set:

**File:** `/Users/wballard/github/sah/swissarmyhammer-rules/src/rule_loader.rs:175`

**Current Behavior:**
```rust
Rule::new(name.to_string(), template.clone(), Severity::Info)
```

**Required Change:**
Change the default severity from `Severity::Info` to `Severity::Error` when no frontmatter metadata is present.

**Implementation Steps:**

1. **Modify `load_from_string` method** in `rule_loader.rs:175`:
   - Change `Severity::Info` to `Severity::Error` for rules without metadata
   - Keep `Severity::Warning` as the default when frontmatter exists but severity is not specified

2. **Update test expectations**:
   - `test_load_from_string_no_metadata` expects `Severity::Info`, needs to be updated to `Severity::Error`
   - Add new test case to verify rules with frontmatter but no severity field default to appropriate severity

3. **Reasoning:**
   - Rules without frontmatter should be considered critical by default (`Error`)
   - Rules with frontmatter but no severity should default to `Warning` (existing behavior)
   - This ensures safety by defaulting to the strictest severity level

4. **Files to modify:**
   - `swissarmyhammer-rules/src/rule_loader.rs` (line 175)
   - Tests in the same file




## Implementation Notes

### Changes Made

**File:** `swissarmyhammer-rules/src/rule_loader.rs`

1. **Line 195** - Changed default severity for rules without frontmatter:
   ```rust
   // Before:
   Rule::new(name.to_string(), template.clone(), Severity::Info)
   
   // After:
   Rule::new(name.to_string(), template.clone(), Severity::Error)
   ```

2. **Line 439** - Updated existing test `test_load_from_string_no_metadata`:
   ```rust
   // Changed expectation from Severity::Info to Severity::Error
   assert_eq!(rule.severity, Severity::Error);
   ```

3. **Lines 485-519** - Added two new tests:
   - `test_load_from_string_frontmatter_without_severity` - Verifies rules with frontmatter but no severity field default to `Warning`
   - `test_load_from_string_no_frontmatter_defaults_to_error` - Verifies rules without frontmatter default to `Error`

### Test Results

All tests pass successfully:
- swissarmyhammer-rules package: 149 tests passed
- Full test suite: 3225 tests passed

### Behavior Summary

The implementation now supports three scenarios:

1. **No frontmatter** → Severity defaults to `Error`
   - Example: Plain markdown file without `---` delimiters
   
2. **Frontmatter without severity field** → Severity defaults to `Warning`
   - Example: Has frontmatter with title/description but no severity
   
3. **Frontmatter with severity field** → Uses specified severity
   - Example: Explicitly sets `severity: info` in frontmatter

This ensures rules without frontmatter are treated as critical by default, meeting the acceptance criteria.

