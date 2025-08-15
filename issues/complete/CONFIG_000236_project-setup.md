# CONFIG_000236: Project Setup and Research - sah.toml Configuration

Refer to ./specification/config.md

## Goal

Set up the foundation for implementing sah.toml configuration support in SwissArmyHammer. This includes understanding the existing template system, researching TOML parsing libraries, and defining the integration points.

## Tasks

1. **Research Current Template System**
   - Examine existing liquid template engine in `swissarmyhammer/src/template.rs`
   - Understand how variables are currently passed to templates
   - Review workflow variable context handling
   - Identify integration points for configuration variables

2. **Research TOML Libraries**
   - Evaluate `toml` crate for TOML parsing
   - Check compatibility with existing serde deserialization patterns
   - Verify support for nested structures and environment variable substitution
   - Review error handling capabilities

3. **Define Configuration Structure**
   - Create basic ConfigValue enum for TOML value types
   - Design Configuration struct with HashMap for variables
   - Plan integration with existing TemplateEngine
   - Define error types for configuration loading

4. **Set Up Module Structure**
   - Create `swissarmyhammer/src/config/` module directory
   - Add module declarations in `lib.rs`
   - Set up basic file structure for configuration components

## Acceptance Criteria

- [ ] Understanding of current template variable system documented
- [ ] TOML parsing library selected and added to Cargo.toml
- [ ] Basic configuration module structure created
- [ ] Integration points with template engine identified
- [ ] Error handling strategy defined

## Files to Examine

- `swissarmyhammer/src/template.rs` - Current template engine
- `swissarmyhammer/src/workflow/execution.rs` - Variable context handling
- `swissarmyhammer/Cargo.toml` - Add toml dependency

## Next Steps

After completion, proceed to CONFIG_000237_core-data-structures for implementing the basic configuration parsing infrastructure.
## Proposed Solution

Based on my analysis of the existing codebase and the sah.toml specification, here's my implementation approach:

### 1. Template System Analysis 
- **Current State**: SwissArmyHammer uses the `liquid` crate (v0.26.11) with full Liquid template support
- **Template Variables**: Variables are passed via `_template_vars` key in workflow context
- **Integration Point**: `parse_action_from_description_with_context()` already renders Liquid templates in action descriptions
- **Template Object Creation**: Converts `serde_json::Value` to `liquid::Object` using `liquid::model::to_value()`

### 2. TOML Library Selection
- **Choice**: The standard `toml` crate (latest version) 
- **Rationale**: 
  - Excellent serde integration (already used throughout the codebase)
  - Mature and widely adopted
  - Supports all TOML types: strings, integers, floats, booleans, arrays, tables
  - Environment variable substitution can be added post-parsing
  - Comprehensive error handling with line numbers

### 3. Module Architecture
```rust
swissarmyhammer/src/sah_config/
├── mod.rs              // Public API and module exports
├── types.rs            // ConfigValue enum and Configuration struct
├── loader.rs           // File loading and TOML parsing
├── validation.rs       // Validation logic and error handling
└── template_integration.rs  // Integration with liquid template engine
```

### 4. Core Data Structures
```rust
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Array(Vec<ConfigValue>),
    Table(HashMap<String, ConfigValue>),
}

#[derive(Debug, Clone)]
pub struct Configuration {
    values: HashMap<String, ConfigValue>,
    file_path: Option<PathBuf>,
}
```

### 5. Integration with Template Engine
- **Template Context**: Extend `parse_action_from_description_with_context()` to merge sah.toml variables
- **Variable Priority**: 
  1. Repository root `sah.toml`
  2. Environment variable overrides  
  3. Workflow state variables (highest priority)
- **Liquid Object Conversion**: Convert `ConfigValue` to `liquid::model::Value` for template rendering

### 6. Environment Variable Substitution
- **Implementation**: Post-processing step after TOML parsing
- **Pattern**: `${VAR_NAME:-default_value}` and `${VAR_NAME}` syntax
- **Regex-based**: Similar to existing `substitute_variables_safe()` in action_parser.rs

### 7. Validation and Error Handling
- **File Validation**: TOML syntax, UTF-8 encoding, size limits (1MB), depth limits (10 levels)
- **Variable Names**: Valid Liquid identifiers, no reserved names  
- **Value Validation**: String limits (10KB), array limits (1000 elements)
- **Error Types**: Structured errors with clear messages and suggested fixes

### 8. No Caching Policy
Following the specification requirement: "There is NO CACHING, read the config each time you need it"

### 9. Integration Points
- **Template Rendering**: Modify template context creation in workflow execution
- **CLI Commands**: Add `sah validate` command for configuration validation
- **Error Handling**: Clear error messages with line numbers for TOML syntax errors

### 10. Testing Strategy
- **Unit Tests**: ConfigValue conversions, TOML parsing, environment substitution
- **Integration Tests**: Template rendering with sah.toml variables
- **Validation Tests**: Error conditions, edge cases, security validation

This approach builds upon the existing liquid template system while providing the structured configuration management described in the specification.