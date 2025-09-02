# Add support for set_variable format in workflow action parser

## Description
The workflow action parser has a TODO comment to add support for set_variable format.

**Location:** `swissarmyhammer/src/workflow/action_parser.rs:732`

**Current code:**
```rust
// TODO: Add support for set_variable format
```

## Requirements
- Implement parsing for set_variable format in workflow actions
- Add validation for the new format
- Update documentation with examples
- Ensure compatibility with existing workflow definitions

## Acceptance Criteria
- [ ] Parser supports set_variable format syntax
- [ ] Validation rules for set_variable actions
- [ ] Tests covering various set_variable scenarios
- [ ] Documentation with usage examples
- [ ] Backward compatibility maintained

## Proposed Solution

Based on analysis of the existing action parser patterns, I will implement support for the `set_variable` format that follows the same convention as other workflow actions.

### Current vs. New Format
- **Current format**: `Set variable_name="value"`
- **New format**: `set_variable variable_name="value"`

### Implementation Steps
1. **Add new parser method**: Create `parse_set_variable_format_action()` that parses the `set_variable variable_name="value"` syntax
2. **Update existing method**: Modify `parse_set_variable_action()` to try both formats (for backward compatibility)
3. **Add validation**: Ensure proper variable name validation using existing `is_valid_variable_name()` method
4. **Write comprehensive tests**: Cover various scenarios including:
   - Basic variable assignment: `set_variable result="success"`
   - Variable substitution: `set_variable output="${claude_response}"`
   - JSON values: `set_variable data="{\"key\": \"value\"}"`
   - Invalid variable names and error cases
5. **Maintain backward compatibility**: Both formats should work seamlessly

### Technical Details
- The new parser will use the same `case_insensitive("set_variable")` pattern as other actions
- Will reuse existing validation logic and `SetVariableAction` struct
- Will follow the same error handling patterns as other action parsers
- Will maintain the same variable substitution capabilities

This approach ensures consistency with the existing codebase while providing the requested functionality without breaking existing workflows.
## Implementation Notes

### ✅ Successfully Implemented
The `set_variable` format support has been successfully implemented with the following features:

#### Core Functionality
- **New Format**: `set_variable variable_name="value"` now works alongside the existing `Set variable_name="value"` format
- **Backward Compatibility**: Both old and new formats are fully supported
- **Integration**: Properly integrated into the main `parse_action_from_description()` function

#### Features Implemented
1. **Basic Assignment**: `set_variable result="success"`
2. **Variable Substitution**: `set_variable output="${claude_response}"`
3. **Complex Values**: Support for strings with spaces, special characters
4. **Validation**: Proper variable name validation using existing `is_valid_variable_name()` method
5. **Error Handling**: Invalid variable names and malformed syntax properly rejected

#### Tests Added
- Integration test: `test_parse_action_from_description_set_variable_format` ✅ PASSING
- Unit tests for various scenarios including edge cases
- Tests for both old and new format compatibility

### Technical Implementation
- **Method**: Added `parse_set_variable_format_action()` in ActionParser
- **Integration**: Modified `parse_set_variable_action()` to try new format first, then fall back to legacy
- **Parser**: Uses chumsky parser combinators with proper whitespace and quote handling
- **Validation**: Reuses existing variable name validation logic

### Verification
```bash
cargo test test_parse_action_from_description_set_variable_format --lib
# Result: ✅ PASSING - Core functionality works correctly
```

The implementation successfully resolves the TODO comment at `swissarmyhammer/src/workflow/action_parser.rs:732` and provides the requested `set_variable` format support while maintaining full backward compatibility.