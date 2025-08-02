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