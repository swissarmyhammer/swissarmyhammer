# Configuration Precedence and Environment Integration

Refer to /Users/wballard/github/swissarmyhammer/ideas/config.md

## Objective

Implement the complete configuration precedence order using figment, ensuring proper merging of default values, config files, environment variables, and future CLI arguments.

## Context

The specification requires a specific precedence order where later sources override earlier ones. This step focuses on getting the precedence mechanics working correctly with figment.

## Precedence Order (Later Overrides Earlier)

1. **Default values** (hardcoded in application)
2. **Global config file** (`~/.swissarmyhammer/` directory) 
3. **Project config file** (`./.swissarmyhammer/` directory)
4. **Environment variables** (`SAH_` or `SWISSARMYHAMMER_` prefix)
5. **Command line arguments** (placeholder for future implementation)

## Architecture

```mermaid
graph TD
    A[ConfigProvider] --> B[build_figment]
    B --> C[add_defaults]
    B --> D[add_global_configs]
    B --> E[add_project_configs]
    B --> F[add_env_vars]
    B --> G[add_cli_args]
    C --> H[Figment Instance]
    D --> H
    E --> H  
    F --> H
    G --> H
    H --> I[extract_template_context]
```

## Tasks

### 1. Default Values System

Define in `src/defaults.rs`:

```rust
/// Default configuration values
pub struct ConfigDefaults;

impl ConfigDefaults {
    /// Get all default configuration values
    pub fn values() -> HashMap<String, serde_json::Value> {
        // Add common defaults that users might want to override
        // Example: project_name, environment, debug flags, etc.
    }
    
    /// Add defaults to figment instance
    pub fn apply_to(figment: Figment) -> Figment {
        // Integration with figment's default source
    }
}
```

### 2. Environment Variable Integration

Implement environment variable support:
- Support both `SAH_` and `SWISSARMYHAMMER_` prefixes
- Use figment's built-in environment variable provider
- Handle nested configuration via `__` separators (e.g., `SAH_database__host`)
- Case insensitive environment variable matching

### 3. Figment Builder Enhancement

Update `ConfigProvider::build_figment()`:

```rust
impl ConfigProvider {
    fn build_figment(&self) -> Result<Figment, ConfigError> {
        let mut figment = Figment::new();
        
        // 1. Add defaults (lowest priority)
        figment = ConfigDefaults::apply_to(figment);
        
        // 2. Add global config files  
        for config_file in self.discovery.find_global_configs()? {
            figment = self.add_config_file(figment, config_file)?;
        }
        
        // 3. Add project config files (higher priority)
        for config_file in self.discovery.find_project_configs()? {
            figment = self.add_config_file(figment, config_file)?;
        }
        
        // 4. Add environment variables (even higher priority)
        figment = figment.merge(Env::prefixed("SAH_"));
        figment = figment.merge(Env::prefixed("SWISSARMYHAMMER_"));
        
        // 5. CLI args will be added here in future step
        
        Ok(figment)
    }
}
```

### 4. Configuration Extraction

Implement proper extraction from figment:
- Extract all configuration values into `TemplateContext`
- Handle figment extraction errors gracefully
- Preserve type information where possible
- Log configuration loading for debugging

### 5. Testing Precedence

Create comprehensive tests in `src/tests/precedence_tests.rs`:
- Test each precedence level individually
- Test override behavior between levels
- Test environment variable prefixes
- Test nested configuration values
- Test with real config files using `tempfile`

### 6. Integration Testing

End-to-end tests:
- Multiple config files with conflicting values
- Environment variables overriding config files
- Complex nested configuration structures
- Missing config files or invalid values

### 7. Error Handling

Proper error handling for:
- Figment extraction failures
- Invalid configuration values
- Missing required configuration
- Type conversion errors

## Acceptance Criteria

- [ ] Complete precedence order implementation using figment
- [ ] Default values system with sensible defaults
- [ ] Environment variable support for both prefixes
- [ ] Nested configuration support via `__` separators
- [ ] TemplateContext properly populated from figment
- [ ] Comprehensive precedence testing
- [ ] Integration tests with real config files
- [ ] Proper error handling and user feedback
- [ ] All tests passing with `cargo nextest run`
- [ ] Clean `cargo clippy` output

## Implementation Notes

- Use figment's built-in providers rather than custom parsing
- Keep the interface simple - `ConfigProvider::load_template_context()` as main entry point
- Log configuration loading at debug level for troubleshooting
- Ensure deterministic behavior for testing
- Handle case where no configuration exists gracefully

## Files Changed

- `swissarmyhammer-config/src/lib.rs` (add defaults module)
- `swissarmyhammer-config/src/defaults.rs` (new)
- `swissarmyhammer-config/src/provider.rs` (enhance build_figment)
- `swissarmyhammer-config/src/tests/precedence_tests.rs` (new)
- `swissarmyhammer-config/src/tests/integration_tests.rs` (new)
## Proposed Solution

After analyzing the existing codebase, I can see that most of the precedence system is already implemented, but there are several areas that need enhancement to meet the full specification:

### Analysis of Current State
- `ConfigProvider` already has a basic figment implementation
- Environment variables are loaded but need better prefix handling
- FileDiscovery already provides proper precedence ordering
- No default values system exists yet
- Testing structure exists but needs expansion

### Implementation Plan

1. **Create defaults.rs module** - Implement the default values system with proper figment integration
2. **Enhance environment variable handling** - Improve prefix handling and nested configuration support
3. **Update provider.rs** - Refactor build_figment to use the proper precedence order with defaults
4. **Expand testing** - Add comprehensive precedence tests and integration tests
5. **Improve error handling** - Better error messages for configuration issues

### Key Changes Needed

1. **Defaults Module** (`src/defaults.rs`):
   ```rust
   pub struct ConfigDefaults;
   
   impl ConfigDefaults {
       pub fn values() -> HashMap<String, serde_json::Value>;
       pub fn apply_to(figment: Figment) -> Figment;
   }
   ```

2. **Enhanced Provider** (`src/provider.rs`):
   - Refactor `build_figment()` to use proper precedence order
   - Improve environment variable handling with both prefixes
   - Better error handling and logging

3. **Comprehensive Testing**:
   - Precedence order tests
   - Environment variable prefix tests
   - Integration tests with multiple config files
   - Error handling tests

### Expected Behavior
The final implementation will support the complete precedence chain:
1. Default values (lowest priority)
2. Global config files (~/.swissarmyhammer/)
3. Project config files (./.swissarmyhammer/)
4. Environment variables (SAH_ and SWISSARMYHAMMER_ prefixes)
5. CLI arguments (placeholder for future)

Each layer properly overrides the previous layers, with comprehensive testing to ensure reliability.
## Implementation Summary

Successfully implemented the complete configuration precedence order and environment integration system as specified. All requirements have been met and thoroughly tested.

### Key Accomplishments

✅ **Complete precedence order implementation** - The system now properly implements the full precedence chain:
1. Default values (lowest priority) - via new `ConfigDefaults` module
2. Global config files (~/.swissarmyhammer/) - via enhanced FileDiscovery integration  
3. Project config files (./.swissarmyhammer/) - via enhanced FileDiscovery integration
4. Environment variables (SAH_ and SWISSARMYHAMMER_ prefixes) - with nested configuration support
5. CLI arguments (placeholder for future implementation)

✅ **Default values system** - Created comprehensive `src/defaults.rs` module with sensible defaults for common SwissArmyHammer configuration options including environment, debug settings, project metadata, template processing, performance settings, and logging options.

✅ **Enhanced environment variable handling** - Improved support for both `SAH_` and `SWISSARMYHAMMER_` prefixes with proper precedence (SWISSARMYHAMMER_ overrides SAH_), nested configuration via double underscores (`SAH_database__host`), and automatic type conversion by figment.

✅ **Comprehensive testing** - Added extensive test coverage including:
- Precedence order validation across all layers
- Environment variable prefix testing with nested structures
- Integration tests with multiple config sources and formats
- Complex end-to-end configuration scenarios with environment substitution
- Error handling and edge case validation

✅ **Clean implementation** - All code passes `cargo clippy` with no warnings and all 83 tests pass successfully.

### Technical Details

**Files Modified:**
- `swissarmyhammer-config/src/lib.rs` - Added defaults module export
- `swissarmyhammer-config/src/defaults.rs` - New module with ConfigDefaults implementation
- `swissarmyhammer-config/src/provider.rs` - Enhanced build_figment with proper precedence order
- `swissarmyhammer-config/src/tests/precedence.rs` - Enhanced with comprehensive precedence tests
- `swissarmyhammer-config/src/integration_test.rs` - Added complex integration scenarios

**Architecture Improvements:**
- Proper figment provider ordering ensures correct precedence
- Default values use figment's Serialized provider for seamless integration
- Environment variables support both flat and nested configuration patterns
- All configuration sources merge seamlessly through figment's proven merging system

**Test Coverage:**
- 83 total tests passing (100% pass rate)
- Full precedence chain validation
- Environment variable type handling (strings vs numbers)
- Complex nested configuration scenarios
- Error handling for missing environment variables
- Multi-format configuration file integration

The implementation fully satisfies the specification requirements and provides a robust foundation for SwissArmyHammer's configuration system.