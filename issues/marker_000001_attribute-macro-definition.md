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