# How to Use Per-Rule Tool Filtering

## Overview

Rules can restrict which MCP tools are available to the LLM during rule checking by specifying regex patterns in the rule's YAML frontmatter.

## Rule Frontmatter

```yaml
---
name: no-code-duplication
severity: error
denied_tools_regex:
  - ".*"  # Deny all tools
---

Check for code duplication without using any tools.
```

Or with allow list:

```yaml
---
name: check-security
severity: error
allowed_tools_regex:
  - "^files_(read|grep|glob)$"  # Only allow read-only file tools
denied_tools_regex:
  - "^shell_.*"  # Explicitly deny shell tools (redundant with allow list, but clear)
---

Check for security issues using only safe file reading tools.
```

## How It Works (Manual Integration Required)

The infrastructure is complete but requires manual integration at the call site:

```rust
use swissarmyhammer_mcp_proxy::{FilteringMcpProxy, ToolFilter, start_proxy_server};
use swissarmyhammer_rules::{Rule, RuleChecker};
use swissarmyhammer_agent_executor::AgentExecutorFactory;
use std::sync::Arc;

async fn check_rule_with_filtering(
    rule: &Rule,
    target_path: &Path,
    mcp_server: Arc<McpServer>,
) -> Result<Option<RuleViolation>> {
    // Check if rule has tool filtering
    if rule.has_tool_filter() {
        // Get filter patterns from rule
        let allowed = rule.get_allowed_tools_regex().unwrap_or_default();
        let denied = rule.get_denied_tools_regex().unwrap_or_default();

        // Create filter
        let filter = ToolFilter::new(allowed, denied)?;

        // Create proxy wrapping main server
        let proxy = Arc::new(FilteringMcpProxy::new(mcp_server.clone(), filter));

        // Start HTTP server for proxy
        let (proxy_port, proxy_handle) = start_proxy_server(proxy, None).await?;

        // Create agent config with proxy port
        let agent_config = AgentConfig::default();  // Or from context
        let mcp_server_for_agent = agent_client_protocol::McpServer::Http {
            name: "sah-filtered".to_string(),
            url: format!("http://127.0.0.1:{}/mcp", proxy_port),
            headers: vec![],
        };

        // Create agent executor
        let agent = AgentExecutorFactory::create_executor(&agent_config, Some(mcp_server_for_agent)).await?;

        // Create checker with filtered agent
        let mut checker = RuleChecker::new(agent)?;
        checker.initialize().await?;

        // Run check
        let result = checker.check_file(rule, target_path).await;

        // Cleanup proxy
        proxy_handle.abort();

        result
    } else {
        // No filtering - use default agent
        let agent = create_default_agent().await?;
        let mut checker = RuleChecker::new(agent)?;
        checker.initialize().await?;
        checker.check_file(rule, target_path).await
    }
}
```

## Why Not Automatic?

**Circular Dependency Problem:**
```
tools → mcp-proxy → tools (CYCLE!)
```

- `swissarmyhammer-tools` contains McpServer
- `swissarmyhammer-mcp-proxy` needs McpServer (depends on tools)
- `swissarmyhammer-tools` would need FilteringMcpProxy (can't depend on mcp-proxy)

**Solution:** Proxy creation happens in higher-level code (like CLI) that can import both crates.

## Future: Automatic Integration

To make this automatic in `rules_check` tool, we would need to:

1. **Option A**: Move McpServer to a separate `swissarmyhammer-mcp-server` crate
   - Both tools and mcp-proxy depend on mcp-server
   - No circular dependency
   - Large refactor

2. **Option B**: Create a trait-based callback system
   - RuleChecker accepts a `ProxyFactory` trait
   - Caller implements the trait with access to both crates
   - More complex but avoids refactor

3. **Option C**: Move rule checking entirely out of tools crate
   - Create `swissarmyhammer-rule-check-tool` crate
   - Can depend on both tools and mcp-proxy
   - Clean separation but more crates

## Current Status

- ✓ Infrastructure complete: proxy crate works, tested, ready
- ✓ Rule frontmatter parsing: `allowed_tools_regex` and `denied_tools_regex` supported
- ✓ `check_file()` signature updated: removed OnceCell, creates fresh checker
- ✗ **Not automatic**: Requires manual integration at call site

## Example: CLI Integration

In `swissarmyhammer-cli` (which can import both tools and mcp-proxy):

```rust
// When running rules check:
for rule in rules {
    let filtered_agent = if rule.has_tool_filter() {
        Some(create_filtered_agent(rule, mcp_server).await?)
    } else {
        None
    };

    let result = checker.check_file(rule, target, filtered_agent).await?;
}
```

This keeps the tools crate dependency-clean while enabling the feature where it's needed.
