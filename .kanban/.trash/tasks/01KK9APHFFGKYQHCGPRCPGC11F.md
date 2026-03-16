---
position_column: done
position_ordinal: u6
title: 'CODE-CONTEXT-MGMT: Integrate doctor command into CLI routing'
---
Integrate code-context doctor reporting into the main CLI.

**Status:** 
- build_status and clear_status operations: ✅ Already implemented
- doctor MCP operation: ✅ Already implemented in code-context tool
- CLI routing: ❌ Not integrated

**What needs to happen:**
Wire doctor command into swissarmyhammer-cli routing:
1. Add \"doctor\" subcommand to CLI argument parser
2. Route "cargo run -- doctor" to doctor command handler  
3. Load project type from working directory
4. Call code-context doctor operation (or duplicate its logic)
5. Load LSP configs from builtin/lsp/ YAML (now done in FIX-3)
6. Execute check_command for each LSP
7. Format output human-readable with:
   - Project type detected
   - Each LSP status (installed/missing)
   - Install hints for missing LSPs

**Quality Test Criteria:**
1. cargo run -- doctor works without args
2. Detects project type (rust for swissarmyhammer-tools)
3. Shows available LSPs from loaded YAML
4. Runs check_command (which rust-analyzer)
5. Shows installed paths
6. Shows install hints for missing LSPs
7. Output is human-readable (not JSON)