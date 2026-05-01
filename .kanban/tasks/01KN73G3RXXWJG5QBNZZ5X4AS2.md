---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffa680
title: Add tests for ModelManager::use_agent and validate_agent_name_security
---
swissarmyhammer-config/src/model.rs:1781-1864\n\nUncovered lines: 1794, 1800, 1810-1812, 1819, 1821, 1825-1828, 1834-1836, 1844, 1858-1860\n\n```rust\npub fn use_agent(agent_name: &str, paths: &ModelPaths) -> Result<(), ModelError>\nfn validate_agent_name_security(agent_name: &str) -> Result<(), ModelError>\n```\n\nuse_agent sets the active agent in config. validate_agent_name_security checks for empty names, length overflow, null bytes, path traversal, and control characters. Test each validation branch: empty name, overly long name, name with null bytes, name with '../', name with control chars, and a valid name that writes config. #Coverage_Gap