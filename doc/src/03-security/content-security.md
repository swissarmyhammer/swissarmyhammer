# Content Security

The `ContentSecurityValidator` provides comprehensive protection against malicious content including SSRF attacks, XSS injection, content type spoofing, and resource exhaustion attacks.

## Overview

Content security is critical when processing user-provided or external data. SwissArmyHammer validates:

- Base64-encoded binary data
- URIs and URLs
- Text content
- Resource content (embedded and linked)
- Content arrays and total size

## Security Policies

Three pre-configured security levels are available:

### Strict Policy

Maximum security for production environments:

```rust
let validator = ContentSecurityValidator::strict()?;
```

**Configuration:**
- URI schemes: `https` only
- Max base64 size: 5 MB
- Max total content: 10 MB
- Content array limit: 10 items
- SSRF protection: Full (blocks all private IPs)
- Content sniffing: Enabled
- Malicious pattern detection: Enabled
- Rate limiting: 60 requests/minute

**Blocked URIs:**
- `localhost` and `127.*`
- Private IPv4: `192.168.*`, `10.*`, `172.16-31.*`
- IPv6 loopback: `::1`

### Moderate Policy

Balanced security for development:

```rust
let validator = ContentSecurityValidator::moderate()?;
```

**Configuration:**
- URI schemes: `https`, `http`, `file`
- Max base64 size: 10 MB
- Max total content: 25 MB
- Content array limit: 50 items
- SSRF protection: Basic (blocks localhost)
- Content sniffing: Enabled
- Malicious pattern detection: Enabled
- Rate limiting: 300 requests/minute

**Blocked URIs:**
- `localhost` and `127.0.0.1`
- IPv6 loopback: `::1`

### Permissive Policy

Minimal restrictions for trusted environments:

```rust
let validator = ContentSecurityValidator::permissive()?;
```

**Configuration:**
- URI schemes: All (`https`, `http`, `file`, `data`, `ftp`)
- Max base64 size: 25 MB
- Max total content: 50 MB
- Content array limit: 100 items
- SSRF protection: Disabled
- Content sniffing: Disabled
- Malicious pattern detection: Disabled
- Rate limiting: Disabled

## Usage

### Basic Content Validation

```rust
use claude_agent::content_security_validator::ContentSecurityValidator;
use agent_client_protocol::ContentBlock;

let validator = ContentSecurityValidator::moderate()?;

// Validate a content block
validator.validate_content_security(&content_block)?;
```

### Validate Content Arrays

```rust
let content_blocks = vec![
    ContentBlock::Text(text_content),
    ContentBlock::Image(image_content),
];

validator.validate_content_blocks_security(&content_blocks)?;
```

### URI Security Validation

```rust
// Valid URI
validator.validate_uri_security("https://example.com")?;

// Blocked by SSRF protection
let result = validator.validate_uri_security("http://localhost");
assert!(matches!(result, Err(ContentSecurityError::SsrfProtectionTriggered { .. })));
```

### Base64 Security Validation

```rust
let base64_data = "SGVsbG8gV29ybGQ="; // "Hello World"
validator.validate_base64_security(base64_data, "image")?;
```

### Text Content Safety

```rust
use agent_client_protocol::TextContent;

let text = TextContent {
    text: "Safe content".to_string(),
    annotations: None,
    meta: None,
};

validator.validate_text_security(&text)?;
```

## Validation Features

### 1. Base64 Security

Validates base64-encoded binary data for size, format, and malicious patterns.

**Checks:**
- Base64 format validation (valid characters, padding)
- Size limits (before and after decoding)
- Malicious pattern detection (executables, zip bombs)
- Repetitive pattern detection

**Example:**
```rust
// Valid base64
let valid = "SGVsbG8gV29ybGQ=";
assert!(validator.validate_base64_security(valid, "test").is_ok());

// Invalid base64
let invalid = "Not!Valid@Base64#";
assert!(validator.validate_base64_security(invalid, "test").is_err());

// Too large
let large = "A".repeat(10 * 1024 * 1024); // 10 MB
let result = validator.validate_base64_security(&large, "test");
assert!(matches!(result, Err(ContentSecurityError::Base64SecurityViolation { .. })));
```

**Malicious Patterns Detected:**
- PE executables (Windows): `TVq` prefix
- ELF executables (Linux): `f0VMR` prefix
- Highly repetitive data (potential zip bombs)

### 2. URI Security & SSRF Protection

Protects against Server-Side Request Forgery attacks.

**Validation:**
- URI format validation
- Scheme whitelist enforcement
- Length limits
- Pattern-based blocking
- IP range blocking

**SSRF Protection:**

**Blocked Addresses (Strict):**
```rust
// Private IPv4 ranges
http://10.0.0.1           // 10.0.0.0/8
http://172.16.0.1         // 172.16.0.0/12
http://192.168.1.1        // 192.168.0.0/16

// Loopback
http://127.0.0.1          // 127.0.0.0/8
http://localhost          // Resolved to 127.0.0.1

// Link-local
http://169.254.169.254    // AWS metadata service

// IPv6
http://[::1]              // IPv6 loopback
```

**Allowed Addresses:**
```rust
https://example.com       // Public domain
https://1.1.1.1          // Public IP
https://[2606:4700::1111] // Public IPv6
```

**Example:**
```rust
let validator = ContentSecurityValidator::strict()?;

// Allowed
assert!(validator.validate_uri_security("https://example.com").is_ok());

// Blocked - localhost
let result = validator.validate_uri_security("http://localhost");
assert!(matches!(result, Err(ContentSecurityError::SsrfProtectionTriggered { .. })));

// Blocked - private IP
let result = validator.validate_uri_security("http://192.168.1.1");
assert!(matches!(result, Err(ContentSecurityError::SsrfProtectionTriggered { .. })));

// Blocked - metadata service
let result = validator.validate_uri_security("http://169.254.169.254");
assert!(matches!(result, Err(ContentSecurityError::SsrfProtectionTriggered { .. })));
```

### 3. Content Type Sniffing

Detects content type spoofing by examining magic numbers.

**How It Works:**
1. Decode first 512 bytes of base64 data
2. Examine magic numbers using `infer` crate
3. Compare detected type with declared MIME type
4. Reject if mismatch detected

**Supported Formats:**
- Images: PNG, JPEG, GIF, WebP, etc.
- Audio: MP3, WAV, FLAC, OGG, etc.
- Video: MP4, AVI, WebM, etc.
- Documents: PDF, ZIP, etc.

**Example:**
```rust
// 1x1 PNG image
let png_data = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";

// Correct type - OK
assert!(validator
    .validate_content_type_consistency(png_data, "image/png")
    .is_ok());

// Wrong type - Error
let result = validator
    .validate_content_type_consistency(png_data, "image/jpeg");
assert!(matches!(result, 
    Err(ContentSecurityError::ContentTypeSpoofingDetected { .. })));
```

**Magic Number Detection:**
```rust
let validator = ContentSecurityValidator::moderate()?;

// PNG magic: 89 50 4E 47
let png_bytes = vec![0x89, 0x50, 0x4E, 0x47, ...];
assert_eq!(validator.sniff_content_type(&png_bytes), Some("image/png"));

// JPEG magic: FF D8 FF
let jpeg_bytes = vec![0xFF, 0xD8, 0xFF, ...];
assert_eq!(validator.sniff_content_type(&jpeg_bytes), Some("image/jpeg"));
```

### 4. Text Content Safety

Detects potentially dangerous patterns in text content.

**Patterns Detected:**
- `<script` - Script injection
- `javascript:` - JavaScript protocol
- `onload=` - Event handler injection
- `onerror=` - Error handler injection
- `eval(` - Dynamic code execution
- `document.cookie` - Cookie theft

**Example:**
```rust
// Safe text
let safe = TextContent {
    text: "Normal text content".to_string(),
    annotations: None,
    meta: None,
};
assert!(validator.validate_text_security(&safe).is_ok());

// Dangerous text
let dangerous = TextContent {
    text: "<script>alert('xss')</script>".to_string(),
    annotations: None,
    meta: None,
};
let result = validator.validate_text_security(&dangerous);
assert!(matches!(result, 
    Err(ContentSecurityError::ContentSanitizationFailed { .. })));
```

**Note:** This is a basic check. For comprehensive XSS protection, use a dedicated HTML sanitization library.

### 5. Resource Content Validation

Validates embedded and linked resources.

**Text Resources:**
```rust
use agent_client_protocol::{EmbeddedResource, TextResourceContents};

let text_resource = TextResourceContents {
    uri: "https://example.com/data.json".to_string(),
    text: "{ \"key\": \"value\" }".to_string(),
    mime_type: None,
    meta: None,
};

let embedded = EmbeddedResource {
    resource: EmbeddedResourceResource::TextResourceContents(text_resource),
    annotations: None,
    meta: None,
};

validator.validate_resource_content(&embedded)?;
```

**Blob Resources:**
```rust
use agent_client_protocol::BlobResourceContents;

let blob_resource = BlobResourceContents {
    uri: "https://example.com/image.png".to_string(),
    blob: "iVBORw0KGgo...".to_string(),
    mime_type: Some("image/png".to_string()),
    meta: None,
};

let embedded = EmbeddedResource {
    resource: EmbeddedResourceResource::BlobResourceContents(blob_resource),
    annotations: None,
    meta: None,
};

validator.validate_resource_content(&embedded)?;
```

### 6. Content Array Limits

Prevents DoS attacks via excessive content items.

**Limits by Policy:**
- Strict: 10 items
- Moderate: 50 items
- Permissive: 100 items

**Total Size Validation:**

Estimated total size across all content blocks:
- Text: character count
- Base64: decoded size estimate (length × 3 / 4)
- Resources: conservative estimates

**Example:**
```rust
let many_items = vec![
    ContentBlock::Text(TextContent {
        text: "item".to_string(),
        annotations: None,
        meta: None,
    });
    100
];

let result = validator.validate_content_blocks_security(&many_items);
// Fails if limit exceeded
```

## Error Handling

### ContentSecurityError Types

```rust
pub enum ContentSecurityError {
    SecurityValidationFailed { reason: String, policy_violated: String },
    SuspiciousContentDetected { threat_type: String, details: String },
    DoSProtectionTriggered { protection_type: String, threshold: String },
    UriSecurityViolation { uri: String, reason: String },
    Base64SecurityViolation { reason: String },
    ContentTypeSpoofingDetected { declared: String, actual: String },
    ContentSanitizationFailed { reason: String },
    SsrfProtectionTriggered { target: String, reason: String },
    MemoryLimitExceeded { actual: usize, limit: usize },
    RateLimitExceeded { operation: String },
    ContentArrayTooLarge { length: usize, max_length: usize },
    InvalidContentEncoding { encoding: String },
    MaliciousPatternDetected { pattern_type: String },
}
```

### Common Error Scenarios

#### SSRF Protection

```rust
match validator.validate_uri_security("http://localhost") {
    Err(ContentSecurityError::SsrfProtectionTriggered { target, reason }) => {
        eprintln!("Blocked access to {}: {}", target, reason);
    }
    _ => {}
}
```

#### Content Type Spoofing

```rust
match validator.validate_content_type_consistency(data, "image/jpeg") {
    Err(ContentSecurityError::ContentTypeSpoofingDetected { declared, actual }) => {
        eprintln!("Type mismatch: declared {} but detected {}", declared, actual);
    }
    _ => {}
}
```

#### Size Limits

```rust
match validator.validate_base64_security(large_data, "image") {
    Err(ContentSecurityError::Base64SecurityViolation { reason }) => {
        eprintln!("Size limit exceeded: {}", reason);
    }
    _ => {}
}
```

## Custom Security Policies

Create custom policies for specific requirements:

```rust
use claude_agent::content_security_validator::{SecurityPolicy, SecurityLevel};
use std::collections::HashSet;

let mut custom_policy = SecurityPolicy::moderate();

// Customize settings
custom_policy.max_base64_size = 20 * 1024 * 1024; // 20 MB
custom_policy.enable_ssrf_protection = true;
custom_policy.allowed_uri_schemes = {
    let mut schemes = HashSet::new();
    schemes.insert("https".to_string());
    schemes
};

// Add custom blocked patterns
custom_policy.blocked_uri_patterns.push(r"internal\.company\.com".to_string());

let validator = ContentSecurityValidator::new(custom_policy)?;
```

## Security Best Practices

### 1. Choose Appropriate Policy

```rust
// Production - strict
let validator = ContentSecurityValidator::strict()?;

// Development - moderate
let validator = ContentSecurityValidator::moderate()?;

// Testing only - permissive
let validator = ContentSecurityValidator::permissive()?;
```

### 2. Validate All External Content

```rust
// User-provided content
validator.validate_content_blocks_security(&user_content)?;

// Content from external APIs
validator.validate_uri_security(&api_url)?;
validator.validate_content_security(&api_response)?;
```

### 3. Handle Errors Appropriately

```rust
match validator.validate_content_security(&content) {
    Ok(()) => process_content(content),
    Err(ContentSecurityError::SsrfProtectionTriggered { .. }) => {
        log_security_event("SSRF attempt blocked");
        Err("Invalid URL")
    }
    Err(ContentSecurityError::MaliciousPatternDetected { .. }) => {
        log_security_event("Malicious content detected");
        Err("Unsafe content")
    }
    Err(e) => {
        log::warn!("Content validation failed: {}", e);
        Err("Validation failed")
    }
}
```

### 4. Log Security Events

```rust
use tracing::{warn, error};

if let Err(e) = validator.validate_uri_security(uri) {
    match &e {
        ContentSecurityError::SsrfProtectionTriggered { target, reason } => {
            error!("SSRF attempt: {} - {}", target, reason);
        }
        ContentSecurityError::MaliciousPatternDetected { pattern_type } => {
            error!("Malicious pattern: {}", pattern_type);
        }
        _ => {
            warn!("Content security error: {}", e);
        }
    }
    return Err(e);
}
```

### 5. Defense in Depth

Combine content security with other security layers:

```rust
// Path validation
let safe_path = path_validator.validate_absolute_path(path)?;

// Content validation
validator.validate_content_security(&content)?;

// Permission check
permission_engine.evaluate_tool_call("fs_write", &args).await?;

// Proceed with operation
std::fs::write(&safe_path, content)?;
```

## Attack Scenarios & Mitigations

### SSRF (Server-Side Request Forgery)

**Attack:**
```
https://api.example.com/fetch?url=http://169.254.169.254/latest/meta-data/iam/security-credentials/
```

**Mitigation:**
- Private IP blocking
- Metadata service blocking
- DNS resolution validation
- URI scheme restrictions

### Content Type Spoofing

**Attack:**
Upload executable disguised as image:
```rust
// PE executable but declared as image
let malicious = ImageContent {
    data: "TVqQAAMAAAAEAAAA...", // PE header
    mime_type: "image/png".to_string(),
    uri: None,
};
```

**Mitigation:**
- Magic number detection
- Content type consistency check
- Extension validation

### XSS via Content

**Attack:**
```rust
let xss = TextContent {
    text: "<img src=x onerror='alert(1)'>".to_string(),
    ...
};
```

**Mitigation:**
- Pattern-based detection
- HTML sanitization
- Content Security Policy
- Output encoding

### Zip Bombs

**Attack:**
Highly compressed data that expands to huge size:
```
42.zip: 42 KB → 4.5 PB (uncompressed)
```

**Mitigation:**
- Size limits before decoding
- Repetitive pattern detection
- Decompression limits
- Stream processing

### Resource Exhaustion

**Attack:**
Send many large content blocks to exhaust memory:
```rust
let attack = vec![large_content; 1000];
```

**Mitigation:**
- Content array limits
- Total size limits
- Rate limiting
- Memory monitoring

## Testing

### Test SSRF Protection

```rust
#[test]
fn test_ssrf_protection() {
    let validator = ContentSecurityValidator::strict().unwrap();
    
    let blocked_urls = vec![
        "http://localhost",
        "http://127.0.0.1",
        "http://169.254.169.254",
        "http://192.168.1.1",
        "http://10.0.0.1",
    ];
    
    for url in blocked_urls {
        let result = validator.validate_uri_security(url);
        assert!(matches!(result, 
            Err(ContentSecurityError::SsrfProtectionTriggered { .. })));
    }
}
```

### Test Content Type Validation

```rust
#[test]
fn test_content_type_spoofing() {
    let validator = ContentSecurityValidator::moderate().unwrap();
    
    let png_data = "iVBORw0KGgo..."; // PNG image
    
    // Correct type
    assert!(validator
        .validate_content_type_consistency(png_data, "image/png")
        .is_ok());
    
    // Spoofed type
    assert!(validator
        .validate_content_type_consistency(png_data, "image/jpeg")
        .is_err());
}
```

### Test Size Limits

```rust
#[test]
fn test_size_limits() {
    let strict = ContentSecurityValidator::strict().unwrap();
    
    let large_data = "A".repeat(10 * 1024 * 1024); // 10 MB
    
    let result = strict.validate_base64_security(&large_data, "test");
    assert!(matches!(result, 
        Err(ContentSecurityError::Base64SecurityViolation { .. })));
}
```

## See Also

- [Security Overview](overview.md) - Comprehensive security documentation
- [Path Security](path-security.md) - Path validation and file access control
- [Permission System](permissions.md) - Tool authorization system
