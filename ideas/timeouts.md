# Hard-Coded Timeout Values Audit

This document tracks all hard-coded timeout values found in the codebase that violate the principle of avoiding hard-coded constants.

## Major Violations

### Shell Execute Tool
**File:** `swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs:62-72`
```rust
const DEFAULT_MIN_TIMEOUT: u32 = 1;        // 1 second
const DEFAULT_MAX_TIMEOUT: u32 = 1800;     // 30 minutes  
const DEFAULT_DEFAULT_TIMEOUT: u32 = 300;  // 5 minutes
```

### Web Fetch Tool  
**File:** `swissarmyhammer-tools/src/mcp/tools/web_fetch/fetch/mod.rs:16-18`
```rust
const DEFAULT_TIMEOUT_SECONDS: u32 = 30;   // 30 seconds
const MIN_TIMEOUT_SECONDS: u32 = 5;        // 5 seconds
const MAX_TIMEOUT_SECONDS: u32 = 120;      // 2 minutes
```

### Web Search Content Fetcher
**File:** `swissarmyhammer-tools/src/mcp/tools/web_search/content_fetcher.rs:98-101`
```rust
fetch_timeout: Duration::from_secs(45),           // 45 seconds
default_domain_delay: Duration::from_millis(1000), // 1 second
max_domain_delay: Duration::from_secs(30),        // 30 seconds
```

### Workflow Actions
**File:** `swissarmyhammer/src/workflow/actions.rs:1393-1394`
```rust
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);  // 5 minutes
const MAX_TIMEOUT: Duration = Duration::from_secs(3600);     // 1 hour
```

### Shell Security
**File:** `swissarmyhammer/src/shell_security.rs:23-26`
```rust
const DEFAULT_TIMEOUT_SECONDS: u64 = 300;  // 5 minutes
const MAX_TIMEOUT_SECONDS: u64 = 3600;     // 1 hour
```

## Other Hard-Coded Timeout Values

### LLama Agent SSE Keepalive
**File:** `swissarmyhammer/src/workflow/agents/llama_agent_executor.rs:818`
```rust
sse_keep_alive_secs: Some(30)  // 30 second keepalive
```

### Web Search Privacy Settings
**File:** `swissarmyhammer-tools/src/mcp/tools/web_search/privacy.rs:88-91`
```rust
captcha_backoff_initial_ms: 1000,        // 1 second
captcha_backoff_max_ms: 30000,           // 30 seconds
captcha_backoff_duration_mins: 10,       // 10 minutes
```

### Security Check Timeout
**File:** `swissarmyhammer/src/shell_security_hardening.rs:91`
```rust
security_check_timeout: Duration::from_secs(5)  // 5 seconds
```

### Rate Limiter Windows
**Multiple locations with hard-coded 1-minute windows:**
```rust
Duration::from_secs(60)  // 1 minute window
```

**File:** `swissarmyhammer/src/shell_security_hardening.rs:135`
```rust
frequency_window: Duration::from_secs(60 * 60)  // 1 hour
```

### Cache TTLs
**File:** `swissarmyhammer/src/workflow/cache.rs:346-347`
```rust
Duration::from_secs(300)  // 5 minutes cache TTL
```

**Multiple test files with:**
```rust
Duration::from_secs(1)  // 1 second for rate limiter windows
```

### Performance Thresholds
**File:** `swissarmyhammer/src/shell_performance.rs:80,86`
```rust
Duration::from_millis(100)  // 100ms overhead threshold
Duration::from_secs(1)      // 1 second cleanup threshold
```

### Executor Timeouts
**File:** `swissarmyhammer/src/workflow/executor_utils.rs:52-53`
```rust
AgentExecutorType::ClaudeCode => Duration::from_secs(30),  // 30 seconds
AgentExecutorType::LlamaAgent => Duration::from_secs(60),  // 60 seconds
```

## Additional Patterns Found

### Documentation References
- Multiple configuration files reference default values like `300` (5 minutes), `1800` (30 minutes)
- Shell tool documentation shows hard-coded defaults throughout

### Test Files
- Hundreds of hard-coded timeout values in test files using `Duration::from_millis()` and `Duration::from_secs()`
- Test timeouts range from 1ms to several minutes

## Recommended Actions

1. **Move to Configuration**: All timeout constants should be moved to configuration files or environment variables
2. **Create Timeout Config Struct**: Centralized timeout configuration structure
3. **Environment Variable Override**: Allow all timeouts to be overridden via environment variables
4. **Default Value Documentation**: Document all default timeout values and their rationale
5. **Configuration Validation**: Add validation for timeout ranges to prevent invalid values

## Configuration File Structure Suggestion

```toml
[timeouts]
shell_execute_default = 300      # 5 minutes
shell_execute_max = 1800         # 30 minutes
shell_execute_min = 1            # 1 second

web_fetch_default = 30           # 30 seconds
web_fetch_min = 5                # 5 seconds
web_fetch_max = 120              # 2 minutes

workflow_action_default = 300    # 5 minutes
workflow_action_max = 3600       # 1 hour

security_check = 5               # 5 seconds
cache_ttl = 300                 # 5 minutes
rate_limiter_window = 60        # 1 minute
```

## Environment Variable + Hard-Coded Fallback Violations

These are particularly insidious - they appear to be "configurable" via environment variables but still fall back to hard-coded magic numbers:

### Workflow Actions
**File:** `swissarmyhammer/src/workflow/actions.rs:129-141`
```rust
.unwrap_or(3600),    // 1 hour - hard-coded fallback
.unwrap_or(300),     // 5 minutes - hard-coded fallback
.unwrap_or(3600),    // 1 hour - hard-coded fallback
```

### Workflow Executor Core
**File:** `swissarmyhammer/src/workflow/executor/core.rs:268`
```rust
.unwrap_or(3600), // Default - 1 hour hard-coded fallback
```

### Workflow Template Context
**File:** `swissarmyhammer/src/workflow/template_context.rs:127`
```rust
.unwrap_or(30);  // 30 second hard-coded fallback
```

### Config Library
**File:** `swissarmyhammer-config/src/lib.rs:427`
```rust
.unwrap_or(120),  // 120 second hard-coded fallback
```

### Web Fetch Tool (Double Violation)
**File:** `swissarmyhammer-tools/src/mcp/tools/web_fetch/fetch/mod.rs:103`
```rust
request.timeout.unwrap_or(DEFAULT_TIMEOUT_SECONDS) as u64,
```
*This defaults to a constant that's already hard-coded!*

### Test Configuration
**File:** `swissarmyhammer-config/tests/llama_test_config.rs:56,61`
```rust
.unwrap_or(60)   // 60 seconds in CI - hard-coded
.unwrap_or(120)  // 120 seconds locally - hard-coded
```

### Web Search Results Count
**File:** `swissarmyhammer-tools/src/mcp/tools/web_search/duckduckgo_client.rs:299`
```rust
request.results_count.unwrap_or(10)  // Hard-coded 10 results
```

## Environment Variables Suggestion

```bash
SAH_TIMEOUT_SHELL_EXECUTE_DEFAULT=300
SAH_TIMEOUT_SHELL_EXECUTE_MAX=1800
SAH_TIMEOUT_WEB_FETCH_DEFAULT=30
SAH_TIMEOUT_WORKFLOW_ACTION_DEFAULT=300
# ... etc
```