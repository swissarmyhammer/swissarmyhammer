# Define CLI Exclusion Attribute Macro

Refer to /Users/wballard/github/sah-marker/ideas/marker.md

## Objective

Create the foundational `#[cli_exclude]` attribute macro that can be applied to MCP tool structs to mark them for exclusion from future CLI generation.

## Implementation Tasks

### 1. Create Attribute Macro Module
- Create `swissarmyhammer-tools/src/attributes/mod.rs` with the macro definition
- Define `cli_exclude` as a simple attribute macro that accepts no parameters
- Add documentation explaining the attribute's purpose and usage

### 2. Macro Definition
```rust
/// Marks an MCP tool as excluded from CLI generation
///
/// Tools marked with this attribute are designed specifically for MCP workflow
/// operations and should not be exposed as direct CLI commands.
///
/// # Example
/// ```rust
/// #[cli_exclude]
/// #[derive(Default)]
/// pub struct WorkflowSpecificTool;
/// ```
#[proc_macro_attribute]
pub fn cli_exclude(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // For now, this is a no-op marker attribute
    // Future CLI generation will read this attribute via reflection/parsing
    item
}
```

### 3. Export Macro
- Add macro export to `swissarmyhammer-tools/src/lib.rs`
- Ensure macro is available for use in tool definitions

### 4. Add Dependencies
- Add `proc-macro = true` to `swissarmyhammer-tools/Cargo.toml` if needed
- Add `proc-macro2`, `quote`, `syn` dependencies as needed for macro processing

## Testing Requirements

### 1. Compilation Tests
- Verify the attribute compiles without errors when applied to structs
- Test that the attribute doesn't interfere with existing derive macros
- Ensure multiple attributes can be combined properly

### 2. Basic Usage Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[cli_exclude]
    #[derive(Default)]
    struct TestTool;

    #[test]
    fn test_attribute_compiles() {
        let _tool = TestTool::default();
        // If this compiles, the attribute works
    }
}
```

## Documentation

- Create comprehensive rustdoc documentation for the attribute
- Include examples of proper usage
- Document the exclusion philosophy and when to use the attribute
- Add to the codebase architecture documentation

## Acceptance Criteria

- [ ] `cli_exclude` attribute macro is defined and exported
- [ ] Attribute can be applied to MCP tool structs without compilation errors
- [ ] Attribute doesn't interfere with existing tool functionality
- [ ] Comprehensive tests verify attribute compilation
- [ ] Documentation explains usage and philosophy

## Notes

This is a foundational step that creates the attribute infrastructure. The attribute is currently a no-op marker but provides the foundation for future CLI generation systems to detect and exclude marked tools.
## Proposed Solution

I will implement the `#[cli_exclude]` attribute macro by:

### 1. Dependencies Setup
- Update `swissarmyhammer-tools/Cargo.toml` to add `proc-macro = true` and necessary proc-macro dependencies
- Add `proc-macro2`, `quote`, and `syn` as dependencies for macro processing

### 2. Attribute Module Structure
- Create `swissarmyhammer-tools/src/attributes/` directory
- Create `swissarmyhammer-tools/src/attributes/mod.rs` with the macro implementation
- Follow existing code patterns and documentation standards

### 3. Macro Implementation
- Implement `cli_exclude` as a no-op procedural attribute macro
- Add comprehensive rustdoc documentation with examples
- Ensure it properly passes through the original token stream unchanged

### 4. Module Integration
- Update `swissarmyhammer-tools/src/lib.rs` to include the new `attributes` module
- Export the `cli_exclude` macro for external usage

### 5. Testing Strategy
- Create compilation tests to ensure the attribute works with structs
- Test compatibility with existing derive macros
- Verify multiple attributes can be combined
- Test the no-op behavior (attribute doesn't change functionality)

### 6. Quality Assurance
- Run `cargo test` to verify all tests pass
- Use `cargo fmt` for code formatting
- Use `cargo clippy` to catch any potential issues

This approach creates the foundational infrastructure as requested while maintaining compatibility with existing code and following Rust procedural macro best practices.
## Implementation Notes

### Architectural Decision: Separate Proc-Macro Crate

During implementation, I discovered that Rust proc-macro crates have restrictions - they can only export procedural macros and cannot export other items like modules or structs. To maintain the existing `swissarmyhammer-tools` architecture while adding macro functionality, I created a separate `sah-marker-macros` crate.

### Structure Created

1. **New Crate**: `sah-marker-macros/`
   - Dedicated procedural macro crate with `proc-macro = true`
   - Contains the `cli_exclude` attribute macro implementation
   - Dependencies: `proc-macro2`, `quote`, `syn` for macro processing

2. **Workspace Integration**: 
   - Added `sah-marker-macros` to workspace members
   - Added as dependency to `swissarmyhammer-tools`
   - Re-exported through `swissarmyhammer-tools` for convenience

3. **Testing Strategy**:
   - Integration tests in `sah-marker-macros/tests/` (proc macros can't be tested in same crate)
   - Verification test in `swissarmyhammer-tools` to confirm re-export works
   - Comprehensive test coverage for various use cases

### Implementation Details

- **No-op Behavior**: The macro correctly passes through the input unchanged
- **Documentation**: Comprehensive rustdoc with examples and philosophy
- **Compatibility**: Works with other attributes, derives, generics, and trait implementations
- **Error Handling**: Proper syntax parsing with syn for validation

### Quality Assurance Completed

- ✅ All tests pass (including integration tests)
- ✅ Code formatted with `cargo fmt`  
- ✅ Clippy checks completed (only formatting warnings unrelated to macro)
- ✅ Macro available through `use swissarmyhammer_tools::cli_exclude;`

The foundational infrastructure is now ready for future CLI generation systems to detect and exclude marked tools.

## Code Review Resolution - 2025-08-19

### Issues Addressed
Successfully resolved all lint issues identified in the code review:

#### ✅ Fixed Format String Warnings (23 instances)
- Used `cargo clippy --fix --lib -p swissarmyhammer-tools --allow-dirty`
- Automatically converted `format!("{}", var)` to `format!("{var}")` patterns
- Files affected:
  - `swissarmyhammer-tools/src/mcp/tools/files/shared_utils.rs` (9 fixes)
  - `swissarmyhammer-tools/src/mcp/tools/files/grep/mod.rs` (8 fixes)
  - `swissarmyhammer-tools/src/mcp/tools/files/edit/mod.rs` (2 fixes)
  - `swissarmyhammer-tools/src/mcp/tools/files/glob/mod.rs` (4 fixes)

#### ✅ Verified Code Quality
- All clippy warnings resolved - `cargo clippy -p swissarmyhammer-tools` runs clean
- Code formatted with `cargo fmt --all` for consistency
- The manual contains check in grep module was already correctly using `sample.contains(&0)`
- Unit struct `.default()` calls were already optimized by previous clippy runs

#### ✅ Test Suite Validation
- All 2912 tests pass with `cargo nextest run --fail-fast`
- No regressions introduced by lint fixes
- All existing functionality preserved

### Code Quality Improvements
The automatic clippy fixes improved code readability and consistency by:
- Using more concise format string syntax (`format!("{var}")` vs `format!("{}", var)`)
- Reducing visual noise in string formatting calls
- Following modern Rust idioms and best practices

### Files Modified
- `swissarmyhammer-tools/src/mcp/tools/files/edit/mod.rs`
- `swissarmyhammer-tools/src/mcp/tools/files/glob/mod.rs` 
- `swissarmyhammer-tools/src/mcp/tools/files/grep/mod.rs`
- `swissarmyhammer-tools/src/mcp/tools/files/shared_utils.rs`
- `swissarmyhammer-tools/src/lib.rs` (minor cleanup)

### Summary
All code review items have been successfully addressed. The codebase now meets the highest quality standards with:
- Zero clippy warnings
- Consistent formatting
- All tests passing
- No functional changes - only style improvements

The `#[cli_exclude]` attribute macro implementation remains fully functional and ready for use.