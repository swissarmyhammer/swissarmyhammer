---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffad80
title: AgentResolver precedence order changed from builtin<local<user to builtin<user<local
---
swissarmyhammer-agents/src/agent_resolver.rs:57-71\n\nThe old `resolve_all()` loaded in order: builtins, local paths, user paths -- meaning user-level agents had highest precedence (user overrode local overrode builtin).\n\nThe new implementation loads: builtins, VFS directories (user then local), extra paths -- meaning local now has highest precedence (local overrides user overrides builtin).\n\nThe new order (`builtin < user < local`) is actually the more conventional and correct precedence (project-local config should win). The doc comment was updated to reflect this. However, this is a behavioral change that could surprise existing users who relied on user-level agents taking precedence over project-local ones.\n\nSuggestion: This is likely the desired fix, but document it in release notes as a breaking change in agent override precedence. #review-finding