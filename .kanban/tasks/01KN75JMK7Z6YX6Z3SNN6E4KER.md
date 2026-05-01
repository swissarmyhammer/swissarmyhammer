---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffa080
title: Test project detection for npm workspaces and TOML array parsing
---
File: swissarmyhammer-project-detection/src/detect.rs (79.5%, 24 uncovered lines)\n\nUncovered paths:\n- `detect_npm_workspace()` (lines 228-258): npm workspaces parsing - array form, string form, missing workspaces key\n- `extract_toml_array()` (lines 262-300): multi-line TOML array parsing, items on same line as opening bracket, closing bracket with items\n- `clean_toml_string()` (lines 303-314): quote stripping, empty after cleaning\n- Various single-line gaps in main detect function: specific project type detection branches\n\nTests needed:\n- Unit test detect_npm_workspace with workspaces as array, string, absent\n- Unit test extract_toml_array with inline array, multi-line array\n- Unit test clean_toml_string edge cases\n\nAcceptance: coverage >= 85% for detect.rs #coverage-gap