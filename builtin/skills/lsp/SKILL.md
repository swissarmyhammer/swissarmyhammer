---
name: lsp
description: >-
  Diagnose and install missing LSP servers for your project. Use when the user
  says "lsp", "language servers", "check lsp", or wants to ensure code
  intelligence is fully working. Also use when live code intelligence ops
  (get_hover, get_completions, go to definition) return degraded results from
  the tree-sitter layer instead of LSP, or when you see "no code intelligence",
  "can't go to definition", "no type info available", or "source_layer:
  TreeSitter" on ops that should have full LSP data.
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

# LSP

Diagnose LSP server health for the current project and install any missing servers. When live LSP operations (e.g., `get_hover`, `get_completions`, `go_to_definition`) return results from the tree-sitter layer instead of a real LSP server, the most likely cause is a missing or uninstalled language server. This skill helps you find and fix that.

## Process

### 1. Get LSP status

Call the `code_context` tool to get LSP server status for all detected languages:

```json
{"op": "lsp status"}
```

This returns a JSON object with two fields:

- `languages`: array of objects, each with:
  - `icon`: language icon string (e.g. "\ue7a8" for Rust)
  - `extensions`: file extensions found in the index (e.g. `["rs"]`)
  - `lsp_server`: server command name (e.g. `"rust-analyzer"`)
  - `installed`: boolean, whether the server binary is on PATH
  - `install_hint`: string with the install command (only present when `installed` is false)
- `all_healthy`: boolean, true when every detected language has its LSP server installed

### 2. Present findings

Display a table with one row per language:

| Icon | Language Server | Status | Install Command |
|------|----------------|--------|-----------------|
| (icon) | rust-analyzer | Installed | -- |
| (icon) | typescript-language-server | Missing | `npm install -g typescript-language-server` |

### 3. Act on results

**If `all_healthy` is true**: Report that all LSP servers are installed and working. No action needed.

**If servers are missing**:
1. List the missing servers and their install commands
2. Ask the user for permission before running any install commands
3. Run each approved install command via the `shell` tool
4. After all installs complete, re-run `code_context` with `{"op": "lsp status"}` to confirm the fix
5. Present the updated table

### 4. Verify with a live op

After installing a server, confirm it is working end-to-end by trying a live LSP operation on a known file. For example, call `code_context` with a `get symbol` query for a symbol you know exists:

```json
{"op": "get symbol", "query": "main"}
```

If the result comes back with LSP-sourced data (not just tree-sitter), the server is working. If results are still degraded, the server may need a project restart or additional configuration (e.g., a `compile_commands.json` for C/C++, or a `tsconfig.json` for TypeScript).

### 5. Handle errors

- **Install command fails**: Report the error output and suggest the user install manually (e.g. different package manager, permissions issue, version conflict).
- **No languages detected**: Suggest the user check that the project has source files and re-run `code_context` with `{"op": "lsp status"}` after adding them.
