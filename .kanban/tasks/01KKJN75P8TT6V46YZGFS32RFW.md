---
position_column: done
position_ordinal: ffffff8b80
title: '[nit] `log_retry_attempt` uses emoji in a tracing warn! call — inconsistent with codebase style'
---
**File:** `swissarmyhammer-tools/src/mcp/server.rs` line 124\n**Severity:** nit\n\n```rust\ntracing::warn!(\"⚠️ {} attempt {} failed, retrying in {}ms: {}\", ...);\n```\n\nAnd line 160:\n```rust\ntracing::info!(\"✓ {} succeeded on attempt {}\", ...);\n```\n\nThe project style guide (AGENT.md) explicitly states \"avoid adding emojis\" and the rest of the codebase's tracing calls do not use emoji. These two lines are the only occurrences of emoji in tracing output. Remove them for consistency." #review-finding