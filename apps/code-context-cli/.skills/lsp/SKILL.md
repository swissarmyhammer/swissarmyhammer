---
name: lsp
description: 'Diagnose and install missing LSP servers for your project. Use when the user says "lsp", "language servers", "check lsp", or wants to ensure code intelligence is fully working. Also use when live code intelligence ops (get_hover, get_completions, go to definition) return degraded results from the tree-sitter layer instead of LSP, or when you see "no code intelligence", "can''t go to definition", "no type info available", or "source_layer: TreeSitter" on ops that should have full LSP data.'
license: MIT OR Apache-2.0
compatibility: Requires the `code_context` MCP tool for `lsp status` and `detect projects`. Also needs locally installed LSP servers (e.g. rust-analyzer, pyright, gopls, typescript-language-server) on the system PATH for the languages present in the workspace.
metadata:
  version: 0.12.11
  author: swissarmyhammer
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

## Troubleshooting

### Error: `get_hover` / `get_definition` still return `source_layer: TreeSitter` after the server reports `installed: true`

- **Cause**: The LSP process was already running (against the previous state of the workspace) when the server binary was installed, or the server has not finished its initial project scan. Installation does not restart live sessions.
- **Solution**: Restart the MCP server (or the parent agent harness) so `sah` spawns a fresh LSP process, then wait for the initial scan. Verify with:
  ```json
  {"op": "get hover", "file_path": "<file-you-know>", "line": 0, "character": 0}
  ```
  A successful fix returns a non-empty `contents` field sourced from the LSP layer.

### Error: `clangd` (C/C++) reports no symbols or "Unable to handle compilation, expected compilation database"

- **Cause**: `clangd` needs a `compile_commands.json` at the workspace root (or in a `build/` directory it can find) to know include paths and compiler flags. Without it, it falls back to tree-sitter-only behavior.
- **Solution**: Generate one from your build system and re-run `lsp status`:
  - CMake: `cmake -S . -B build -DCMAKE_EXPORT_COMPILE_COMMANDS=ON && ln -sf build/compile_commands.json .`
  - Bear (Make-based builds): `bear -- make`
  - Meson: already emitted in the build directory — symlink it to the root.

### Error: `typescript-language-server` returns no completions or types in a monorepo

- **Cause**: No `tsconfig.json` (or the wrong one) resolves for the file — the server cannot determine module resolution, target, or paths. Common in monorepos where each package has its own `tsconfig.json` but the root does not.
- **Solution**: Add a root `tsconfig.json` with `"references"` to each package's config, or open the agent from inside the specific package directory. Confirm with:
  ```json
  {"op": "get hover", "file_path": "packages/<pkg>/src/index.ts", "line": 0, "character": 0}
  ```

### Error: `install command` succeeds but the binary is still not on `PATH`

- **Cause**: The install placed the binary in a directory (e.g. `~/.cargo/bin`, `~/.npm-global/bin`, `~/go/bin`) that the MCP server's environment has not picked up. Shell `rc` files only affect interactive shells.
- **Solution**: Export the directory in the environment that launches the agent (e.g. add it to `launchd` env on macOS or your service manager on Linux), then restart the MCP server. Verify with `which <server-binary>` in the same environment that launches `sah`.
