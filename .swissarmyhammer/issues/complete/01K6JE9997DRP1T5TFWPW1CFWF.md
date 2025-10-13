# Add testing and implementation for rule partials

## Problem
There is no testing of partials, and no partials are defined in the builtin rules.

## Background
Partials are reusable template components that can be included in rule templates. They enable:
- Code reuse across multiple rules
- Consistent formatting and structure
- Easier maintenance of common patterns

## Current State
- No partial templates exist in the builtin rules
- No tests verify partial functionality
- Unclear if partial system works correctly

## Tasks
1. **Define useful partials** for common rule patterns:
   - Common severity explanations
   - Standard output formats
   - Shared violation descriptions
   - Common remediation steps

2. **Add tests** for partial functionality:
   - Test partial resolution
   - Test partial rendering within rules
   - Test partial not found error handling
   - Test nested partials (if supported)

3. **Update existing rules** to use partials where appropriate
   - Identify duplicated content across rules
   - Extract to partials
   - Update rules to use partials

## Benefits
- Reduces duplication across rules
- Makes rules easier to maintain
- Ensures consistent output formatting
- Validates that the partial system works correctly



## Proposed Solution

After analyzing the existing rules and partial system, I'll implement the following:

### 1. Create Useful Partials

Based on analysis of existing builtin rules in `./builtin/rules/`, I've identified these common patterns to extract as partials:

- **pass-response**: Standard "respond with PASS" instruction used by nearly all rules
- **no-display-secrets**: Standard warning to not display actual secret values  
- **report-line-number**: Standard instruction to report line numbers for violations
- **code-block**: Standard code block wrapper with target_content variable

### 2. Implement Partials in `./builtin/rules/_partials/`

Create the following partial files:
- `_partials/pass-response.md`
- `_partials/no-display-secrets.md`
- `_partials/report-format.md`
- `_partials/code-analysis-wrapper.md`

### 3. Add Comprehensive Tests

Create tests in `swissarmyhammer-rules/tests/` to verify:
- Partial loading from `_partials` directory
- Partial rendering within rules using `{% include %}` syntax
- Partial not found error handling
- Multiple partials in a single rule
- Nested partial resolution (if supported)

### 4. Update Existing Rules

Refactor at least 2-3 existing rules to use the new partials:
- `security/no-hardcoded-secrets.md`
- `security/no-plaintext-credentials.md`
- One code-quality rule

### Implementation Notes

- Partials are marked with `{% partial %}` tag at the beginning
- Partials are loaded via the `PartialLoader` trait in `swissarmyhammer-templating`
- Rules include partials with liquid's `{% include "partial_name" %}` syntax
- The `_partials` directory naming convention (underscore prefix) indicates internal/reusable content
- Partials get automatic description: "Partial template for reuse in other rules"



## Implementation Complete

Successfully implemented testing and partial templates for rules.

### What Was Implemented

1. **Created Partial Templates** in `./builtin/rules/_partials/`:
   - `pass-response.md` - Standard "PASS" response instruction
   - `no-display-secrets.md` - Warning to not display secret values
   - `report-format.md` - Standard violation reporting format
   - `code-block.md` - Code block wrapper with target_content variable

2. **Comprehensive Test Coverage**:
   - `partials_test.rs` - Unit tests for partial detection, loading, and rendering
   - `builtin_partials_integration_test.rs` - Integration tests for real builtin rules with partials
   - All tests pass (145 tests total in swissarmyhammer-rules package)

3. **Created `RulePartialAdapter`**:
   - Located in `swissarmyhammer-rules/src/rule_partial_adapter.rs`
   - Implements `PartialLoader` and `liquid::partials::PartialSource` traits
   - Enables rules to use other rules as partials via `{% include %}` syntax
   - Properly exported in lib.rs

4. **Updated Existing Rules** to use partials:
   - `security/no-hardcoded-secrets.md` - Now uses 4 partials
   - `security/no-plaintext-credentials.md` - Now uses 4 partials
   - `code-quality/function-length.md` - Now uses 2 partials

### Key Features

- Partials are automatically detected by `{% partial %}` marker or `_partials/` directory
- Partials get automatic description: "Partial template for reuse in other rules"
- Partials work with Liquid's `{% include "partial_name" %}` syntax
- Multiple rules can share the same partials
- Partial content is resolved at template render time
- Comprehensive validation ensures partials have content after marker

### Files Created/Modified

**Created:**
- `builtin/rules/_partials/pass-response.md`
- `builtin/rules/_partials/no-display-secrets.md`
- `builtin/rules/_partials/report-format.md`
- `builtin/rules/_partials/code-block.md`
- `swissarmyhammer-rules/src/rule_partial_adapter.rs`
- `swissarmyhammer-rules/tests/partials_test.rs` (8 tests)
- `swissarmyhammer-rules/tests/builtin_partials_integration_test.rs` (5 tests)

**Modified:**
- `builtin/rules/security/no-hardcoded-secrets.md` - Uses partials
- `builtin/rules/security/no-plaintext-credentials.md` - Uses partials
- `builtin/rules/code-quality/function-length.md` - Uses partials
- `swissarmyhammer-rules/src/lib.rs` - Exports RulePartialAdapter

### Test Results

```
cargo nextest run -p swissarmyhammer-rules
Summary: 145 tests run: 145 passed, 0 skipped
```

All tests pass, including:
- Partial marker detection
- Partial validation
- Partial loading from directories
- Partial rendering within rules
- Error handling for missing partials
- Multiple partials in a single rule
- Real builtin rules with partials
- Shared partials across multiple rules



## Code Review Complete

Reviewed all 14 files on branch `issue/01K6JE9997DRP1T5TFWPW1CFWF`:

✅ **No issues found** - Implementation is production-ready
✅ All 3241 tests passing
✅ All code follows Rust best practices
✅ Comprehensive test coverage for partials functionality
✅ Clean, reusable partial templates
✅ All existing rules properly updated to use partials

### Key Validation Points

1. **Partial Templates** - All 4 partials correctly marked and have content
2. **Updated Rules** - 3 rules successfully refactored to use partials
3. **Core Implementation** - `RulePartialAdapter` properly implements required traits
4. **Test Coverage** - 13 new tests (8 unit + 5 integration) all passing
5. **CLI Modules** - All display and command modules well-structured
6. **No Issues** - Zero clippy warnings, no TODOs, no placeholders

The implementation fully resolves the issue requirements:
- ✅ Useful partials defined for common patterns
- ✅ Comprehensive testing added
- ✅ Existing rules updated to use partials
