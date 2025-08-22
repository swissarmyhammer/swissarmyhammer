# TemplateContext Implementation

Refer to /Users/wballard/github/swissarmyhammer/ideas/config.md

## Objective

Implement a comprehensive TemplateContext struct that replaces the current HashMap-based approach for template variable management across prompts, workflows, and actions.

## Context

The specification calls for moving away from raw HashMap usage to a proper TemplateContext object that can be used consistently across the template rendering system. This context should handle merging, environment variable substitution, and integration with the liquid template engine.

## Current Usage Analysis

From the existing code in `src/sah_config/template_integration.rs`, the current system:
- Uses `HashMap<String, serde_json::Value>` as context
- Stores template vars under `_template_vars` key
- Merges configuration with workflow state variables
- Supports environment variable substitution with `${VAR}` and `${VAR:-default}` patterns
- Workflow variables have highest priority (override config)

## Architecture

```mermaid
graph TD
    A[TemplateContext] --> B[vars HashMap]
    A --> C[Environment Substitution]
    A --> D[Liquid Integration]
    A --> E[Merge Operations]
    B --> F[String Keys]
    B --> G[serde_json::Value Values]
    C --> H[${VAR} Patterns]
    C --> I[${VAR:-default} Patterns]
    E --> J[Priority-based Merging]
    D --> K[liquid::Object Conversion]
```

## Tasks

### 1. Core TemplateContext Structure

Define in `src/context.rs`:

```rust
/// Template context for rendering prompts, workflows, and actions
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TemplateContext {
    vars: HashMap<String, serde_json::Value>,
    // Optional: metadata about variable sources for debugging
    #[cfg(debug_assertions)]
    var_sources: HashMap<String, String>,
}

impl TemplateContext {
    /// Create empty context
    pub fn new() -> Self { ... }
    
    /// Create with initial variables
    pub fn with_vars(vars: HashMap<String, serde_json::Value>) -> Self { ... }
    
    /// Create from configuration only
    pub fn from_config(config_vars: HashMap<String, serde_json::Value>) -> Self { ... }
}
```

### 2. Variable Access Methods

Implement getter/setter methods:

```rust
impl TemplateContext {
    /// Get template variable value
    pub fn get(&self, key: &str) -> Option<&serde_json::Value> { ... }
    
    /// Get variable as specific type
    pub fn get_string(&self, key: &str) -> Option<String> { ... }
    pub fn get_bool(&self, key: &str) -> Option<bool> { ... }
    pub fn get_number(&self, key: &str) -> Option<f64> { ... }
    
    /// Set template variable
    pub fn set<K, V>(&mut self, key: K, value: V) 
    where 
        K: Into<String>,
        V: Into<serde_json::Value> { ... }
    
    /// Insert multiple variables
    pub fn extend(&mut self, vars: HashMap<String, serde_json::Value>) { ... }
    
    /// Check if variable exists
    pub fn contains_key(&self, key: &str) -> bool { ... }
    
    /// Get all variable keys
    pub fn keys(&self) -> impl Iterator<Item = &String> { ... }
}
```

### 3. Merging and Priority

Implement merging with proper precedence:

```rust
impl TemplateContext {
    /// Merge with another context, giving priority to the other context
    /// (other context variables override self variables)
    pub fn merge(&mut self, other: &TemplateContext) { ... }
    
    /// Merge with configuration context (config has lower priority)
    pub fn merge_config(&mut self, config_vars: HashMap<String, serde_json::Value>) { ... }
    
    /// Merge with workflow variables (workflow has higher priority)
    pub fn merge_workflow(&mut self, workflow_vars: HashMap<String, serde_json::Value>) { ... }
    
    /// Create merged context without modifying self
    pub fn merged_with(&self, other: &TemplateContext) -> TemplateContext { ... }
}
```

### 4. Environment Variable Substitution

Port and enhance the existing substitution logic:

```rust
impl TemplateContext {
    /// Substitute environment variables in all values
    /// Supports ${VAR} and ${VAR:-default} patterns
    pub fn substitute_env_vars(&mut self) { ... }
    
    /// Get context with environment variables substituted (non-mutating)
    pub fn with_env_substitution(&self) -> TemplateContext { ... }
    
    /// Substitute environment variables in a single value
    fn substitute_env_vars_in_value(&self, value: &mut serde_json::Value) { ... }
    
    /// Substitute environment variables in a string
    fn substitute_env_vars_in_string(&self, s: &str) -> String { ... }
}
```

### 5. Liquid Template Integration

Provide seamless integration with liquid templates:

```rust
impl TemplateContext {
    /// Convert to liquid::Object for template rendering
    pub fn to_liquid_object(&self) -> liquid::Object { ... }
    
    /// Create from liquid::Object
    pub fn from_liquid_object(obj: liquid::Object) -> Self { ... }
}

// Implement conversion traits
impl From<TemplateContext> for liquid::Object { ... }
impl From<liquid::Object> for TemplateContext { ... }
impl From<HashMap<String, serde_json::Value>> for TemplateContext { ... }
impl From<TemplateContext> for HashMap<String, serde_json::Value> { ... }
```

### 6. Compatibility Layer

Maintain compatibility with existing HashMap usage:

```rust
impl TemplateContext {
    /// Extract as HashMap for compatibility with existing code
    pub fn as_hashmap(&self) -> &HashMap<String, serde_json::Value> { ... }
    
    /// Convert to HashMap (consuming)
    pub fn into_hashmap(self) -> HashMap<String, serde_json::Value> { ... }
    
    /// Get variables in the legacy `_template_vars` format
    pub fn as_legacy_context(&self) -> HashMap<String, serde_json::Value> {
        // Returns HashMap with "_template_vars" key containing all variables
    }
}
```

### 7. Comprehensive Testing

Create tests in `src/tests/context_tests.rs`:
- Variable getting/setting
- Merging with different precedence scenarios
- Environment variable substitution
- Liquid integration
- Legacy compatibility
- Complex nested structures
- Edge cases (empty contexts, null values, etc.)

## Acceptance Criteria

- [ ] Complete TemplateContext struct with all methods
- [ ] Variable access with type-safe getters
- [ ] Proper merging with precedence rules
- [ ] Environment variable substitution matching existing behavior
- [ ] Liquid template engine integration
- [ ] Compatibility layer for existing HashMap usage
- [ ] Comprehensive test coverage (>95%)
- [ ] All tests passing with `cargo nextest run`
- [ ] Clean `cargo clippy` output
- [ ] Documentation with examples for all public methods

## Implementation Notes

- Preserve exact behavior of existing environment variable substitution
- Use `regex` crate with thread-local caching for performance
- Ensure deterministic ordering for testing
- Consider using `IndexMap` if ordered keys are needed
- Add debug-only metadata tracking for troubleshooting
- Make the API ergonomic for common use cases

## Files Changed

- `swissarmyhammer-config/src/lib.rs` (add context module)
- `swissarmyhammer-config/src/context.rs` (new)
- `swissarmyhammer-config/src/tests/context_tests.rs` (new)
- `swissarmyhammer-config/Cargo.toml` (add regex, liquid dependencies)
## Proposed Solution

After analyzing the existing code in `swissarmyhammer-config`, I found that a `TemplateContext` struct already exists with basic functionality. I've extended it to implement all the requirements from the specification:

### Implementation Approach

1. **Enhanced Core TemplateContext Structure**: 
   - Extended the existing struct with additional methods while maintaining backward compatibility
   - Preserved the `HashMap<String, serde_json::Value>` backend as specified
   - Added comprehensive type-safe getter methods

2. **Variable Access Methods**:
   - ✅ `get()` - already existed
   - ✅ `get_string()`, `get_bool()`, `get_number()` - added with intelligent type coercion
   - ✅ `set()` - enhanced to accept generic types implementing `Into<serde_json::Value>`
   - ✅ `extend()`, `contains_key()`, `keys()` - added for comprehensive access

3. **Merging with Proper Precedence**:
   - ✅ `merge()` - already existed (other context overrides self)
   - ✅ `merge_config()` - config has lower priority than existing workflow vars
   - ✅ `merge_workflow()` - workflow has higher priority, overrides existing vars
   - ✅ `merged_with()` - non-mutating version that creates a new context

4. **Environment Variable Substitution**:
   - ✅ `substitute_env_vars()` - already existed, preserves exact behavior
   - ✅ `with_env_substitution()` - added non-mutating version
   - ✅ Supports `${VAR}` and `${VAR:-default}` patterns in nested structures

5. **Liquid Template Integration**:
   - ✅ `to_liquid_object()` - already existed
   - ✅ `From<liquid::Object>` and `Into<liquid::Object>` traits added
   - ✅ Bidirectional conversion with proper type mapping

6. **Compatibility Layer**:
   - ✅ `as_hashmap()` - reference access to internal HashMap
   - ✅ `into_hashmap()` - consuming conversion to HashMap
   - ✅ `as_legacy_context()` - creates `_template_vars` format for existing code
   - ✅ `From<HashMap>` and `Into<HashMap>` traits for seamless integration

7. **Factory Methods**:
   - ✅ `new()` - already existed
   - ✅ `with_vars()` - already existed  
   - ✅ `from_config()` - added for config-only initialization

### Key Design Decisions

- **Preserved existing functionality**: All existing methods work exactly as before
- **Added intelligent type coercion**: `get_string()` can convert numbers/bools to strings, `get_bool()` handles string representations
- **Maintained precedence semantics**: Config has lowest priority, workflow variables have highest priority
- **Non-destructive operations**: Added `merged_with()` and `with_env_substitution()` for functional-style operations
- **Comprehensive error handling**: Environment variable substitution properly handles missing variables and provides clear error messages

### Testing Strategy

Added comprehensive test coverage for all new functionality:
- Type-safe getters with edge cases
- All merging scenarios with proper precedence verification
- Environment variable substitution in complex nested structures
- Conversion traits bidirectional testing
- Compatibility layer verification
- Legacy format compliance

### Integration Points

The enhanced `TemplateContext` can now:
1. Replace all `HashMap<String, serde_json::Value>` usage in template rendering
2. Integrate seamlessly with existing `merge_config_into_context()` function
3. Provide a clean API for prompt, workflow, and action template rendering
4. Maintain backward compatibility with existing code via compatibility methods

This implementation fully satisfies the specification requirements while building on the existing solid foundation.