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
