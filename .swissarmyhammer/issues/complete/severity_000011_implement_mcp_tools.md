# Step 11: Implement Severity for MCP Tool Errors

**Refer to ideas/severity.md**

## Goal

Implement the `Severity` trait for MCP tool-specific error types in swissarmyhammer-tools.

## Context

The tools crate contains several tool-specific error types:
- SecurityError (web fetch)
- ContentFetchError (web search)
- DuckDuckGoError (web search)
- ToolValidationError
- ValidationError (tool registry)
- SendError (notifications and progress)

## Tasks

### 1. Ensure swissarmyhammer-common Dependency

Verify `swissarmyhammer-tools/Cargo.toml` depends on swissarmyhammer-common (it likely already does).

### 2. Implement Severity for SecurityError

In `swissarmyhammer-tools/src/mcp/tools/web_fetch/security.rs`:

```rust
use swissarmyhammer_common::{ErrorSeverity, Severity};

impl Severity for SecurityError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // All security violations are Critical
            SecurityError::BlockedDomain { .. } => ErrorSeverity::Critical,
            SecurityError::UnsafeUrl { .. } => ErrorSeverity::Critical,
            SecurityError::ContentSecurityViolation { .. } => ErrorSeverity::Critical,
            SecurityError::RateLimitExceeded { .. } => ErrorSeverity::Error,
        }
    }
}
```

### 3. Implement Severity for ContentFetchError

In `swissarmyhammer-tools/src/mcp/tools/web_search/content_fetcher.rs`:

```rust
use swissarmyhammer_common::{ErrorSeverity, Severity};

impl Severity for ContentFetchError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // Critical: System cannot fetch content
            ContentFetchError::HttpClientFailed { .. } => ErrorSeverity::Critical,
            
            // Error: Fetch operation failed
            ContentFetchError::RequestFailed { .. } => ErrorSeverity::Error,
            ContentFetchError::InvalidUrl { .. } => ErrorSeverity::Error,
            ContentFetchError::ConversionFailed { .. } => ErrorSeverity::Error,
            ContentFetchError::TimeoutError { .. } => ErrorSeverity::Error,
            
            // Warning: Non-critical issues
            ContentFetchError::PartialContent { .. } => ErrorSeverity::Warning,
        }
    }
}
```

### 4. Implement Severity for DuckDuckGoError

In `swissarmyhammer-tools/src/mcp/tools/web_search/duckduckgo_client.rs`:

```rust
use swissarmyhammer_common::{ErrorSeverity, Severity};

impl Severity for DuckDuckGoError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // Critical: Search system cannot function
            DuckDuckGoError::ClientInitializationFailed { .. } => ErrorSeverity::Critical,
            
            // Error: Search operation failed
            DuckDuckGoError::SearchFailed { .. } => ErrorSeverity::Error,
            DuckDuckGoError::ParseError { .. } => ErrorSeverity::Error,
            DuckDuckGoError::RateLimited { .. } => ErrorSeverity::Error,
            
            // Warning: Search issues but can continue
            DuckDuckGoError::NoResults { .. } => ErrorSeverity::Warning,
        }
    }
}
```

### 5. Implement Severity for Tool Registry Errors

In `swissarmyhammer-tools/src/mcp/tool_registry.rs`:

```rust
use swissarmyhammer_common::{ErrorSeverity, Severity};

impl Severity for ToolValidationError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            ToolValidationError::InvalidToolDefinition { .. } => ErrorSeverity::Critical,
            ToolValidationError::MissingRequiredField { .. } => ErrorSeverity::Error,
            ToolValidationError::InvalidParameter { .. } => ErrorSeverity::Error,
        }
    }
}

impl Severity for ValidationError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            ValidationError::SchemaValidationFailed { .. } => ErrorSeverity::Critical,
            ValidationError::InvalidSchema { .. } => ErrorSeverity::Error,
            ValidationError::MissingField { .. } => ErrorSeverity::Error,
        }
    }
}
```

### 6. Implement Severity for Notification SendError

In `swissarmyhammer-tools/src/mcp/notifications.rs` and `progress_notifications.rs`:

```rust
use swissarmyhammer_common::{ErrorSeverity, Severity};

impl Severity for SendError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // Warning: Notification failures should not block operations
            SendError::ChannelClosed => ErrorSeverity::Warning,
            SendError::SerializationFailed { .. } => ErrorSeverity::Warning,
        }
    }
}
```

### 7. Add Tests for All Implementations

Add severity tests in each file where error types are defined.

## Severity Guidelines

### Security Errors
**Critical**: All security violations (blocked domains, unsafe URLs)
**Error**: Rate limit exceeded (operational, not security)

### Web Fetch/Search Errors
**Critical**: HTTP client failed, initialization failures
**Error**: Request failures, timeouts, parse errors
**Warning**: No results, partial content

### Tool Registry Errors
**Critical**: Invalid tool definitions, schema validation failures
**Error**: Missing fields, invalid parameters

### Notification Errors
**Warning**: All notification errors (should not block operations)

## Acceptance Criteria

- [ ] All 6+ error types implement Severity trait
- [ ] Unit tests for each implementation
- [ ] Tests pass: `cargo test -p swissarmyhammer-tools`
- [ ] Code compiles: `cargo build -p swissarmyhammer-tools`
- [ ] Clippy clean: `cargo clippy -p swissarmyhammer-tools`

## Files to Modify

- `swissarmyhammer-tools/src/mcp/tools/web_fetch/security.rs`
- `swissarmyhammer-tools/src/mcp/tools/web_search/content_fetcher.rs`
- `swissarmyhammer-tools/src/mcp/tools/web_search/duckduckgo_client.rs`
- `swissarmyhammer-tools/src/mcp/tool_registry.rs`
- `swissarmyhammer-tools/src/mcp/notifications.rs`
- `swissarmyhammer-tools/src/mcp/progress_notifications.rs`

## Estimated Changes

~150 lines of code (6+ implementations + tests)

## Next Step

Step 12: Implement Severity for PlanCommandError



## Proposed Solution

After reviewing the code, I've identified that the issue description is partially incorrect. The tools crate has **only 3 error types** that need Severity implementation, not 6+:

1. **SecurityError** in `swissarmyhammer-tools/src/mcp/tools/web_fetch/security.rs`
   - 4 variants: InvalidUrl, BlockedDomain, SsrfAttempt, UnsupportedScheme
   
2. **ContentFetchError** in `swissarmyhammer-tools/src/mcp/tools/web_search/content_fetcher.rs`
   - 6 variants: HttpError, NetworkError, ProcessingError, RateLimited, QualityCheckFailed, Timeout, InvalidUrl

3. **DuckDuckGoError** in `swissarmyhammer-tools/src/mcp/tools/web_search/duckduckgo_client.rs`
   - 7 variants: Browser, Parse, InvalidRequest, NoResults, ElementNotFound, Timeout, CaptchaDetected

**Note:** The issue mentions ToolValidationError, ValidationError, and SendError but these types **do not exist** in the current codebase. I've searched thoroughly and confirmed they are not present.

### Implementation Steps

1. **SecurityError severity mapping:**
   - `InvalidUrl` → Critical (prevents any web fetch operation)
   - `BlockedDomain` → Critical (security policy violation)
   - `SsrfAttempt` → Critical (security attack attempt)
   - `UnsupportedScheme` → Critical (security policy violation)

2. **ContentFetchError severity mapping:**
   - `HttpError` → Error (fetch operation failed)
   - `NetworkError` → Error (fetch operation failed)
   - `ProcessingError` → Error (content processing failed)
   - `RateLimited` → Error (operational limit reached)
   - `QualityCheckFailed` → Warning (content available but low quality)
   - `Timeout` → Error (fetch operation failed)
   - `InvalidUrl` → Error (invalid input)

3. **DuckDuckGoError severity mapping:**
   - `Browser` → Critical (browser automation system failed)
   - `Parse` → Error (result parsing failed)
   - `InvalidRequest` → Error (invalid search parameters)
   - `NoResults` → Warning (search completed but found nothing)
   - `ElementNotFound` → Error (page structure changed)
   - `Timeout` → Error (search operation failed)
   - `CaptchaDetected` → Error (search blocked by CAPTCHA)

### Testing Approach

Will add unit tests for each implementation following the pattern in swissarmyhammer-common/src/error.rs:

```rust
#[test]
fn test_security_error_severity() {
    let errors = vec![
        SecurityError::InvalidUrl("test".to_string()),
        SecurityError::BlockedDomain("test".to_string()),
        SecurityError::SsrfAttempt("test".to_string()),
        SecurityError::UnsupportedScheme("test".to_string()),
    ];
    
    for error in errors {
        assert_eq!(error.severity(), ErrorSeverity::Critical);
    }
}
```

Similar tests will be created for ContentFetchError and DuckDuckGoError with appropriate severity assertions.



## Implementation Complete

Successfully implemented the `Severity` trait for all 3 MCP tool error types:

### 1. SecurityError (`swissarmyhammer-tools/src/mcp/tools/web_fetch/security.rs`)
- **All 4 variants → Critical**
  - `InvalidUrl` - Prevents web fetch operation
  - `BlockedDomain` - Security policy violation
  - `SsrfAttempt` - Security attack attempt
  - `UnsupportedScheme` - Security policy violation
- Added comprehensive unit test `test_security_error_severity()`

### 2. ContentFetchError (`swissarmyhammer-tools/src/mcp/tools/web_search/content_fetcher.rs`)
- **6 variants → Error**
  - `HttpError` - Fetch operation failed
  - `NetworkError` - Fetch operation failed
  - `ProcessingError` - Content processing failed
  - `RateLimited` - Operational limit reached
  - `Timeout` - Fetch operation failed
  - `InvalidUrl` - Invalid input
- **1 variant → Warning**
  - `QualityCheckFailed` - Content available but low quality
- Added comprehensive unit test `test_content_fetch_error_severity()`

### 3. DuckDuckGoError (`swissarmyhammer-tools/src/mcp/tools/web_search/duckduckgo_client.rs`)
- **1 variant → Critical**
  - `Browser` - Browser automation system failed
- **5 variants → Error**
  - `Parse` - Result parsing failed
  - `InvalidRequest` - Invalid search parameters
  - `ElementNotFound` - Page structure changed
  - `Timeout` - Search operation failed
  - `CaptchaDetected` - Search blocked by CAPTCHA
- **1 variant → Warning**
  - `NoResults` - Search completed but found nothing
- Added comprehensive unit test `test_duckduckgo_error_severity()`

### Test Results
All tests passed successfully:
```
Summary [17.549s] 602 tests run: 602 passed (5 slow), 0 skipped
```

### Code Changes
- Total of ~90 lines added (implementations + tests)
- All error types now properly categorized by severity
- Consistent with severity guidelines from swissarmyhammer-common
- No clippy warnings or compilation errors

### Notes
The issue description mentioned 6+ error types including ToolValidationError, ValidationError, and SendError. After thorough investigation, these types **do not exist** in the current codebase. Only the 3 error types listed above were found and implemented.



## Final Verification

### Build Status: ✅ PASSED
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.41s
```

### Clippy Status: ✅ PASSED
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 18.22s
```
No warnings or errors.

### Test Status: ✅ PASSED
```
Summary [17.549s] 602 tests run: 602 passed (5 slow), 0 skipped
```

All acceptance criteria met:
- ✅ All 3 error types implement Severity trait
- ✅ Unit tests for each implementation
- ✅ Tests pass: `cargo test -p swissarmyhammer-tools`
- ✅ Code compiles: `cargo build -p swissarmyhammer-tools`
- ✅ Clippy clean: `cargo clippy -p swissarmyhammer-tools`

## Summary

Successfully implemented Severity trait for MCP tool errors. The implementation correctly categorizes errors by their operational impact:
- **Critical**: Security violations and browser system failures
- **Error**: Operation failures (HTTP, network, parsing, timeouts)
- **Warning**: Non-blocking issues (quality checks, no results)

The implementation is consistent with the severity guidelines established in swissarmyhammer-common and follows the same patterns used throughout the codebase.
