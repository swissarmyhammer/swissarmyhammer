# Enhance file path validation beyond basic checks

## Locations
- `swissarmyhammer-tools/src/mcp/tools/files/shared_utils.rs:554` - Basic validation, return path as-is
- `swissarmyhammer-tools/src/mcp/tools/files/shared_utils.rs:608` - Simple existence and metadata check

## Current State
File path validation currently performs basic validation and simple checks. This should be enhanced with comprehensive path validation.

## Requirements
- Implement thorough path sanitization
- Check for path traversal attempts (../)
- Validate against symlink attacks
- Enforce workspace boundaries
- Handle platform-specific path issues
- Normalize paths consistently
- Add security tests for malicious paths
- Document path validation rules

## Security Considerations
- Path traversal vulnerabilities
- Symlink following attacks
- Race conditions (TOCTOU)
- Case sensitivity issues
- Unicode normalization attacks

## Impact
Insufficient path validation could allow access to files outside intended boundaries.