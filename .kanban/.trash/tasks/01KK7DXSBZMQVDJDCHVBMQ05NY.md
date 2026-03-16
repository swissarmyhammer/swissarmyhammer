---
assignees:
- assistant
position_column: done
position_ordinal: s2
title: 'code_context tool: get symbol (fuzzy matching)'
---
Implement get_symbol operation with multi-tier fuzzy matching in swissarmyhammer-code-context/src/ops/get_symbol.rs. Returns full source text of a symbol by name, searching across the whole indexed codebase with exact, suffix, case-insensitive, and fuzzy matching tiers.