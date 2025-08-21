# CONFIG_000237: Core Data Structures - sah.toml Configuration

Refer to ./specification/config.md

## Goal

Implement the core data structures for sah.toml configuration support, including TOML parsing, value representation, and basic configuration loading.

## Tasks

1. **Create Configuration Value Types**
   - Implement `ConfigValue` enum supporting all TOML types (String, Integer, Float, Boolean, Array, Table)
   - Add serde deserialization support
   - Implement conversion from TOML values to JSON for liquid templates
   - Add type coercion methods

2. **Implement Configuration Structure**
   - Create `Configuration` struct with HashMap<String, ConfigValue>
   - Add nested table support for dot notation access
   - Implement environment variable substitution parsing (${VAR:-default})
   - Add validation methods for variable names and values

3. **Create Configuration Parser**
   - Implement TOML file parsing with comprehensive error handling
   - Add file size and depth validation (1MB max, 10 levels deep)
   - Support UTF-8 encoding validation
   - Handle missing file gracefully (return empty configuration)

4. **Add Error Handling**
   - Create `ConfigError` enum with specific variants
   - Add context preservation for parse errors with line numbers
   - Implement error chaining for detailed diagnostics
   - Add validation error reporting

## Acceptance Criteria

- [ ] ConfigValue enum supports all TOML types with serde integration
- [ ] Configuration struct handles nested tables and dot notation
- [ ] Environment variable substitution works correctly
- [ ] TOML parsing handles errors gracefully with detailed messages
- [ ] File validation enforces size and depth limits
- [ ] Unit tests cover all data structure functionality

## Files to Create

- `swissarmyhammer/src/config/value.rs` - ConfigValue enum and conversions
- `swissarmyhammer/src/config/configuration.rs` - Main Configuration struct
- `swissarmyhammer/src/config/parser.rs` - TOML parsing logic
- `swissarmyhammer/src/config/error.rs` - Configuration error types
- `swissarmyhammer/src/config/mod.rs` - Module declarations

## Next Steps

After completion, proceed to CONFIG_000238_template-integration for integrating configuration variables with the liquid template engine.

## Proposed Solution

Based on the existing code structure in `swissarmyhammer/src/sah_config/`, I can see that there's already a foundation with:
- `types.rs` - Basic ConfigValue enum and Configuration struct
- `loader.rs` - Configuration loading functionality
- `template_integration.rs` - Integration with liquid templates
- `validation.rs` - Configuration validation

However, the issue requires implementing core data structures in a new `config/` directory structure. I will:

1. **Create new config module structure** following the required file organization:
   - `swissarmyhammer/src/config/error.rs` - Comprehensive ConfigError enum with detailed error variants
   - `swissarmyhammer/src/config/value.rs` - Enhanced ConfigValue with environment variable substitution and type coercion
   - `swissarmyhammer/src/config/configuration.rs` - Configuration struct with nested table support and dot notation
   - `swissarmyhammer/src/config/parser.rs` - TOML parsing with size/depth validation and detailed error handling
   - `swissarmyhammer/src/config/mod.rs` - Module declarations and public API

2. **Implement comprehensive error handling** with:
   - Specific error variants for parse errors, validation errors, I/O errors
   - Context preservation with line numbers for parse errors
   - Error chaining for detailed diagnostics

3. **Add advanced configuration features**:
   - Environment variable substitution parsing (`${VAR:-default}`)
   - Nested table support with dot notation access
   - File size validation (1MB max) and depth validation (10 levels)
   - UTF-8 encoding validation

4. **Ensure full TOML type support** with:
   - All TOML types (String, Integer, Float, Boolean, Array, Table)
   - Serde integration for deserialization
   - Type coercion methods for flexible value handling
   - JSON conversion for liquid templates

This approach will create a robust, well-structured configuration system that meets all the acceptance criteria while following the existing codebase patterns.