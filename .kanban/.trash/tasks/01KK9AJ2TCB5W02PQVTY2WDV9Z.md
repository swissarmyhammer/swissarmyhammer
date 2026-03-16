---
position_column: done
position_ordinal: t3
title: 'CODE-CONTEXT-A: Load LSP configs from YAML instead of hardcoded registry'
---
Make LSP configuration files the single source of truth instead of hardcoded `static SERVERS` array.

**Currently:** swissarmyhammer-lsp/src/registry.rs has hardcoded rust-analyzer only
**Should be:** Load from builtin/lsp/*.yaml files (rust-analyzer.yaml, pylsp.yaml, etc.)

**Requirements:**
- Create LSP config loader that reads YAML from builtin/lsp/ directory
- Parse YAML into LspServerSpec structs
- Replace `pub static SERVERS` with runtime-loaded config
- `servers_for_project()` loads from YAML, not static array
- Doctor, code-context, LSP startup all read from same YAML config

**Quality Test Criteria:**
1. Build succeeds: `cargo build 2>&1 | grep -c "error"` = 0
2. `servers_for_project(ProjectType::Rust)` returns rust-analyzer from YAML
3. `servers_for_project(ProjectType::NodeJs)` returns typescript-language-server from YAML
4. `servers_for_project(ProjectType::Python)` returns pylsp from YAML
5. `servers_for_project(ProjectType::Go)` returns gopls from YAML
6. All existing LSP tests pass
7. Doctor command reads same YAML config and validates LSPs
8. Integration test: `cargo run -- doctor` shows LSPs from YAML, not hardcoded