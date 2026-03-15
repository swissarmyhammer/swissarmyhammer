---
position_column: done
position_ordinal: t1
title: 'CODE-CONTEXT-FIX-3: Load LSP servers from YAML instead of hardcoded registry'
---
User explicitly called this out: "so this config isn't just for doctor -- it's to know which LSPs are available" and "i asked you to figure what you missed from the spec and you MISSED THIS".

**Current state:**
- builtin/lsp/ has 4 YAML files created (rust-analyzer.yaml, typescript-language-server.yaml, pylsp.yaml, gopls.yaml)
- swissarmyhammer-lsp/src/registry.rs hardcodes ONLY rust-analyzer
- YAML files are never loaded or parsed
- Dead code

**What needs to happen:**
Option A: Load YAML at runtime
- Create function to load YAML files from builtin/lsp/
- Parse each YAML into LspServerSpec struct
- Use loaded specs instead of hardcoded SERVERS array

Option B: Generate Rust code from YAML at build time
- Write build.rs script to read YAML files at compile time
- Generate registry.rs with all specs built in
- Users can update YAML without code changes

**Choose one approach and implement it.**

**Why this matters:** Spec says "users never write config files" but also says the registry should be extensible. YAML makes sense for this. Currently can't add Node.js/Python/Go LSP support without code changes.

**Quality Test Criteria:**
- cargo build succeeds
- Unit test: load all YAML files, verify they parse correctly
- Unit test: servers_for_project(ProjectType::NodeJs) returns typescript-language-server
- Unit test: servers_for_project(ProjectType::Python) returns pylsp
- Unit test: servers_for_project(ProjectType::Go) returns gopls
- All 4 LSP servers appear in registry after load