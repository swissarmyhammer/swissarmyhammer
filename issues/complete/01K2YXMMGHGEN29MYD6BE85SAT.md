eliminate parameter groups
## Proposed Solution

Based on analysis of the codebase, parameter groups are implemented throughout the system but can be eliminated as follows:

### Implementation Steps

1. **Remove ParameterGroup struct** from `swissarmyhammer/src/common/parameters.rs`
   - Remove `ParameterGroup` struct definition
   - Remove related builder methods and functionality

2. **Remove parameter group methods from ParameterProvider trait**:
   - Remove `get_parameter_groups()` method
   - Remove `get_parameters_by_group()` method  
   - Remove `is_parameter_in_any_group()` method
   - Simplify trait to focus only on basic parameter provision

3. **Remove parameter groups from workflow definitions**:
   - Remove `parameter_groups` field from `Workflow` struct
   - Remove `cached_parameter_groups` field  
   - Remove `validate_parameter_groups()` method
   - Update `validate_structure()` to not call parameter group validation

4. **Remove parameter group CLI functionality**:
   - Remove group-based help generation from CLI tools
   - Simplify parameter display to show all parameters in a flat list
   - Remove group-based organization from interactive prompts

5. **Clean up tests and documentation**:
   - Remove all parameter group tests 
   - Remove parameter group examples from documentation
   - Update migration guides to reflect removal

### Benefits

- Simplifies the parameter system significantly
- Reduces code complexity and maintenance burden
- Eliminates the "general" group fallback concept
- Makes parameter handling more straightforward
- Reduces cognitive load for users and developers

### Breaking Changes

This is a breaking change that will affect:
- Workflow frontmatter containing `parameter_groups`
- Code using `get_parameter_groups()` method
- Tests expecting parameter group functionality

### Migration

Users with workflows containing `parameter_groups` should:
1. Remove the `parameter_groups` section from workflow frontmatter
2. All parameters will be displayed in a flat list automatically

## Code Review Cleanup Completed

✅ **All lint issues resolved:**

1. **parameter_cli.rs:117** - Removed unused `capitalize_words` function
2. **interactive_prompts.rs:76** - Removed unused `should_prompt_parameter` function  
3. **interactive_prompts.rs:97** - Removed unused `capitalize_words` function
4. **interactive_prompts.rs:94** - Fixed documentation formatting issue

✅ **Verification:**
- `cargo fmt --all` completed successfully
- `cargo clippy` completed with no warnings
- All dead code warnings eliminated

✅ **Final status:**
- Parameter groups elimination implementation is complete
- Code is clean with no lint warnings
- All tests passing
- Ready for integration