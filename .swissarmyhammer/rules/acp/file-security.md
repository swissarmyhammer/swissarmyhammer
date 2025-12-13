---
severity: error
tags:
- acp
- security
- filesystem
---

# ACP File System Security

File operations must validate paths and enforce security policies.

## Requirements

- Only accept absolute paths (no relative paths)
- Prevent path traversal attacks (.., symlinks)
- Enforce allowed/blocked path lists from configuration
- Validate file size limits before read operations
- Use atomic writes to prevent file corruption
- Check permissions before all file operations
- Log all file access attempts for audit
- Return meaningful errors for security violations

## Verification

Test with malicious paths including:
- `../../../etc/passwd`
- Symlinks outside allowed directories
- Files larger than size limit
- Paths in blocked list