---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffaa80
title: 'Dead code: agent_source_from_file_source defined in tests but never used by production code'
---
swissarmyhammer-agents/src/agent_resolver.rs:201-207\n\nThe test helper `agent_source_from_file_source` maps `FileSource` to `AgentSource`. It is tested but never called by production code. Meanwhile, the production code uses `agent_source_for_directory` (string heuristic) instead.\n\nThis suggests the VFS migration was intended to use the FileSource metadata but the implementation took a different path. Either the function should be promoted to production code (replacing the string heuristic), or it should be removed if it is truly dead.\n\nSuggestion: If Finding #1 (string matching) is fixed to use `get_search_paths()`, this mapping function becomes the natural production implementation. Otherwise remove it. #review-finding