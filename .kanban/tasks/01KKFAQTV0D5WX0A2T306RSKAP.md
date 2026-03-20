---
position_column: done
position_ordinal: ffb080
title: '[nit] Duplicate detection logic between doctor.rs and detect.rs'
---
**Severity: nit**\n**File:** swissarmyhammer-tools/src/mcp/tools/code_context/doctor.rs:49-72 vs swissarmyhammer-project-detection/src/detect.rs\n\nThe `detect_project_types()` and `detect_project_type_enums()` functions in `doctor.rs` re-implement project detection from scratch with their own `KNOWN_PROJECT_TYPES` constant, while `swissarmyhammer-project-detection` already provides `detect_projects()`. The doctor module already calls `swissarmyhammer_lsp::servers_for_project()` which uses the proper types. The custom detection is unnecessary duplication and risks drifting out of sync with the canonical detection logic."