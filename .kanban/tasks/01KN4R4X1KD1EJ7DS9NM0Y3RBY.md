---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffa680
title: agent_source_for_directory uses fragile string matching for source classification
---
swissarmyhammer-agents/src/agent_resolver.rs:138-146\n\nThe `agent_source_for_directory` function uses `path_str.contains(\"/sah/agents\")` to determine if a directory is a User source. This is fragile: a project path like `/home/user/projects/sah/agents-tool/.agents` would match the `/sah/agents` substring and be misclassified as User.\n\nThe VFS already knows the FileSource for each search path. The SkillResolver handles this correctly via `get_search_paths()` which returns `SearchPath` entries with source metadata. The AgentResolver should use the same pattern instead of heuristic string matching.\n\nSuggestion: Use `vfs.get_search_paths()` (which returns `Vec<SearchPath>` with source metadata) instead of `vfs.get_directories()` + `agent_source_for_directory`. This mirrors the pattern already used in SkillResolver's `resolve_search_paths()` method. #review-finding