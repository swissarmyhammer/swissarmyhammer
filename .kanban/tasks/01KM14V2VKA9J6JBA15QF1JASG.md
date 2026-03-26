---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffef80
title: Add ToolFilter to ToolRegistry with YAML-backed persistence
---
## What

Add enable/disable filtering directly into `ToolRegistry` (not via the external HTTP proxy). The registry already holds all tools behind `Arc<RwLock<...>>` — adding a filter set here means `list_tools()` and `call_tool()` can check it inline without architectural changes.

**Config file:** `tools.yaml` loaded from the standard 3-layer stack:
- Global: `~/.sah/tools.yaml`
- Project: `.sah/tools.yaml`
- Project overrides global (later wins, same as modes/views)

**Format:**
```yaml
# tools.yaml — tool enable/disable configuration
# Unlisted tools default to enabled.
# "enabled: false" disables a tool. "enabled: true" re-enables.
shell:
  enabled: true
kanban:
  enabled: false
```

**Implementation in `swissarmyhammer-tools/src/mcp/tool_registry.rs`:**
- Add `disabled_tools: HashSet<String>` field to `ToolRegistry`
- `list_tools()` filters out disabled tools
- `get_tool()` returns `None` for disabled tools (blocks execution)
- Add `set_tool_enabled(name, bool)`, `is_tool_enabled(name) -> bool`, `set_all_enabled(bool)` methods
- Add `load_tool_config(path)` and `save_tool_config(path)` for YAML persistence
- On server startup, load from disk and apply

**Files:**
- `swissarmyhammer-tools/src/mcp/tool_registry.rs` — add filter state + methods
- New: `swissarmyhammer-tools/src/mcp/tool_config.rs` — YAML serialization/deserialization, 3-layer merge logic

## Acceptance Criteria
- [ ] `ToolRegistry::list_tools()` excludes disabled tools
- [ ] `ToolRegistry::get_tool()` returns None for disabled tools
- [ ] `set_tool_enabled` / `set_all_enabled` mutate the disabled set
- [ ] YAML config loads from `~/.sah/tools.yaml` and `.sah/tools.yaml` with merge
- [ ] Config round-trips correctly (load → save → load yields same state)
- [ ] Unlisted tools default to enabled

## Tests
- [ ] Unit test: `set_tool_enabled(\"shell\", false)` → `list_tools()` excludes shell, `get_tool(\"shell\")` returns None
- [ ] Unit test: `set_all_enabled(false)` → all tools disabled, `set_tool_enabled(\"shell\", true)` → only shell enabled
- [ ] Unit test: YAML round-trip (serialize → deserialize → assert equal)
- [ ] Unit test: 3-layer merge (global disables shell, project enables shell → shell is enabled)
- [ ] `cargo nextest run -p swissarmyhammer-tools` #tool-filter