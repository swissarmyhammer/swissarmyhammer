# Security Model

SwissArmyHammer Tools implements multiple layers of security to protect against common vulnerabilities while maintaining usability for development workflows.

## Security Principles

### Defense in Depth

Multiple security layers protect against threats:

1. **Input Validation**: Schema validation and type checking
2. **Path Sanitization**: Prevention of directory traversal
3. **Resource Limits**: Protection against resource exhaustion
4. **Error Handling**: No information leakage in errors
5. **Least Privilege**: Minimal permissions for operations

### Secure by Default

Security features are enabled automatically:

- All file paths are validated
- Resource limits apply to all operations
- Schema validation is mandatory
- Error messages sanitized

### Transparency

Security measures are documented and auditable:

- All validations are explicit in code
- Security decisions logged
- No security through obscurity
- Open source for community review

## Input Validation

### Schema Validation

All tool arguments are validated against JSON Schema:

```rust
pub fn schema(&self) -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "File path",
                "minLength": 1,
                "maxLength": 4096
            }
        },
        "required": ["path"]
    })
}
```

Validation ensures:
- Required parameters are present
- Types match schema definition
- String lengths within bounds
- Numeric values in valid ranges
- Array sizes limited

### Type Safety

Rust's type system provides compile-time guarantees:

```rust
#[derive(Deserialize)]
struct ReadFileRequest {
    #[serde(deserialize_with = "validate_path")]
    path: PathBuf,
    
    #[serde(default)]
    offset: Option<u64>,
    
    #[serde(default)]
    limit: Option<u64>,
}
```

Custom deserializers enforce additional constraints:

```rust
fn validate_path<'de, D>(deserializer: D) -> Result<PathBuf, D::Error>
where
    D: Deserializer<'de>,
{
    let path_str = String::deserialize(deserializer)?;
    
    // Reject null bytes
    if path_str.contains('\0') {
        return Err(serde::de::Error::custom("Path contains null byte"));
    }
    
    // Reject overly long paths
    if path_str.len() > 4096 {
        return Err(serde::de::Error::custom("Path too long"));
    }
    
    Ok(PathBuf::from(path_str))
}
```

## Path Security

### Directory Traversal Prevention

All file paths are canonicalized and validated:

```rust
fn validate_file_path(path: &Path, base_dir: Option<&Path>) -> Result<PathBuf> {
    // Canonicalize to resolve .. and symlinks
    let canonical = path.canonicalize()
        .map_err(|e| SecurityError::InvalidPath(e.to_string()))?;
    
    // If base directory specified, ensure path is within it
    if let Some(base) = base_dir {
        let canonical_base = base.canonicalize()?;
        if !canonical.starts_with(&canonical_base) {
            return Err(SecurityError::PathTraversal {
                attempted: path.display().to_string(),
                base: base.display().to_string(),
            });
        }
    }
    
    Ok(canonical)
}
```

### Symlink Handling

Symlinks are resolved during canonicalization:

- Symlinks to absolute paths are allowed
- Symlinks that escape base directory are rejected
- Circular symlinks are detected and rejected

### Forbidden Paths

Certain paths are never accessible:

```rust
const FORBIDDEN_PATHS: &[&str] = &[
    "/etc/shadow",
    "/etc/passwd",
    "/proc/",
    "/sys/",
    "~/.ssh/id_rsa",
    "~/.aws/credentials",
];

fn is_forbidden_path(path: &Path) -> bool {
    let path_str = path.to_string_lossy();
    FORBIDDEN_PATHS.iter().any(|forbidden| {
        path_str.contains(forbidden)
    })
}
```

## Resource Limits

### File Size Limits

File operations enforce size limits:

```rust
const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024; // 100 MB

async fn read_file(path: &Path) -> Result<String> {
    let metadata = tokio::fs::metadata(path).await?;
    
    if metadata.len() > MAX_FILE_SIZE {
        return Err(Error::FileTooLarge {
            size: metadata.len(),
            limit: MAX_FILE_SIZE,
        });
    }
    
    tokio::fs::read_to_string(path).await
        .map_err(Into::into)
}
```

### Search Result Limits

Search operations limit result counts:

```rust
const MAX_SEARCH_RESULTS: usize = 1000;

pub async fn search_files(query: &str) -> Result<Vec<SearchResult>> {
    let results = perform_search(query).await?;
    
    if results.len() > MAX_SEARCH_RESULTS {
        results.truncate(MAX_SEARCH_RESULTS);
        tracing::warn!(
            "Search returned {} results, truncated to {}",
            results.len(),
            MAX_SEARCH_RESULTS
        );
    }
    
    Ok(results)
}
```

### Command Timeout

Shell commands have mandatory timeouts:

```rust
const DEFAULT_TIMEOUT_MS: u64 = 120_000; // 2 minutes
const MAX_TIMEOUT_MS: u64 = 600_000;     // 10 minutes

pub async fn execute_command(
    command: &str,
    timeout: Option<u64>,
) -> Result<CommandOutput> {
    let timeout_ms = timeout
        .unwrap_or(DEFAULT_TIMEOUT_MS)
        .min(MAX_TIMEOUT_MS);
    
    let timeout_duration = Duration::from_millis(timeout_ms);
    
    tokio::time::timeout(
        timeout_duration,
        run_command(command)
    ).await
    .map_err(|_| Error::CommandTimeout {
        command: command.to_string(),
        timeout_ms,
    })?
}
```

### Memory Limits

Large data structures are streamed:

```rust
// Instead of loading entire file
let contents = fs::read_to_string(path)?; // Bad: loads all into memory

// Stream file in chunks
let file = File::open(path)?;
let reader = BufReader::new(file);
for line in reader.lines() {
    process_line(line?)?;
}
```

## Process Isolation

### Sandboxing

Shell commands are executed with restricted permissions where supported by the platform.

### Environment Sanitization

Shell commands receive sanitized environment:

```rust
fn sanitize_environment() -> HashMap<String, String> {
    let mut env = HashMap::new();
    
    // Only pass safe environment variables
    for (key, value) in std::env::vars() {
        if is_safe_env_var(&key) {
            env.insert(key, value);
        }
    }
    
    env
}

fn is_safe_env_var(key: &str) -> bool {
    const SAFE_VARS: &[&str] = &[
        "PATH",
        "HOME",
        "USER",
        "LANG",
        "TERM",
    ];
    
    SAFE_VARS.contains(&key)
}
```

## Credential Protection

### No Credential Storage

SwissArmyHammer does not store credentials:

- API keys provided via environment variables
- Authentication tokens managed by external tools
- No password storage or management

### Environment Variable Handling

Sensitive environment variables are handled carefully:

```rust
// Never log sensitive variables
fn log_environment() {
    for (key, value) in std::env::vars() {
        if is_sensitive_var(&key) {
            tracing::debug!("ENV: {}=[REDACTED]", key);
        } else {
            tracing::debug!("ENV: {}={}", key, value);
        }
    }
}

fn is_sensitive_var(key: &str) -> bool {
    key.contains("PASSWORD") ||
    key.contains("SECRET") ||
    key.contains("TOKEN") ||
    key.contains("KEY")
}
```

### Credential Detection

Tools warn when attempting to commit sensitive files:

```rust
const SENSITIVE_FILES: &[&str] = &[
    ".env",
    "credentials.json",
    "id_rsa",
    "id_ed25519",
    ".aws/credentials",
];

fn check_sensitive_files(files: &[PathBuf]) -> Vec<String> {
    let mut warnings = Vec::new();
    
    for file in files {
        let filename = file.file_name()
            .unwrap_or_default()
            .to_string_lossy();
            
        if SENSITIVE_FILES.iter().any(|s| filename.contains(s)) {
            warnings.push(format!(
                "⚠️  Attempting to commit sensitive file: {}",
                file.display()
            ));
        }
    }
    
    warnings
}
```

## Error Handling

### Information Disclosure Prevention

Error messages avoid leaking sensitive information:

```rust
// Bad: leaks internal paths
return Err(Error::FileNotFound {
    path: format!("/home/user/.config/secret.key"),
});

// Good: sanitized error
return Err(Error::FileNotFound {
    path: "configuration file",
});
```

### Error Sanitization

Errors are sanitized before returning to clients:

```rust
pub fn sanitize_error(error: &Error) -> String {
    match error {
        Error::Io(e) => {
            // Don't leak file paths from IO errors
            format!("IO error: {}", e.kind())
        }
        Error::InvalidPath { .. } => {
            // Don't reveal path validation logic
            "Invalid file path".to_string()
        }
        Error::CommandFailed { command, .. } => {
            // Don't leak command details
            "Command execution failed".to_string()
        }
        _ => error.to_string(),
    }
}
```

### Stack Trace Protection

Stack traces are logged but not returned to clients:

```rust
pub async fn handle_error(error: Error) -> McpError {
    // Log full error with stack trace for debugging
    tracing::error!("Internal error: {:?}", error);
    
    // Return sanitized error to client
    McpError::internal_error(
        sanitize_error(&error),
        None // No additional details
    )
}
```

## MCP Protocol Security

### Request Validation

All MCP requests are validated:

```rust
async fn call_tool(
    &self,
    request: CallToolRequestParam,
    context: RequestContext<RoleServer>,
) -> Result<CallToolResult, McpError> {
    // Validate tool name
    if !self.is_valid_tool_name(&request.name) {
        return Err(McpError::invalid_request(
            "Invalid tool name",
            None
        ));
    }
    
    // Validate arguments
    let tool = self.get_tool(&request.name)
        .ok_or_else(|| McpError::invalid_request(
            "Unknown tool",
            None
        ))?;
    
    // Schema validation happens in tool.execute()
    tool.execute(request.arguments.unwrap_or_default(), &self.tool_context).await
}
```

### Transport Security

#### Stdio Mode

- Local-only communication
- Process isolation via OS
- No network exposure

#### HTTP Mode

- CORS configured for known origins
- Rate limiting recommended via reverse proxy
- HTTPS recommended for production

## Audit Logging

### Security Event Logging

Security-relevant events are logged:

```rust
// File access
tracing::info!(
    "File access: path={}, user={}, result={}",
    path.display(),
    user,
    if result.is_ok() { "success" } else { "denied" }
);

// Command execution
tracing::warn!(
    "Shell command executed: command={}, exit_code={}",
    command,
    exit_code
);

// Permission denied
tracing::error!(
    "Access denied: path={}, reason={}",
    path.display(),
    reason
);
```

### Audit Trail

Operations leave an audit trail in logs:

```
2024-01-15T10:30:00Z INFO File read: .swissarmyhammer/issues/FEATURE_001.md
2024-01-15T10:30:05Z WARN Shell command: cargo build --release
2024-01-15T10:30:10Z ERROR Path traversal attempt: ../../../etc/passwd
```

## Dependency Security

### Supply Chain Security

- Dependencies audited with `cargo audit`
- Minimal dependency tree
- Regular updates for security patches
- No dependencies on unmaintained crates

### Lock File

`Cargo.lock` is committed for reproducible builds:

```bash
# Ensure reproducible builds
git add Cargo.lock
git commit -m "Lock dependencies"
```

## Secure Defaults

### Safe Configuration

Default configuration is secure:

```yaml
# Safe defaults
agent:
  max_tokens: 100000  # Prevents unbounded generation
  timeout: 300        # 5 minute timeout

files:
  max_size: 104857600  # 100 MB limit
  
search:
  max_results: 1000    # Limit result set
  
shell:
  timeout: 120000      # 2 minute timeout
  max_timeout: 600000  # 10 minute maximum
```

### Opt-In Dangerous Operations

Dangerous operations require explicit opt-in:

```rust
// Force push requires explicit flag
if force && branch_name.is_protected() {
    return Err(Error::ProtectedBranchForce {
        branch: branch_name,
    });
}
```

## Vulnerability Response

### Reporting

Security vulnerabilities should be reported to:
- GitHub Security Advisories
- Email: security@swissarmyhammer.org

### Response Process

1. **Acknowledge**: Within 24 hours
2. **Assess**: Severity and impact analysis
3. **Patch**: Develop and test fix
4. **Disclose**: Coordinated disclosure after patch
5. **Release**: Security update with advisory

## Security Best Practices

### For Users

1. **Keep Updated**: Install security updates promptly
2. **Review Permissions**: Audit file access regularly
3. **Limit Scope**: Use project-local configuration
4. **Monitor Logs**: Watch for suspicious activity
5. **Report Issues**: Report security concerns immediately

### For Developers

1. **Validate Input**: Always validate and sanitize input
2. **Sanitize Errors**: Never leak sensitive information in errors
3. **Limit Resources**: Apply appropriate resource limits
4. **Audit Dependencies**: Regular security audits
5. **Test Security**: Include security tests in test suite

## Security Testing

### Static Analysis

```bash
# Run clippy with security lints
cargo clippy -- -W clippy::all -W clippy::pedantic

# Check for known vulnerabilities
cargo audit
```

### Fuzzing

Critical parsers are fuzz tested:

```bash
# Fuzz test path parser
cargo fuzz run path_parser

# Fuzz test command parser
cargo fuzz run command_parser
```

### Security Test Suite

```rust
#[test]
fn test_path_traversal_prevention() {
    let base = PathBuf::from("/safe/dir");
    
    // Should reject traversal attempts
    assert!(validate_path("../../../etc/passwd", Some(&base)).is_err());
    assert!(validate_path("/etc/passwd", Some(&base)).is_err());
    
    // Should accept safe paths
    assert!(validate_path("file.txt", Some(&base)).is_ok());
}

#[test]
fn test_resource_limits() {
    // Should reject oversized files
    let large_file = create_file_of_size(MAX_FILE_SIZE + 1);
    assert!(read_file(&large_file).await.is_err());
    
    // Should accept normal files
    let normal_file = create_file_of_size(1024);
    assert!(read_file(&normal_file).await.is_ok());
}
```

## Compliance

SwissArmyHammer follows security best practices from:

- OWASP Top 10
- CWE/SANS Top 25
- Rust Security Guidelines
- NIST Cybersecurity Framework

## Related Documentation

- [MCP Server](./mcp-server.md)
- [Tool Registry](./tool-registry.md)
- [Storage Backends](./storage-backends.md)
- [Configuration Reference](../reference/configuration.md)
