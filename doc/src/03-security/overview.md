# Security Overview

SwissArmyHammer implements multiple layers of security to protect against common vulnerabilities and ensure safe operation in production environments.

## Security Architecture

The security system is built on defense-in-depth principles with multiple independent layers:

1. **Path Security** - Prevents directory traversal and unauthorized file access
2. **Permission System** - Controls tool execution with user consent flows
3. **Content Security** - Validates and sanitizes all content types
4. **Size Limits** - Protects against resource exhaustion attacks
5. **Network Security** - Prevents SSRF and other network-based attacks

## Security Layers

### 1. Path Security

The `PathValidator` provides comprehensive protection against path-based attacks:

**Features:**
- Absolute path validation (platform-specific)
- Path traversal detection (canonical and string-based)
- Symlink resolution and canonicalization
- Allowed/blocked path lists with prefix matching
- Path length validation
- Null byte detection

**Configuration:**
```rust
// Allow only specific directories
let validator = PathValidator::with_allowed_roots(vec![
    PathBuf::from("/home/user/workspace"),
    PathBuf::from("/tmp"),
]);

// Block sensitive paths
let validator = PathValidator::with_blocked_paths(vec![
    PathBuf::from("/etc"),
    PathBuf::from("/home/user/.ssh"),
]);

// Both allowed and blocked (blocked takes precedence)
let validator = PathValidator::with_allowed_and_blocked(
    vec![PathBuf::from("/home/user")],
    vec![PathBuf::from("/home/user/.ssh")],
);
```

**Protection Against:**
- Directory traversal (`../../../etc/passwd`)
- Symlink attacks
- Path canonicalization bypass
- Null byte injection
- Overly long paths

See [Path Security](path-security.md) for detailed documentation.

### 2. Permission System

The permission system controls which tools can execute and when user consent is required:

**Features:**
- Policy-based authorization
- Risk-level categorization (Low, Medium, High, Critical)
- Pattern matching for tool groups
- Permission persistence ("allow always" / "reject always")
- Session isolation
- Time-based expiration

**Risk Levels:**
- **Low Risk:** Read-only operations, offers "allow always"
- **Medium Risk:** File modifications, offers "allow always"
- **High Risk:** Terminal/network operations, no "allow always"
- **Critical Risk:** Security-sensitive operations, no "allow always"

**Example:**
```rust
let engine = PermissionPolicyEngine::new(storage);

// Evaluate tool call
match engine.evaluate_tool_call("fs_write_file", &args).await? {
    PolicyEvaluation::Allowed => {
        // Execute immediately
    }
    PolicyEvaluation::RequireUserConsent { options } => {
        // Present options to user
    }
    PolicyEvaluation::Denied { reason } => {
        // Block execution
    }
}
```

See [Permission System](permissions.md) for detailed documentation.

### 3. Content Security

The `ContentSecurityValidator` protects against malicious content:

**Features:**
- Base64 validation and size limits
- URI security validation
- SSRF protection
- Content type sniffing
- XSS pattern detection
- Malicious pattern detection (executables, zip bombs)
- Content array limits

**Security Policies:**

**Strict:**
- HTTPS only
- Maximum SSRF protection
- Content sniffing enabled
- Malicious pattern detection enabled
- Small content limits

**Moderate:**
- HTTP/HTTPS/file allowed
- Basic SSRF protection
- Content validation enabled
- Medium content limits

**Permissive:**
- All URI schemes allowed
- Minimal validation
- Large content limits

**Example:**
```rust
let validator = ContentSecurityValidator::strict()?;

// Validate content block
validator.validate_content_security(&content)?;

// Validate URI
validator.validate_uri_security("https://example.com")?;

// Validate base64 data
validator.validate_base64_security(data, "image")?;
```

See [Content Security](content-security.md) for detailed documentation.

### 4. Size Limits

Protection against resource exhaustion:

**Limits by Security Level:**

| Resource | Strict | Moderate | Permissive |
|----------|--------|----------|------------|
| Content (base64) | 5 MB | 10 MB | 25 MB |
| Total resource | 10 MB | 25 MB | 50 MB |
| Content array | 10 items | 50 items | 100 items |
| URI length | 2048 chars | 2048 chars | 8192 chars |
| Path length | 4096 chars | 4096 chars | 4096 chars |

**Implementation:**
```rust
let size_validator = SizeValidator::new(SizeLimits {
    max_path_length: 4096,
    max_uri_length: 2048,
    ..Default::default()
});

size_validator.validate_path_length(path)?;
size_validator.validate_uri_length(uri)?;
```

### 5. Network Security

Protection against server-side request forgery (SSRF):

**Blocked by Default:**
- Private IPv4 ranges (10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16)
- Loopback addresses (127.0.0.0/8, ::1/128)
- Link-local addresses (169.254.0.0/16)
- Localhost and private DNS names

**Allowed URI Schemes:**
- Strict: `https` only
- Moderate: `https`, `http`, `file`
- Permissive: All schemes

**Example:**
```rust
// Blocked URLs (with SSRF protection enabled)
validator.validate_uri_security("http://localhost")?; // Error
validator.validate_uri_security("http://127.0.0.1")?; // Error
validator.validate_uri_security("http://169.254.169.254")?; // Error
validator.validate_uri_security("http://192.168.1.1")?; // Error

// Allowed URLs
validator.validate_uri_security("https://example.com")?; // OK
```

## Security Best Practices

### For Developers

1. **Always Validate Paths**
   - Use `PathValidator` for all file operations
   - Never construct paths from untrusted input without validation
   - Configure allowed/blocked paths based on your use case

2. **Implement Permission Checks**
   - Evaluate permissions before executing tools
   - Store user preferences for better UX
   - Don't offer "allow always" for high-risk operations

3. **Validate All Content**
   - Check content blocks before processing
   - Validate URIs before making requests
   - Use appropriate security level for your context

4. **Handle Errors Securely**
   - Don't leak sensitive information in error messages
   - Log security events for monitoring
   - Fail closed (deny by default)

5. **Test Security Features**
   - Write tests for path traversal attempts
   - Test permission flows
   - Verify SSRF protection

### For Operators

1. **Choose Appropriate Security Level**
   - Use Strict for production environments
   - Use Moderate for development
   - Only use Permissive in trusted, isolated environments

2. **Configure Path Restrictions**
   - Limit file access to necessary directories
   - Block sensitive system paths
   - Use the principle of least privilege

3. **Monitor Security Events**
   - Review security validation failures
   - Watch for repeated permission denials
   - Alert on suspicious patterns

4. **Keep Permissions Minimal**
   - Regularly review stored permissions
   - Clear expired permissions
   - Don't grant "allow always" unnecessarily

5. **Update Security Policies**
   - Review and update blocked path lists
   - Adjust size limits based on needs
   - Update blocked URI patterns as threats evolve

## Common Attack Vectors & Mitigations

### Path Traversal

**Attack:** `../../../../etc/passwd`

**Mitigation:**
- String-based quick check rejects obvious patterns
- Path canonicalization resolves `.` and `..` components
- Component-based validation catches encoded attempts
- Blocked paths prevent access even with valid paths

### SSRF (Server-Side Request Forgery)

**Attack:** `http://169.254.169.254/latest/meta-data/`

**Mitigation:**
- Private IP range blocking
- Localhost/loopback blocking
- DNS resolution validation (if enabled)
- URI scheme restrictions

### Content Type Spoofing

**Attack:** Declaring `image/jpeg` for a `application/x-executable` file

**Mitigation:**
- Magic number detection (first 512 bytes)
- MIME type consistency validation
- Malicious pattern detection
- Content sniffing

### XSS (Cross-Site Scripting)

**Attack:** `<script>alert('xss')</script>` in text content

**Mitigation:**
- Pattern-based detection for common XSS vectors
- Content sanitization (when enabled)
- HTML entity escaping in outputs
- Content Security Policy enforcement

### Zip Bombs / Resource Exhaustion

**Attack:** Highly compressed malicious payloads

**Mitigation:**
- Base64 size limits before decoding
- Total content size limits
- Repetitive pattern detection
- Content array length limits

### Symlink Attacks

**Attack:** Symlink points to sensitive file outside allowed directory

**Mitigation:**
- Path canonicalization resolves symlinks
- Validation on canonical path
- Blocked paths checked after resolution

## Security Considerations by Component

### claude-agent

**Path Security:**
- Implements `PathValidator` with configurable allowed/blocked paths
- All file operations go through path validation
- Supports both strict and non-strict canonicalization modes

**Permission System:**
- Full permission policy engine
- File-based permission storage
- ACP-compliant consent flows
- Risk-based permission options

**Content Security:**
- Validates all content blocks in messages
- SSRF protection on resource URIs
- Content type consistency checking
- Malicious pattern detection

### llama-agent

**Similar Security Features:**
- Shares path validation logic with claude-agent
- Implements ACP security requirements
- Content validation for tool inputs
- Resource limits enforcement

### swissarmyhammer-tools

**MCP Tool Security:**
- Path validation for file operations
- Size limits on file operations
- URI validation for web operations
- Input sanitization for shell commands

## Compliance & Standards

### ACP (Agent Communication Protocol)

SwissArmyHammer implements ACP security requirements:
- Path validation for file operations
- Permission consent flows
- Content security validation
- Error handling and reporting

### OWASP Top 10 Coverage

1. **Injection:** Input validation, parameterized queries
2. **Broken Authentication:** N/A (no authentication layer)
3. **Sensitive Data Exposure:** Path blocking, permission system
4. **XML External Entities:** Not applicable (no XML processing)
5. **Broken Access Control:** Permission system, path validation
6. **Security Misconfiguration:** Secure defaults, validation
7. **XSS:** Content sanitization, pattern detection
8. **Insecure Deserialization:** Size limits, validation
9. **Known Vulnerabilities:** Regular updates, dependency scanning
10. **Insufficient Logging:** Security event logging

## Reporting Security Issues

If you discover a security vulnerability:

1. **Do not** create a public GitHub issue
2. Email security concerns to the maintainers
3. Provide detailed reproduction steps
4. Allow time for fixes before public disclosure

## Further Reading

- [Path Security](path-security.md) - Detailed path validation documentation
- [Permission System](permissions.md) - Authorization and consent flows
- [Content Security](content-security.md) - Content validation and sanitization
- [Configuration](../04-configuration/security.md) - Security configuration options
