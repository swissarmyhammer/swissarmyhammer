---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffeb80
title: Missing MCP handlers for 5 of 10 new ops (get_definition, get_type_definition, get_hover, get_references, get_implementations, get_code_actions)
---
swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs\n\nThe dispatch match in `execute()` only has handlers for 4 of the 10 new ops:\n- get rename_edits (present)\n- get diagnostics (present)\n- get inbound_calls (present)\n- search workspace_symbol (present)\n\nThe other 6 new ops have library implementations in swissarmyhammer-code-context but no MCP dispatch:\n- get_definition\n- get_type_definition\n- get_hover\n- get_references\n- get_implementations\n- get_code_actions\n\nThey also have no Operation struct definitions in the MCP module and are not in the CODE_CONTEXT_OPERATIONS list. The explore skill references these ops (e.g., `get_definition`, `get_hover`, `get_references`) as if they are available, but they cannot be called through MCP.\n\nSuggestion: Add Operation structs, dispatch entries, and handler functions for all 6 missing ops. This is likely the next tranche of work, but the skill documentation should not reference unavailable operations." #review-finding