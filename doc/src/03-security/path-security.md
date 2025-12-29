# Path Security

The `PathValidator` component provides comprehensive protection against path-based security vulnerabilities including directory traversal, symlink attacks, and unauthorized file access.

## Overview

Path security is critical for preventing attackers from accessing files outside intended boundaries. SwissArmyHammer implements multiple layers of path validation:

1. **Quick String Checks** - Fast rejection of obvious attacks
2. **Absolute Path Validation** - Platform-specific verification
3. **Path Canonicalization** - Resolves symlinks and relative components
4. **Component-Based Validation** - Authoritative security check
5. **Blocked Path Enforcement** - Explicit deny list
6. **Boundary Validation** - Allowed directory restrictions

## Architecture

### PathValidator Structure

```rust
pub struct PathValidator {
    /// Allowed root directories (empty = allow all except blocked)
    allowed_roots: Vec<PathBuf>,
    
    /// Blocked path prefixes (explicitly denied)
    blocked_paths: Vec<PathBuf>,
    
    /// Whether to perform strict canonicalization
    strict_canonicalization: bool,
    
    /// Size validator for path length validation
    size_validator: SizeValidator,
}
```

### Validation Flow

```
Input Path String
    ↓
1. Empty & Null Byte Check
    ↓
2. Length Validation
    ↓
3. Absolute Path Check (Platform-Specific)
    ↓
4. Quick Traversal Check (String-Based)
    ↓
5. Path Canonicalization (if strict mode)
    ↓
6. Component-Based Security Validation
    ↓
7. Blocked Path Check (with precedence)
    ↓
8. Boundary Validation (if configured)
    ↓
Valid Canonical Path
```

## Usage

### Basic Validation

```rust
use claude_agent::path_validator::PathValidator;

let validator = PathValidator::new();
let safe_path = validator.validate_absolute_path("/home/user/file.txt")?;
```

### With Allowed Roots

Restrict file access to specific directories:

```rust
let validator = PathValidator::with_allowed_roots(vec![
    PathBuf::from("/home/user/workspace"),
    PathBuf::from("/tmp"),
]);

// Allowed
let path = validator.validate_absolute_path("/home/user/workspace/file.txt")?;

// Denied - outside allowed roots
let result = validator.validate_absolute_path("/etc/passwd");
assert!(matches!(result, Err(PathValidationError::OutsideBoundaries(_))));
```

### With Blocked Paths

Explicitly block sensitive directories:

```rust
let validator = PathValidator::with_blocked_paths(vec![
    PathBuf::from("/etc"),
    PathBuf::from("/root"),
    PathBuf::from("/home/user/.ssh"),
]);

// Denied - blocked path
let result = validator.validate_absolute_path("/etc/passwd");
assert!(matches!(result, Err(PathValidationError::Blocked(_))));

// Denied - subdirectory of blocked path
let result = validator.validate_absolute_path("/etc/shadow");
assert!(matches!(result, Err(PathValidationError::Blocked(_))));
```

### Combined Allowed & Blocked

Block specific subdirectories within allowed paths:

```rust
let validator = PathValidator::with_allowed_and_blocked(
    vec![PathBuf::from("/home/user")],
    vec![PathBuf::from("/home/user/.ssh")],
);

// Allowed
let path = validator.validate_absolute_path("/home/user/documents/file.txt")?;

// Denied - blocked subdirectory (blocked takes precedence)
let result = validator.validate_absolute_path("/home/user/.ssh/id_rsa");
assert!(matches!(result, Err(PathValidationError::Blocked(_))));
```

### Custom Path Length

Limit maximum path length:

```rust
let validator = PathValidator::with_max_length(1024);

let long_path = "/".repeat(2000);
let result = validator.validate_absolute_path(&long_path);
assert!(matches!(result, Err(PathValidationError::PathTooLong(_, _))));
```

### Non-Strict Canonicalization

Allow validation of non-existent paths:

```rust
let validator = PathValidator::new()
    .with_strict_canonicalization(false);

// Succeeds even though file doesn't exist
let path = validator.validate_absolute_path("/tmp/future_file.txt")?;
```

## Validation Details

### 1. Empty & Null Byte Check

**Purpose:** Prevent empty paths and null byte injection

**Implementation:**
```rust
if path_str.is_empty() {
    return Err(PathValidationError::EmptyPath);
}

if path_str.contains('\0') {
    return Err(PathValidationError::NullBytesInPath);
}
```

**Attacks Prevented:**
- Null byte injection: `/etc/passwd\0.txt`
- Empty path confusion

### 2. Length Validation

**Purpose:** Prevent buffer overflow and DoS via extremely long paths

**Limits:**
- Default: 4096 characters
- Configurable via `with_max_length()`

**Error:** `PathValidationError::PathTooLong(actual, limit)`

### 3. Absolute Path Check

**Purpose:** Ensure path is absolute (not relative)

**Platform-Specific Rules:**

**Unix/Linux/macOS:**
- Must start with `/`
- Examples: `/home/user`, `/tmp/file.txt`

**Windows:**
- Must have drive letter: `C:\path\file.txt`
- Or UNC path: `\\server\share\file.txt`

**Error:** `PathValidationError::NotAbsolute(path)`

### 4. Quick Traversal Check

**Purpose:** Fast rejection of obvious path traversal attempts

**Patterns Detected:**
- `/../`
- `\\..\\`
- `/..$`
- `\\..$`
- `../` (at start)
- `..\\` (at start)

**Note:** This is an optimization. The authoritative check happens after canonicalization.

**Error:** `PathValidationError::PathTraversalAttempt(path)`

### 5. Path Canonicalization

**Purpose:** Resolve symlinks, `.` and `..` components to real path

**Behavior:**

**Strict Mode (default):**
- Resolves all symlinks
- Removes `.` and `..` components
- Fails if path doesn't exist
- Error: `PathValidationError::CanonicalizationFailed(path, reason)`

**Non-Strict Mode:**
- Skips canonicalization
- Allows non-existent paths
- Useful for path validation before file creation

**Example:**
```rust
// Strict mode
let validator = PathValidator::new();
let result = validator.validate_absolute_path("/tmp/nonexistent.txt");
// Fails with CanonicalizationFailed

// Non-strict mode
let validator = PathValidator::new().with_strict_canonicalization(false);
let path = validator.validate_absolute_path("/tmp/nonexistent.txt")?;
// Succeeds
```

### 6. Component-Based Security Validation

**Purpose:** Authoritative check for path traversal after normalization

**Implementation:**
```rust
for component in path.components() {
    match component {
        std::path::Component::ParentDir => {
            return Err(PathValidationError::PathTraversalAttempt(_));
        }
        std::path::Component::CurDir => {
            return Err(PathValidationError::RelativeComponent(_));
        }
        _ => {}
    }
}
```

**Why This Matters:**
- Catches encoded traversal attempts
- Works on canonicalized paths
- Platform-independent
- Robust against bypass techniques

### 7. Blocked Path Check

**Purpose:** Explicitly deny access to sensitive paths

**Behavior:**
- Checks if path starts with any blocked prefix
- Applies to all subdirectories
- **Takes precedence over allowed roots**
- Performed on canonical path (after symlink resolution)

**Example:**
```rust
// Block /etc
let validator = PathValidator::with_blocked_paths(vec![
    PathBuf::from("/etc")
]);

// All blocked
validator.validate_absolute_path("/etc/passwd")?;        // Error
validator.validate_absolute_path("/etc/shadow")?;        // Error
validator.validate_absolute_path("/etc/ssl/certs")?;     // Error
```

**Precedence:**
```rust
// Both allowed AND blocked
let validator = PathValidator::with_allowed_and_blocked(
    vec![PathBuf::from("/home/user")],
    vec![PathBuf::from("/home/user/.ssh")],
);

// .ssh is blocked even though /home/user is allowed
let result = validator.validate_absolute_path("/home/user/.ssh/id_rsa");
assert!(matches!(result, Err(PathValidationError::Blocked(_))));
```

### 8. Boundary Validation

**Purpose:** Restrict access to approved directories

**Behavior:**
- Only checks if `allowed_roots` is non-empty
- Path must start with one of the allowed roots
- Checked AFTER blocked path validation
- Performed on canonical path

**Example:**
```rust
let validator = PathValidator::with_allowed_roots(vec![
    PathBuf::from("/home/user/workspace"),
    PathBuf::from("/tmp"),
]);

// Allowed
validator.validate_absolute_path("/home/user/workspace/src/main.rs")?;
validator.validate_absolute_path("/tmp/output.txt")?;

// Denied
let result = validator.validate_absolute_path("/home/user/.bashrc");
assert!(matches!(result, Err(PathValidationError::OutsideBoundaries(_))));
```

## Error Handling

### PathValidationError Types

```rust
pub enum PathValidationError {
    /// Path is not absolute
    NotAbsolute(String),
    
    /// Path traversal attempt detected
    PathTraversalAttempt(String),
    
    /// Path contains relative components
    RelativeComponent(String),
    
    /// Path exceeds maximum length
    PathTooLong(usize, usize),
    
    /// Path canonicalization failed
    CanonicalizationFailed(String, String),
    
    /// Path outside allowed boundaries
    OutsideBoundaries(String),
    
    /// Path is explicitly blocked
    Blocked(String),
    
    /// Invalid path format
    InvalidFormat(String),
    
    /// Path contains null bytes
    NullBytesInPath,
    
    /// Empty path provided
    EmptyPath,
}
```

### Common Error Scenarios

#### File Not Found

```rust
let validator = PathValidator::new(); // Strict mode

let result = validator.validate_absolute_path("/tmp/nonexistent.txt");
match result {
    Err(PathValidationError::CanonicalizationFailed(path, err)) => {
        // err contains "No such file or directory"
    }
    _ => panic!("Unexpected result"),
}
```

**Solution:** Use non-strict mode for paths that will be created:
```rust
let validator = PathValidator::new().with_strict_canonicalization(false);
let path = validator.validate_absolute_path("/tmp/nonexistent.txt")?;
```

#### Permission Denied

```rust
// Attempting to access restricted system file
let result = validator.validate_absolute_path("/root/.ssh/id_rsa");
match result {
    Err(PathValidationError::CanonicalizationFailed(path, err)) => {
        // err contains "Permission denied"
    }
    _ => {}
}
```

#### Multiple Errors

When multiple validation issues exist, errors are detected in order:

1. Empty path → `EmptyPath`
2. Null bytes → `NullBytesInPath`
3. Path too long → `PathTooLong`
4. Not absolute → `NotAbsolute`
5. Quick traversal → `PathTraversalAttempt`
6. Canonicalization fails → `CanonicalizationFailed`
7. Component check → `PathTraversalAttempt` or `RelativeComponent`
8. Blocked path → `Blocked`
9. Outside boundaries → `OutsideBoundaries`

## Security Considerations

### Symlink Attacks

**Attack:** Create symlink to sensitive file, then access via symlink

```bash
ln -s /etc/passwd /tmp/innocent.txt
```

**Protection:**
- Canonicalization resolves symlinks to real path
- Validation performed on canonical path
- Blocked paths checked after resolution

```rust
// Even if /tmp is allowed, /etc/passwd is blocked
let validator = PathValidator::with_allowed_and_blocked(
    vec![PathBuf::from("/tmp")],
    vec![PathBuf::from("/etc")],
);

// Symlink resolves to /etc/passwd, which is blocked
let result = validator.validate_absolute_path("/tmp/innocent.txt");
assert!(matches!(result, Err(PathValidationError::Blocked(_))));
```

### Time-of-Check Time-of-Use (TOCTOU)

**Attack:** Change file between validation and use

**Mitigation:**
- Keep time window minimal
- Use file descriptors when possible
- Re-validate before sensitive operations
- Consider file locking

### Unicode/Encoding Attacks

**Attack:** Use Unicode or encoding tricks to bypass string checks

**Protection:**
- Component-based validation on `Path` type
- Rust's `Path` handles Unicode correctly
- Canonicalization normalizes representation

### Case Sensitivity

**Platform Differences:**
- Unix/Linux: Case-sensitive (`/etc` ≠ `/Etc`)
- macOS: Case-insensitive by default (`/etc` = `/Etc`)
- Windows: Case-insensitive (`C:\etc` = `C:\ETC`)

**Recommendation:**
- Always use lowercase for blocked paths on case-insensitive systems
- Test on target platform
- Document case sensitivity expectations

### Race Conditions

**Scenario:** File created/deleted during validation

**Protection:**
- Non-strict mode for creation scenarios
- Atomic operations when possible
- Check errors on actual file operations

## Performance Considerations

### Canonicalization Cost

**Impact:**
- Stat calls to resolve symlinks
- Disk I/O for path traversal
- System calls add latency

**Optimization:**
- Quick string check rejects most attacks early
- Cache validation results when appropriate
- Use non-strict mode for known paths

### Blocked Path Matching

**Complexity:** O(n × m) where n = blocked paths, m = path components

**Optimization:**
- Keep blocked path list small
- Use most specific paths
- Consider path trie for large lists

## Testing

### Test Path Validation

```rust
#[test]
fn test_path_traversal_blocked() {
    let validator = PathValidator::new();
    
    let malicious_paths = vec![
        "/tmp/../../../etc/passwd",
        "/home/user/../../root/.ssh",
    ];
    
    for path in malicious_paths {
        let result = validator.validate_absolute_path(path);
        assert!(result.is_err());
    }
}
```

### Test Allowed/Blocked Configuration

```rust
#[test]
fn test_blocked_precedence() {
    let validator = PathValidator::with_allowed_and_blocked(
        vec![PathBuf::from("/home/user")],
        vec![PathBuf::from("/home/user/.ssh")],
    );
    
    // Allowed
    assert!(validator
        .validate_absolute_path("/home/user/documents/file.txt")
        .is_ok());
    
    // Blocked (precedence)
    assert!(validator
        .validate_absolute_path("/home/user/.ssh/id_rsa")
        .is_err());
}
```

## Best Practices

### 1. Always Validate Before File Operations

```rust
// ❌ Bad
let path = PathBuf::from(user_input);
std::fs::read_to_string(&path)?;

// ✅ Good
let validator = PathValidator::with_allowed_roots(vec![workspace_dir]);
let safe_path = validator.validate_absolute_path(user_input)?;
std::fs::read_to_string(&safe_path)?;
```

### 2. Use Strict Mode for Existing Files

```rust
// For reading existing files
let validator = PathValidator::new(); // Strict by default
let path = validator.validate_absolute_path(input)?;
let content = std::fs::read_to_string(&path)?;
```

### 3. Use Non-Strict Mode for Creating Files

```rust
// For creating new files
let validator = PathValidator::new()
    .with_strict_canonicalization(false);
let path = validator.validate_absolute_path(input)?;
std::fs::write(&path, content)?;
```

### 4. Configure Allowed Roots for Applications

```rust
// Application-specific workspace
let workspace = std::env::current_dir()?;
let validator = PathValidator::with_allowed_roots(vec![workspace]);
```

### 5. Block Sensitive System Paths

```rust
let validator = PathValidator::with_blocked_paths(vec![
    PathBuf::from("/etc"),
    PathBuf::from("/root"),
    PathBuf::from("/var/log"),
    PathBuf::from(&format!("/home/{}/.ssh", username)),
]);
```

### 6. Handle Errors Appropriately

```rust
match validator.validate_absolute_path(input) {
    Ok(path) => process_file(path),
    Err(PathValidationError::NotAbsolute(_)) => {
        return Err("Path must be absolute");
    }
    Err(PathValidationError::Blocked(_)) => {
        return Err("Access to this path is not allowed");
    }
    Err(PathValidationError::OutsideBoundaries(_)) => {
        return Err("Path is outside allowed workspace");
    }
    Err(e) => {
        return Err(format!("Path validation failed: {}", e));
    }
}
```

## Integration Examples

### With File Read Tool

```rust
pub fn read_file(path: &str, validator: &PathValidator) -> Result<String> {
    let safe_path = validator.validate_absolute_path(path)?;
    let content = std::fs::read_to_string(&safe_path)?;
    Ok(content)
}
```

### With File Write Tool

```rust
pub fn write_file(
    path: &str,
    content: &str,
    validator: &PathValidator,
) -> Result<()> {
    let safe_path = validator.validate_absolute_path(path)?;
    
    // Optional: Check parent directory
    if let Some(parent) = safe_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    
    std::fs::write(&safe_path, content)?;
    Ok(())
}
```

### With MCP Server

```rust
impl McpServer {
    fn handle_file_read(&self, args: FileReadArgs) -> Result<FileReadResult> {
        // Validate path with server's validator
        let safe_path = self.path_validator.validate_absolute_path(&args.path)?;
        
        // Proceed with file operation
        let content = std::fs::read_to_string(&safe_path)?;
        
        Ok(FileReadResult { content })
    }
}
```

## See Also

- [Security Overview](overview.md) - Comprehensive security documentation
- [Permission System](permissions.md) - Tool authorization and consent
- [Content Security](content-security.md) - Content validation and sanitization
