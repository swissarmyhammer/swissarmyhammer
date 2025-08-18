# File Tools Troubleshooting

This guide covers common issues, error messages, and troubleshooting steps for the SwissArmyHammer file tools.

## Common Error Categories

### Path Validation Errors

#### Relative Path Error
**Error Message:** `File path must be absolute, not relative`

**Cause:** The provided path is relative instead of absolute.

**Examples:**
```bash
# ❌ Incorrect - relative path
sah file read "src/main.rs"
sah file read "./config.toml"
sah file read "../parent/file.txt"

# ✅ Correct - absolute path
sah file read "/workspace/src/main.rs"
sah file read "/home/user/project/config.toml"
```

**Solution:** Always use absolute paths starting with `/` (Unix) or drive letters on Windows.

#### Empty Path Error
**Error Message:** `File path cannot be empty` or `absolute_path cannot be empty`

**Cause:** The path parameter is empty, contains only whitespace, or is not provided.

**Examples:**
```bash
# ❌ Incorrect
sah file read ""
sah file read "   "

# ✅ Correct
sah file read "/workspace/file.txt"
```

**Solution:** Provide a valid non-empty absolute path.

#### Path Traversal Error  
**Error Message:** `Path contains blocked pattern '../'` or `Path contains dangerous traversal sequences`

**Cause:** The path contains sequences that could be used for directory traversal attacks.

**Examples:**
```bash
# ❌ Incorrect - contains blocked patterns
sah file read "/workspace/../../../etc/passwd"
sah file read "/workspace/./file.txt"
sah file write "/workspace\\..\\dangerous" "content"

# ✅ Correct
sah file read "/workspace/config/settings.toml"
```

**Solution:** Remove dangerous path sequences and use clean absolute paths.

### Workspace Boundary Errors

#### Outside Workspace Error
**Error Message:** `Path is outside workspace boundaries`

**Cause:** The file path is outside the configured workspace directory.

**Diagnosis:**
```bash
# Check current workspace boundaries
echo $SAH_WORKSPACE_ROOT
pwd
```

**Examples:**
```bash
# If workspace is /home/user/project:
# ❌ Incorrect - outside workspace
sah file read "/etc/passwd"
sah file read "/tmp/file.txt"

# ✅ Correct - within workspace
sah file read "/home/user/project/src/main.rs"
```

**Solutions:**
1. Move files into the workspace directory
2. Configure workspace boundaries to include the target directory
3. Use relative paths from within the workspace (after converting to absolute)

### Permission Errors

#### Permission Denied
**Error Message:** `Permission denied accessing: /path/to/file`

**Cause:** Insufficient file system permissions for the operation.

**Diagnosis:**
```bash
# Check file permissions
ls -la /path/to/file

# Check directory permissions
ls -ld /path/to/directory

# Check current user permissions
id
groups
```

**Solutions:**
```bash
# Make file readable
chmod +r /path/to/file

# Make file writable  
chmod +w /path/to/file

# Make directory accessible
chmod +x /path/to/directory

# Change ownership if needed (as root)
sudo chown $USER:$USER /path/to/file
```

#### Read-Only File Error
**Error Message:** `File is read-only: /path/to/file` or `File is read-only and cannot be edited`

**Cause:** Attempting to write or edit a file marked as read-only.

**Diagnosis:**
```bash
# Check file attributes
ls -la /path/to/file
lsattr /path/to/file  # Linux extended attributes
```

**Solutions:**
```bash
# Remove read-only attribute
chmod +w /path/to/file

# Remove immutable attribute (Linux)
sudo chattr -i /path/to/file
```

### File System Errors

#### File Not Found
**Error Message:** `File not found: /path/to/file`

**Cause:** The specified file does not exist.

**Diagnosis:**
```bash
# Check if file exists
ls -la /path/to/file

# Check if parent directory exists
ls -ld /path/to/

# Find similar files
find /path/to/ -name "*filename*"
```

**Solutions:**
1. Verify the correct file path
2. Create the file if it should exist
3. Check for typos in the filename
4. Use glob patterns to find similar files

#### Parent Directory Missing
**Error Message:** `Parent directory does not exist: /path/to` or `Failed to create directory`

**Cause:** The parent directory for a write operation doesn't exist and cannot be created.

**Diagnosis:**
```bash
# Check parent directory
ls -ld /path/to/

# Check permissions on parent of parent
ls -ld /path/
```

**Solutions:**
```bash
# Create parent directories manually
mkdir -p /path/to/directory

# Fix permissions on parent directories
chmod +wx /path/
```

### Parameter Validation Errors

#### Invalid Offset/Limit Values
**Error Messages:**
- `offset must be less than 1,000,000 lines`
- `limit must be greater than 0`
- `limit must be less than or equal to 100,000 lines`

**Cause:** Read tool offset or limit parameters are outside valid ranges.

**Examples:**
```bash
# ❌ Incorrect
sah file read /workspace/file.txt --offset -1        # Negative offset
sah file read /workspace/file.txt --offset 1000001   # Too large
sah file read /workspace/file.txt --limit 0          # Zero limit
sah file read /workspace/file.txt --limit 100001     # Too large

# ✅ Correct
sah file read /workspace/file.txt --offset 1 --limit 100
```

**Solution:** Use valid parameter ranges:
- Offset: 1 to 1,000,000
- Limit: 1 to 100,000

#### Content Size Limit Error
**Error Message:** `content exceeds maximum size limit of 10MB`

**Cause:** Write tool content parameter exceeds the 10MB safety limit.

**Diagnosis:**
```bash
# Check content size
echo -n "content" | wc -c
ls -lh large_file.txt
```

**Solutions:**
1. Split large content into smaller chunks
2. Use streaming approaches for large data
3. Consider if the content is appropriate for the tool's use case

### String Replacement Errors

#### String Not Found
**Error Message:** `String 'search_text' not found in file`

**Cause:** The old_string parameter doesn't exist in the target file.

**Diagnosis:**
```bash
# Check if string exists in file
grep -n "search_text" /path/to/file

# Check for case sensitivity issues
grep -ni "search_text" /path/to/file

# Check for whitespace issues
cat -A /path/to/file | grep "search_text"
```

**Solutions:**
1. Verify the exact string to replace
2. Check for case sensitivity
3. Include surrounding whitespace if needed
4. Use grep to find the exact text

#### Multiple Matches Error  
**Error Message:** `String 'text' appears 3 times in file. Use replace_all=true for multiple replacements`

**Cause:** Attempting single replacement when multiple matches exist.

**Examples:**
```bash
# File contains multiple instances of "config"
# ❌ This will fail
sah file edit /workspace/file.txt "config" "configuration"

# ✅ This will work
sah file edit /workspace/file.txt "config" "configuration" --replace-all
```

**Solutions:**
1. Use `--replace-all` flag for multiple replacements
2. Use more specific search strings for single replacements
3. Examine file content to understand match locations

#### Identical Strings Error
**Error Message:** `old_string and new_string cannot be identical`

**Cause:** The replacement string is identical to the search string.

**Solution:** Ensure old_string and new_string are different.

### Security-Related Errors

#### Blocked Pattern Error
**Error Message:** `Path contains blocked pattern '..': /path`

**Cause:** Path contains patterns blocked by security validation.

**Common Blocked Patterns:**
- `..` (parent directory)
- `./` (current directory)
- `\0` (null bytes)
- Control characters

**Solution:** Use clean paths without dangerous sequences.

#### Null Byte Error
**Error Message:** `Path contains invalid control characters`

**Cause:** Path contains null bytes or other control characters.

**Diagnosis:**
```bash
# Check for hidden characters
cat -A filename
od -c filename
```

**Solution:** Remove control characters from path strings.

### Tool-Specific Errors

#### Glob Pattern Errors
**Error Message:** `Invalid glob pattern` or `Pattern cannot be empty`

**Cause:** Malformed or empty glob patterns.

**Examples:**
```bash
# ❌ Incorrect
sah file glob ""           # Empty pattern
sah file glob "[invalid"   # Unclosed bracket

# ✅ Correct
sah file glob "**/*.rs"
sah file glob "src/**/*.{js,ts}"
```

**Solution:** Use valid glob pattern syntax.

#### Grep Pattern Errors
**Error Message:** `Invalid regular expression` or `Regex parse error`

**Cause:** Malformed regular expression patterns.

**Diagnosis:**
```bash
# Test regex pattern
echo "test string" | grep -E "your_pattern"
```

**Solutions:**
1. Escape special regex characters
2. Use valid regex syntax
3. Test patterns with simple tools first

#### Result Limit Exceeded
**Error Message:** `Result limit exceeded (10,000 files)`

**Cause:** Glob operation found too many matching files.

**Solutions:**
1. Use more specific patterns
2. Search in subdirectories instead of root
3. Use grep with file type filters instead of glob

## Diagnostic Commands

### File System Diagnostics
```bash
# Check file existence and permissions
ls -la /path/to/file

# Check directory structure
tree /path/to/directory

# Check disk space
df -h /workspace

# Check file system permissions
getfacl /path/to/file  # Linux extended permissions
```

### Path Resolution
```bash
# Resolve absolute path
realpath relative/path

# Check current working directory
pwd

# Check symbolic links
ls -l /path/to/symlink
readlink /path/to/symlink
```

### Content Analysis
```bash
# Check file encoding
file /path/to/file
file -i /path/to/file

# Check for binary content
hexdump -C /path/to/file | head

# Check line endings
cat -e /path/to/file | head
```

### Security Validation
```bash
# Check for dangerous patterns
grep -E '\.\.|/\.|\\' filename

# Check for control characters
cat -A filename | head

# Validate workspace boundaries
find /workspace -name filename
```

## Performance Troubleshooting

### Slow Read Operations
**Symptoms:** File reading takes excessive time

**Causes & Solutions:**
1. **Large Files:** Use offset/limit parameters
```bash
# Instead of
sah file read /workspace/huge.log

# Use
sah file read /workspace/huge.log --offset 10000 --limit 100
```

2. **Network File Systems:** Access files on local storage when possible
3. **Binary Files:** May be slower due to base64 encoding

### Slow Glob Operations
**Symptoms:** Pattern matching takes too long

**Solutions:**
1. Use more specific patterns
```bash
# Instead of
sah file glob "**/*"

# Use
sah file glob "src/**/*.rs"
```

2. Search in specific directories
```bash
sah file glob "*.txt" --path /workspace/docs
```

3. Use file type filters in grep instead
```bash
sah file grep "pattern" --type rust  # Instead of glob + grep
```

### Memory Usage Issues
**Symptoms:** High memory consumption or out-of-memory errors

**Solutions:**
1. Use streaming operations with limits
2. Process files individually instead of batch operations
3. Use count mode in grep when possible

## Best Practices for Error Prevention

### Path Handling
1. Always use absolute paths
2. Validate paths before operations
3. Use proper path separators for the platform
4. Avoid special characters in filenames

### Security
1. Configure appropriate workspace boundaries
2. Validate all user-provided paths
3. Monitor audit logs for security violations
4. Use principle of least privilege for file permissions

### Performance
1. Use appropriate limits for large file operations
2. Choose the right tool for the job (grep vs glob)
3. Monitor resource usage during operations
4. Implement proper error handling in scripts

### Development
1. Test file operations with small files first
2. Implement proper error handling in automation
3. Use structured logging for debugging
4. Validate inputs before calling tools

## Getting Help

### Log Analysis
Check SwissArmyHammer logs for detailed error information:
```bash
# View recent logs
tail -f ~/.swissarmyhammer/logs/sah.log

# Search for specific errors
grep "ERROR" ~/.swissarmyhammer/logs/sah.log
```

### Debugging Mode
Enable verbose logging for detailed information:
```bash
SAH_LOG_LEVEL=debug sah file read /workspace/file.txt
```

### Community Support
- GitHub Issues: Report bugs and request features
- Documentation: Check online documentation for updates
- Examples: Review example workflows and usage patterns

This troubleshooting guide should help resolve most common issues with the file tools. For persistent problems, check the logs, verify system configuration, and ensure proper security setup.