---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffc680
title: 'warning: KNOWN_TOOLS static list will drift from actual registered tools'
---
swissarmyhammer-cli/src/commands/tools/mod.rs:12-23\n\n`KNOWN_TOOLS` is a hardcoded `&[&str]` of tool names. This list must be kept in sync manually with the tools actually registered in `tool_registry.rs`. When a new tool is added to the registry (or an existing one is renamed/removed), `KNOWN_TOOLS` will silently go out of sync, causing:\n1. `sah tools list` to show stale names\n2. `validate_tool_names` to reject valid tool names\n3. `handle_disable` (disable all) to miss new tools\n\nThere is currently no compile-time or test-time guard that catches this drift.\n\nSuggestion: Expose a `known_tool_names() -> &[&str]` function from `swissarmyhammer-tools` that returns the same slice the registry would produce (e.g., from an enum or associated const on the registration functions), or add an integration test that asserts `KNOWN_TOOLS` is a subset of the registered tool names.\n\nVerification: Add a test that builds a real registry and asserts every name in `KNOWN_TOOLS` is present.\n\n#review-finding #review-finding