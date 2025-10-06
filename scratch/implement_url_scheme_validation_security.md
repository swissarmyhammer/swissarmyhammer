# Enhance URL security validation beyond HTTP/HTTPS

## Location
`swissarmyhammer-tools/src/mcp/tools/web_fetch/security.rs:22`

## Current State
```rust
/// URL scheme is not supported (only HTTP/HTTPS allowed)
```

## Description
The web fetch tool only supports HTTP/HTTPS URLs. While this is appropriate for security, the validation and error messages should be enhanced to be more informative.

## Requirements
- Ensure clear error messages for unsupported schemes
- Document why other schemes are not supported
- Add comprehensive tests for various URL schemes (file://, ftp://, etc.)
- Consider if any additional schemes should be safely supported
- Review for any bypass vulnerabilities

## Security Impact
Prevents SSRF and local file access attacks.