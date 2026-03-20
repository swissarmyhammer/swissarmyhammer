---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffcd80
title: version key set with String::to_string() when a &str literal suffices
---
**File:** `swissarmyhammer-tools/src/mcp/tools/skill/use_op.rs:58-61` and `claude-agent/src/agent.rs:447-449`\n\n**What:** Both injection sites call `template_context.set(\"version\".to_string(), ...)`. If `TemplateContext::set` accepts `impl Into<String>` or `&str`, passing an owned `String` from a literal is an unnecessary allocation.\n\n**Suggestion:** Check the signature of `TemplateContext::set`. If it accepts `impl Into<String>`, pass `\"version\"` directly (the literal is `Copy`-able and the `Into<String>` impl will handle it). This is a zero-cost nit — only worth fixing if the API already accepts it.\n\n**Verification:** Check `swissarmyhammer-config/src/lib.rs` or wherever `TemplateContext::set` is defined." #review-finding