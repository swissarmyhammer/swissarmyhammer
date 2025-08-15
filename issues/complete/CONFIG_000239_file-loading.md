# CONFIG_000239: File Loading and Discovery - sah.toml Configuration

Refer to ./specification/config.md

## Goal

Implement sah.toml file discovery and loading from repository roots, with proper handling of missing files, file system errors, and security validation.

## Tasks

1. **Create Configuration Loader**
   - Implement file discovery from current working directory upward
   - Look for `sah.toml` at repository root (where .git exists)
   - Support absolute and relative path resolution
   - Handle missing files gracefully (return empty configuration)

2. **Add File System Operations**
   - Implement secure file reading with path validation
   - Prevent directory traversal attacks
   - Validate file permissions and accessibility
   - Add file modification time tracking for caching

3. **Environment Variable Processing**
   - Parse environment variable substitution syntax (${VAR:-default})
   - Support required variables (${VAR}) vs optional with defaults
   - Handle boolean conversion for environment variables
   - Validate environment variable names

4. **Configuration Caching**
   - Avoid re-reading unchanged files
   - Implement file modification time checking
   - Cache parsed configuration in memory
   - Handle cache invalidation properly

5. **Security Validation**
   - Implement file size limits (1MB maximum)
   - Validate TOML depth limits (10 levels maximum)
   - Sanitize paths to prevent traversal
   - Validate UTF-8 encoding

## Acceptance Criteria

- [ ] sah.toml discovered automatically from repository root
- [ ] Missing files handled gracefully without errors
- [ ] Environment variable substitution works correctly
- [ ] File caching avoids unnecessary re-reads
- [ ] Security limits enforced (size, depth, paths)
- [ ] Path traversal attacks prevented
- [ ] Unit tests cover all loading scenarios

## Files to Create

- `swissarmyhammer/src/config/loader.rs` - Configuration file loading
- `swissarmyhammer/src/config/discovery.rs` - File discovery logic
- `swissarmyhammer/src/config/env_vars.rs` - Environment variable processing

## Files to Modify

- `swissarmyhammer/src/config/mod.rs` - Add new modules

## Next Steps

After completion, proceed to CONFIG_000240_cli-integration for adding CLI commands to validate and inspect sah.toml configurations.
## Proposed Solution

After analyzing the existing codebase, I've identified that we have two overlapping configuration modules:
- `sah_config/` - Basic loader with minimal functionality  
- `toml_core/` - More complete parser with better validation

My approach will be to enhance the existing `sah_config` module to meet all the requirements while consolidating the functionality:

### 1. File Discovery Enhancement (`sah_config/loader.rs`)
- ✅ Already implemented: Repository root discovery (walks up to .git directory)
- ✅ Already implemented: File existence and size validation
- **Add**: Path traversal protection
- **Add**: File permission validation
- **Add**: Better error messages with file paths

### 2. Environment Variable Processing (`sah_config/env_vars.rs` - new file)
- **Create**: Environment variable substitution parser for `${VAR:-default}` syntax
- **Create**: Required vs optional variable handling  
- **Create**: Boolean conversion utilities
- **Create**: Environment variable name validation

### 3. Security Validation Enhancement (`sah_config/validation.rs`)
- **Enhance**: File size limits (1MB) - already partially implemented
- **Add**: TOML depth limits (10 levels) - borrow from toml_core
- **Add**: Path traversal attack prevention
- **Add**: UTF-8 encoding validation - borrow from toml_core
- **Add**: String value length limits (10KB each)
- **Add**: Array size limits (1000 elements)

### 4. Configuration Structure (`sah_config/types.rs`)
- **Enhance**: Add file modification time tracking for caching decisions
- **Add**: Cache invalidation logic
- **Add**: Security limits as configurable parameters

### 5. Module Integration (`sah_config/mod.rs`)
- **Update**: Export new environment variable functionality
- **Update**: Integrate enhanced validation
- **Add**: Convenience functions for common operations

### Implementation Strategy

1. **Consolidate best practices**: Take the UTF-8 validation and depth checking from `toml_core` and integrate into `sah_config`

2. **Add missing functionality**: Environment variable processing and comprehensive security validation

3. **Test-driven development**: Create comprehensive tests for all new functionality

4. **Maintain backward compatibility**: Ensure existing API continues to work

### Files to Modify/Create

1. **Modify**: `swissarmyhammer/src/sah_config/loader.rs` - Add security enhancements
2. **Create**: `swissarmyhammer/src/sah_config/env_vars.rs` - Environment variable processing  
3. **Modify**: `swissarmyhammer/src/sah_config/validation.rs` - Enhanced security validation
4. **Modify**: `swissarmyhammer/src/sah_config/types.rs` - Add caching support
5. **Modify**: `swissarmyhammer/src/sah_config/mod.rs` - Integrate new modules

This approach leverages existing working code while adding the missing pieces specified in the requirements.