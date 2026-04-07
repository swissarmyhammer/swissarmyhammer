---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffef80
title: Explore skill references ops that are not yet wired to MCP dispatch
---
builtin/skills/explore/SKILL.md\n\nThe explore skill references these operations as available:\n- `get_definition` (line ~78)\n- `get_hover` (line ~82)\n- `get_inbound_calls` (line ~92)\n- `get_references` (line ~98)\n- `workspace_symbol_live` (line ~58)\n\nOf these, only `get inbound_calls` and `search workspace_symbol` are wired in the MCP dispatch. The others (get_definition, get_hover, get_references) have no MCP handlers. An agent following the explore skill will get 'Unknown operation' errors when trying these ops.\n\nAdditionally, the skill uses underscore-separated op names (`get_definition`, `get_hover`) but the MCP dispatch convention uses space-separated names (`get symbol`, `get diagnostics`). Even once wired, the op names won't match.\n\nSuggestion: Either defer updating the explore skill until all ops are wired, or add a note that these ops are 'coming soon'. Fix the naming convention to match whichever format the dispatch will use." #review-finding