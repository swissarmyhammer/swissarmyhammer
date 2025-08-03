# Set Up File System Integration and Gitignore Management

Refer to ./specification/todo_tool.md

## Overview
Implement file system operations for todo files and ensure proper gitignore management to prevent accidental commits of ephemeral todo lists.

## File System Requirements
- Todo lists stored as `.yaml` files in `./swissarmyhammer/todo/`
- Directory auto-creation when first todo list is created
- Thread-safe file operations for concurrent access
- Proper error handling for file system failures

## Gitignore Management
According to specification:
- `./swissarmyhammer/todo/` should be added to `.gitignore`
- Todo lists are ephemeral and should never be committed
- Prevent accidental inclusion in version control

## Implementation Tasks
1. Create file system utilities for todo operations:
   - Directory creation with proper permissions
   - File path validation and sanitization
   - Atomic file write operations
   - Thread-safe file locking if needed

2. Implement gitignore management:
   - Check if `.gitignore` exists
   - Add `./swissarmyhammer/todo/` entry if missing
   - Handle various gitignore formats and edge cases
   - Preserve existing gitignore content

3. Add path security validation:
   - Prevent directory traversal attacks
   - Validate file names and extensions
   - Ensure files stay within designated directory
   - Sanitize user input for file names

4. Error handling for:
   - Permission denied errors
   - Disk space issues
   - Invalid file names
   - Concurrent access conflicts

## Directory Structure
```
./swissarmyhammer/
├── todo/
│   ├── session1.yaml
│   ├── feature-work.yaml
│   └── debugging.yaml
```

## Testing
- Directory creation tests
- File path validation tests
- Gitignore management tests
- Concurrent access safety tests
- Security validation tests
- Cross-platform compatibility tests

## Success Criteria
- Todo directory created automatically when needed
- Files properly isolated in designated directory
- Gitignore correctly configured to ignore todo files
- Path security prevents malicious file access
- Thread-safe operations prevent file corruption
- Comprehensive error handling and recovery

## Implementation Notes
- Use existing file system utilities from the codebase
- Follow security patterns from other modules
- Consider Windows/Unix path differences
- Use atomic operations to prevent corruption
- Add proper logging for file operations
- Test with various edge cases and error conditions