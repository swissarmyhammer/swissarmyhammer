---
name: lsp
description: Diagnose and install missing LSP servers for your project. Use when the user says "lsp", "language servers", "check lsp", or wants to ensure code intelligence is fully working.
metadata:
  author: "swissarmyhammer"
  version: "1.0"
---

# LSP

Diagnose LSP server health for the current project and install any missing servers.

## Process

### 1. Check index readiness

Before querying LSP status, confirm the code-context index is populated. Call the `code_context` tool:

```json
{"op": "get status"}
```

If the index has zero files or is still building, tell the user to wait for indexing to complete and stop here.

### 2. Get LSP status

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

### 3. Present findings

Display a table with one row per language:

| Icon | Language Server | Status | Install Command |
|------|----------------|--------|-----------------|
| (icon) | rust-analyzer | Installed | -- |
| (icon) | typescript-language-server | Missing | `npm install -g typescript-language-server` |

### 4. Act on results

**If `all_healthy` is true**: Report that all LSP servers are installed and working. No action needed.

**If servers are missing**:
1. List the missing servers and their install commands
2. Ask the user for permission before running any install commands
3. Run each approved install command via the `shell` tool
4. After all installs complete, re-run `code_context` with `{"op": "lsp status"}` to confirm the fix
5. Present the updated table

### 5. Handle errors

- **Install command fails**: Report the error output and suggest the user install manually (e.g. different package manager, permissions issue, version conflict).
- **No languages detected**: The index may be empty. Suggest the user check that the project has source files and that indexing has completed (`code_context` with `{"op": "get status"}`).
