# File System Restrictions

SwissArmyHammer implements comprehensive file system restrictions to protect against resource exhaustion, unauthorized access, and malicious file operations.

## Overview

File system security involves multiple layers of protection:

1. **Path Security** - Controls which files can be accessed (see [Path Security](path-security.md))
2. **File Size Limits** - Prevents resource exhaustion attacks
3. **File Type Validation** - Ensures safe file operations
4. **Permission Controls** - Requires user consent for file operations
5. **Operation Limits** - Rate limiting and concurrent operation controls

## File Size Limits

All components enforce consistent file size limits to prevent memory exhaustion and denial-of-service attacks.

### Standard Limit: 10 MB

The default maximum file size across SwissArmyHammer is **10 MB (10,485,760 bytes)**.

```rust
const MAX_FILE_SIZE: usize = 10 * 1024 * 1024; // 10 MB
```

### Limits by Component

#### claude-agent

**Location:** `claude-agent/src/constants/sizes.rs`

Supports three security levels:

| Security Level | Content Limit | Resource Limit | Use Case |
|---------------|---------------|----------------|----------|
| **Strict** | 1 MB | 5 MB | High security environments |
| **Moderate** (default) | 10 MB | 50 MB | Standard development |
| **Permissive** | 100 MB | 500 MB | Trusted environments only |

**Configuration:**
```rust
use claude_agent::config::SecurityLevel;

// Strict mode - 1 MB content limit
let config = Config::with_security_level(SecurityLevel::Strict);

// Moderate mode - 10 MB content limit (default)
let config = Config::with_security_level(SecurityLevel::Moderate);

// Permissive mode - 100 MB content limit
let config = Config::with_security_level(SecurityLevel::Permissive);
```

#### llama-agent

**Location:** `llama-agent/src/acp/config.rs`

**Default:** 10 MB (10,485,760 bytes)

**Configuration:**
```rust
use llama_agent::acp::FilesystemSettings;

let settings = FilesystemSettings {
    max_file_size: 10_485_760, // 10 MB
};
```

#### swissarmyhammer-tools

All file operation tools enforce 10 MB limits:

**File Write Tool:**
```rust
// swissarmyhammer-tools/src/mcp/tools/files/write/mod.rs
const MAX_FILE_SIZE: usize = 10 * 1024 * 1024;
```

**File Read Tool:**
```rust
// swissarmyhammer-common/src/file_loader.rs
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;
```

**Shell Execute Output:**
```rust
// swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs
const MAX_OUTPUT_SIZE: usize = 10 * 1024 * 1024;
```

**Web Fetch:**
```rust
// swissarmyhammer-tools/src/mcp/tools/web_fetch/fetch/mod.rs
const MAX_CONTENT_LENGTH_BYTES: u32 = 10_485_760;
```

### Why 10 MB?

The 10 MB limit provides a balance between:

**Usability:**
- Handles most source code files
- Supports reasonable documentation
- Allows moderate data files
- Accommodates compressed content

**Security:**
- Prevents memory exhaustion
- Mitigates zip bomb attacks
- Limits DoS potential
- Protects system resources

**Examples of File Sizes:**
```
Source code: 100 KB - 500 KB
Documentation: 50 KB - 200 KB
Small images: 100 KB - 2 MB
Large images: 2 MB - 8 MB
Small videos: 5 MB - 20 MB (exceeds limit)
Large datasets: 50 MB+ (exceeds limit)
```

### Checking File Sizes

Before reading a file:

```rust
use std::fs;

let metadata = fs::metadata(path)?;
let file_size = metadata.len();

const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

if file_size > MAX_FILE_SIZE {
    return Err(format!(
        "File size {} bytes exceeds maximum of {} bytes",
        file_size, MAX_FILE_SIZE
    ));
}

let content = fs::read_to_string(path)?;
```

Before writing a file:

```rust
const MAX_FILE_SIZE: usize = 10 * 1024 * 1024;

if content.len() > MAX_FILE_SIZE {
    return Err(format!(
        "Content size {} bytes exceeds maximum of {} bytes",
        content.len(), MAX_FILE_SIZE
    ));
}

fs::write(path, content)?;
```

### Error Handling

When a file exceeds size limits:

```rust
// Read operation
match fs::read_to_string(path) {
    Ok(content) if content.len() > MAX_FILE_SIZE => {
        Err("File too large to read")
    }
    Ok(content) => Ok(content),
    Err(e) => Err(e.to_string()),
}

// Write operation
if request.content.len() > MAX_FILE_SIZE {
    return Err(McpError::invalid_request(
        format!(
            "Content exceeds maximum size limit of {} bytes",
            MAX_FILE_SIZE
        ),
        None,
    ));
}
```

### Streaming for Large Files

For files larger than 10 MB, use streaming operations:

```rust
use std::io::{BufReader, BufWriter, Read, Write};

// Stream read
let file = File::open(path)?;
let mut reader = BufReader::new(file);
let mut buffer = [0; 8192];

loop {
    let bytes_read = reader.read(&mut buffer)?;
    if bytes_read == 0 {
        break;
    }
    process_chunk(&buffer[..bytes_read])?;
}

// Stream write
let file = File::create(path)?;
let mut writer = BufWriter::new(file);

for chunk in data_chunks {
    writer.write_all(chunk)?;
}
writer.flush()?;
```

## Path Length Limits

Maximum path length varies by platform but is typically limited to prevent buffer overflow attacks.

### Standard Limits

**Location:** `claude-agent/src/constants/sizes.rs`

```rust
pub const MAX_PATH_LENGTH_STANDARD: usize = 4096;  // 4 KB
pub const MAX_PATH_LENGTH_STRICT: usize = 1024;    // 1 KB
```

### Platform-Specific Limits

| Platform | System Limit | SwissArmyHammer Limit |
|----------|-------------|----------------------|
| Linux | 4096 bytes | 4096 bytes |
| macOS | 1024 bytes | 1024 bytes |
| Windows | 260 chars (legacy), 32,767 (extended) | 4096 bytes |

### Validation

```rust
use claude_agent::path_validator::PathValidator;

let validator = PathValidator::with_max_length(4096);

// Valid path
let path = validator.validate_absolute_path("/home/user/file.txt")?;

// Path too long
let long_path = "/".repeat(5000);
let result = validator.validate_absolute_path(&long_path);
assert!(matches!(result, 
    Err(PathValidationError::PathTooLong(actual, limit))));
```

## File Type Restrictions

### By Extension

Certain file types may be restricted based on security policies:

**Commonly Blocked:**
- Executables: `.exe`, `.dll`, `.so`, `.dylib`
- Scripts: `.bat`, `.cmd`, `.ps1`, `.sh` (if execution disabled)
- Archives: `.zip`, `.tar.gz` (size validation applies)
- System files: `.sys`, `.drv`

**Commonly Allowed:**
- Source code: `.rs`, `.js`, `.py`, `.go`, `.java`, `.c`, `.cpp`
- Documents: `.md`, `.txt`, `.json`, `.yaml`, `.toml`
- Web: `.html`, `.css`, `.svg`
- Data: `.csv`, `.xml`, `.sql`

### By Content Type

Content type validation uses magic number detection:

```rust
use claude_agent::content_security_validator::ContentSecurityValidator;

let validator = ContentSecurityValidator::moderate()?;

// Validate content type matches declaration
let base64_data = "iVBORw0KGgo..."; // PNG image
validator.validate_content_type_consistency(base64_data, "image/png")?;
```

**Detected Types:**
- Images: PNG, JPEG, GIF, WebP, BMP, TIFF
- Audio: MP3, WAV, FLAC, OGG
- Video: MP4, AVI, WebM, MOV
- Documents: PDF, ZIP, GZIP
- Executables: PE, ELF, Mach-O

### Malicious Pattern Detection

The content security validator detects potentially dangerous files:

**PE Executables (Windows):**
```rust
// Magic: "MZ" (0x4D 0x5A)
// Base64: "TV..."
if base64_data.starts_with("TV") {
    return Err("Executable content detected");
}
```

**ELF Executables (Linux):**
```rust
// Magic: 0x7F 0x45 0x4C 0x46
// Base64: "f0VM..."
if base64_data.starts_with("f0VM") {
    return Err("Executable content detected");
}
```

## Operation Limits

### Concurrent Operations

Limit concurrent file operations to prevent resource exhaustion:

```rust
use tokio::sync::Semaphore;
use std::sync::Arc;

let semaphore = Arc::new(Semaphore::new(10)); // Max 10 concurrent

async fn read_file_limited(
    path: &str,
    sem: Arc<Semaphore>,
) -> Result<String> {
    let _permit = sem.acquire().await?;
    tokio::fs::read_to_string(path).await
}
```

### Rate Limiting

Prevent abuse through rate limiting:

```rust
use std::time::{Duration, Instant};
use std::collections::HashMap;

struct RateLimiter {
    requests: HashMap<String, Vec<Instant>>,
    max_requests: usize,
    window: Duration,
}

impl RateLimiter {
    fn check_rate_limit(&mut self, key: &str) -> Result<(), String> {
        let now = Instant::now();
        let requests = self.requests.entry(key.to_string()).or_default();
        
        // Remove old requests outside window
        requests.retain(|&time| now.duration_since(time) < self.window);
        
        if requests.len() >= self.max_requests {
            return Err(format!(
                "Rate limit exceeded: {} requests per {:?}",
                self.max_requests, self.window
            ));
        }
        
        requests.push(now);
        Ok(())
    }
}

// Usage
let mut limiter = RateLimiter {
    requests: HashMap::new(),
    max_requests: 100,
    window: Duration::from_secs(60),
};

limiter.check_rate_limit("file_read")?;
```

## Permission Requirements

File operations require user permission based on risk level.

### Read Operations

**Risk Level:** Low

**Behavior:**
- First access requires user approval
- Option to "allow always" for read operations
- Permission persists across sessions

**Example:**
```rust
// First read
match permission_engine.evaluate_tool_call("fs_read", &args).await? {
    PolicyEvaluation::RequireUserConsent { options } => {
        // Options include "allow always"
        let response = prompt_user(options)?;
        if response.allow_always {
            permission_engine.store_permission("fs_read", &args, true).await?;
        }
    }
    _ => {}
}
```

### Write Operations

**Risk Level:** Medium

**Behavior:**
- Requires approval for each new directory
- Option to "allow always" for specific paths
- More restrictive than read operations

**Example:**
```rust
match permission_engine.evaluate_tool_call("fs_write", &args).await? {
    PolicyEvaluation::RequireUserConsent { options } => {
        let response = prompt_user(options)?;
        // User can approve directory for future writes
    }
    _ => {}
}
```

### Delete Operations

**Risk Level:** High

**Behavior:**
- Requires approval for each operation
- No "allow always" option
- Confirmation required

**Example:**
```rust
match permission_engine.evaluate_tool_call("fs_delete", &args).await? {
    PolicyEvaluation::RequireUserConsent { options } => {
        // No "allow always" option for deletes
        let response = prompt_user_with_confirmation(options)?;
    }
    _ => {}
}
```

See [Permission System](permissions.md) for detailed documentation.

## Directory Restrictions

### Allowed Roots

Restrict file operations to specific directories:

```rust
use claude_agent::path_validator::PathValidator;
use std::path::PathBuf;

let workspace = PathBuf::from("/home/user/workspace");
let temp = PathBuf::from("/tmp");

let validator = PathValidator::with_allowed_roots(vec![
    workspace,
    temp,
]);

// Allowed
let path = validator.validate_absolute_path("/home/user/workspace/file.txt")?;

// Denied
let result = validator.validate_absolute_path("/etc/passwd");
assert!(matches!(result, 
    Err(PathValidationError::OutsideBoundaries(_))));
```

### Blocked Paths

Explicitly block sensitive directories:

```rust
let validator = PathValidator::with_blocked_paths(vec![
    PathBuf::from("/etc"),
    PathBuf::from("/root"),
    PathBuf::from("/var/log"),
    PathBuf::from("/home/user/.ssh"),
    PathBuf::from("/home/user/.gnupg"),
]);

// All attempts to access blocked paths fail
let result = validator.validate_absolute_path("/etc/passwd");
assert!(matches!(result, Err(PathValidationError::Blocked(_))));
```

### Recommended Blocked Paths

**System Directories:**
```
/etc
/root
/boot
/sys
/proc
/dev
```

**User Sensitive:**
```
~/.ssh
~/.gnupg
~/.aws
~/.kube
~/.docker
```

**Application Data:**
```
~/.config/credentials
~/.local/share/keyrings
~/Library/Keychains (macOS)
```

**Windows:**
```
C:\Windows\System32
C:\Windows\System32\config
C:\Users\{user}\AppData\Local\Packages
```

## Disk Space Management

### Available Space Check

Check available disk space before large operations:

```rust
use std::fs;

fn check_disk_space(path: &Path, required: u64) -> Result<(), String> {
    // This is platform-specific
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let metadata = fs::metadata(path)?;
        // Note: blocks() gives block count, blksize() gives block size
        // This is a simplified check
    }
    
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // Windows-specific disk space check
    }
    
    Ok(())
}
```

Better approach using external crate:

```rust
use sysinfo::{System, Disks};

fn check_disk_space(path: &Path, required: u64) -> Result<(), String> {
    let disks = Disks::new_with_refreshed_list();
    
    for disk in disks.list() {
        if path.starts_with(disk.mount_point()) {
            let available = disk.available_space();
            if available < required {
                return Err(format!(
                    "Insufficient disk space: {} bytes required, {} bytes available",
                    required, available
                ));
            }
            return Ok(());
        }
    }
    
    Err("Could not determine disk space".to_string())
}
```

### Cleanup Strategies

Automatic cleanup of temporary files:

```rust
use std::time::{SystemTime, Duration};

fn cleanup_old_files(dir: &Path, max_age: Duration) -> Result<usize> {
    let mut removed = 0;
    let now = SystemTime::now();
    
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        
        if let Ok(modified) = metadata.modified() {
            if let Ok(age) = now.duration_since(modified) {
                if age > max_age {
                    fs::remove_file(entry.path())?;
                    removed += 1;
                }
            }
        }
    }
    
    Ok(removed)
}

// Cleanup files older than 7 days
cleanup_old_files(
    Path::new("/tmp/swissarmyhammer"),
    Duration::from_secs(7 * 24 * 60 * 60)
)?;
```

## Testing File System Restrictions

### Test Size Limits

```rust
#[test]
fn test_file_size_limit() {
    let large_content = "x".repeat(11 * 1024 * 1024); // 11 MB
    
    let result = write_file("test.txt", &large_content);
    assert!(matches!(result, Err(_)));
}
```

### Test Path Restrictions

```rust
#[test]
fn test_path_restrictions() {
    let validator = PathValidator::with_blocked_paths(vec![
        PathBuf::from("/etc"),
    ]);
    
    let result = validator.validate_absolute_path("/etc/passwd");
    assert!(matches!(result, Err(PathValidationError::Blocked(_))));
}
```

### Test Permission Requirements

```rust
#[tokio::test]
async fn test_write_permission_required() {
    let engine = PermissionPolicyEngine::new(storage);
    
    let eval = engine.evaluate_tool_call("fs_write", &args).await?;
    assert!(matches!(eval, PolicyEvaluation::RequireUserConsent { .. }));
}
```

## Security Best Practices

### 1. Always Validate Paths

```rust
// ❌ Bad - no validation
let content = fs::read_to_string(user_input)?;

// ✅ Good - validate first
let safe_path = validator.validate_absolute_path(user_input)?;
let content = fs::read_to_string(safe_path)?;
```

### 2. Check File Sizes Before Operations

```rust
// ❌ Bad - read without checking
let content = fs::read_to_string(path)?;

// ✅ Good - check size first
let size = fs::metadata(path)?.len();
if size > MAX_FILE_SIZE {
    return Err("File too large");
}
let content = fs::read_to_string(path)?;
```

### 3. Use Streaming for Large Operations

```rust
// ❌ Bad - load entire file into memory
let content = fs::read(large_file)?;
process_all(content)?;

// ✅ Good - stream processing
let file = File::open(large_file)?;
let reader = BufReader::new(file);
for line in reader.lines() {
    process_line(line?)?;
}
```

### 4. Clean Up Temporary Files

```rust
// ✅ Use RAII for automatic cleanup
use tempfile::NamedTempFile;

let temp = NamedTempFile::new()?;
temp.write_all(data)?;
process_file(temp.path())?;
// Automatically deleted when temp goes out of scope
```

### 5. Validate File Types

```rust
// ❌ Bad - trust user's MIME type
save_file(filename, data, user_mime_type)?;

// ✅ Good - validate content type
let detected_type = detect_content_type(data)?;
if detected_type != user_mime_type {
    return Err("Content type mismatch");
}
save_file(filename, data, detected_type)?;
```

## See Also

- [Path Security](path-security.md) - Path validation and traversal prevention
- [Content Security](content-security.md) - Content validation and SSRF protection
- [Permission System](permissions.md) - Authorization and consent flows
- [Security Overview](overview.md) - Comprehensive security documentation
