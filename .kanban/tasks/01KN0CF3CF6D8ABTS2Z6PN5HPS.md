---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffff580
title: '[Info] scope_commands.rs is a new file — comprehensive and well-structured'
---
**File**: `swissarmyhammer-kanban/src/scope_commands.rs`\n\n**Observation**: This new file is the single source of truth for command resolution given a scope chain. It handles:\n- Entity schema-declared commands with proper template resolution\n- Registry (global/scoped) commands\n- Dynamic runtime commands (views, boards, windows)\n- Deduplication and availability filtering\n- Thorough test coverage with setup harness\n\nThe `TemplateParams` struct and `resolve_name_template` function cleanly separate template logic from command resolution. The `DynamicSources` pattern keeps Tauri-specific data out of the core resolution logic.\n\n**Severity**: Info (positive)\n**Layer**: Design/Architecture"