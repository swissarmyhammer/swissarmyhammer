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
license: MIT OR Apache-2.0
compatibility: Requires the `code_context` MCP tool for `lsp status` and `detect projects`. Also needs locally installed LSP servers (e.g. rust-analyzer, pyright, gopls, typescript-language-server) on the system PATH for the languages present in the workspace.
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

# LSP

Diagnose LSP server health and install missing servers for the `code_context` MCP tool. When live LSP ops (`get_hover`, `get_completions`, `go_to_definition`) return tree-sitter results instead of LSP, the most likely cause is a missing server.

## Process

### 1. Get status

```json
{"op": "lsp status"}
```

Returns:
- `languages[]`: `{icon, extensions, lsp_server, installed, install_hint}` (hint only when not installed)
- `all_healthy`: true when every detected language's server is installed

### 2. Present

One row per language:

| Icon | Server | Status | Install Command |
|------|--------|--------|-----------------|
| (icon) | rust-analyzer | Installed | — |
| (icon) | typescript-language-server | Missing | `npm install -g typescript-language-server` |

### 3. Act

**`all_healthy: true`** — report all good, no action.

**Servers missing**:
1. List the missing servers + install commands
2. Ask permission before installing
3. Run approved installs via `shell`
4. Re-run `lsp status` to confirm
5. Show updated table

### 4. Verify with a live op

Confirm end-to-end with a known symbol:

```json
{"op": "get symbol", "query": "main"}
```

LSP-sourced data confirms it works. Still degraded? The server may need a project restart or config (`compile_commands.json` for C/C++, `tsconfig.json` for TS).

### 5. Errors

- **Install fails**: report output; suggest manual install (different package manager, permissions, version).
- **No languages detected**: confirm source files exist; re-run after adding them.

## Troubleshooting

### `get_hover` / `get_definition` still return `source_layer: TreeSitter` after `installed: true`

The LSP process was already running (against the prior state) when the binary was installed, or the initial scan hasn't finished. Installs don't restart live sessions.

Restart the MCP server (or parent harness) so `sah` spawns a fresh LSP, then wait for the scan. Verify:

```json
{"op": "get hover", "file_path": "<known-file>", "line": 0, "character": 0}
```

A non-empty `contents` from the LSP layer = fixed.

### `clangd` (C/C++): no symbols or "Unable to handle compilation, expected compilation database"

`clangd` needs `compile_commands.json` at the workspace root (or a discoverable `build/`). Generate, then re-run `lsp status`:

- CMake: `cmake -S . -B build -DCMAKE_EXPORT_COMPILE_COMMANDS=ON && ln -sf build/compile_commands.json .`
- Make (Bear): `bear -- make`
- Meson: already emitted in the build dir — symlink to root

### `typescript-language-server` returns nothing in a monorepo

No (or wrong) `tsconfig.json` resolves for the file — common when each package has its own but the root doesn't. Add a root `tsconfig.json` with `"references"` to each package, or open the agent inside the package dir. Confirm:

```json
{"op": "get hover", "file_path": "packages/<pkg>/src/index.ts", "line": 0, "character": 0}
```

### Install succeeded but binary still not on `PATH`

Installed to a dir (`~/.cargo/bin`, `~/.npm-global/bin`, `~/go/bin`) that the MCP server's env doesn't see. Shell rc only affects interactive shells.

Export the directory in the environment that launches the agent (launchd on macOS, your service manager on Linux), then restart the MCP server. Confirm with `which <binary>` in that same environment.
