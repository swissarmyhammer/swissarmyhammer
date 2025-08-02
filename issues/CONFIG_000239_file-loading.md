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