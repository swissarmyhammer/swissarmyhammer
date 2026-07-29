---
name: lsp
description: >-
  Diagnose LSP servers that your project is missing, and install them. Use
  this skill when the user says "lsp", "language servers", "check lsp", or
  wants to make sure code intelligence works fully. Also use it when a live
  code intelligence op (get_hover, get_completions, go to definition) returns
  a weaker result from the tree-sitter layer instead of from LSP, or when you
  see "no code intelligence", "can't go to definition", "no type info
  available", or "source_layer: TreeSitter" on an op that should return full
  LSP data.
license: MIT OR Apache-2.0
compatibility: This skill needs the `code_context` MCP tool, for `lsp status` and `detect projects`. It also needs LSP servers installed locally, for example rust-analyzer, pyright, gopls, or typescript-language-server, on the system PATH, for the languages present in the workspace.
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

# LSP

Diagnose the health of LSP servers, and install missing servers, for the `code_context` MCP tool. When a live LSP op, such as `get_hover`, `get_completions`, or `go_to_definition`, returns a tree-sitter result instead of an LSP result, the most likely cause is a missing server.

## Process

### 1. Get status

```json
{"op": "lsp status"}
```

Returns:
- `languages[]`: `{icon, extensions, lsp_server, installed, install_hint}` (hint only when not installed)
- `all_healthy`: true when the server for every detected language is installed

### 2. Present

One row per language:

| Icon | Server | Status | Install Command |
|------|--------|--------|-----------------|
| (icon) | rust-analyzer | Installed | — |
| (icon) | typescript-language-server | Missing | `npm install -g typescript-language-server` |

### 3. Act

**`all_healthy: true`** — report that everything is fine. Take no action.

**Servers missing**:
1. List the missing servers and their install commands
2. Ask permission before installing
3. Run approved installs via `shell`
4. Run `lsp status` again to confirm
5. Show the updated table

### 4. Verify with a live op

Confirm the fix from end to end, with a known symbol:

```json
{"op": "get symbol", "query": "main"}
```

Data from LSP confirms that it works. Is the result still degraded? The server may need a project restart, or config, for example `compile_commands.json` for C or C++, or `tsconfig.json` for TypeScript.

### 5. Errors

- **Install fails**: report the output. Suggest a manual install, for example with a different package manager, with different permissions, or a different version.
- **No languages detected**: confirm that source files exist. Run the check again after you add them.

## Troubleshooting

### `get_hover` or `get_definition` still return `source_layer: TreeSitter` after `installed: true`

The LSP process was already running, against the earlier state, when you installed the binary. Or the initial scan has not finished yet. An install does not restart a live session.

Restart the MCP server, or the parent harness, so that `sah` starts a fresh LSP. Then wait for the scan to finish. Verify:

```json
{"op": "get hover", "file_path": "<known-file>", "line": 0, "character": 0}
```

A non-empty `contents` field from the LSP layer means the problem is fixed.

### `clangd` (C or C++): no symbols, or "Unable to handle compilation, expected compilation database"

`clangd` needs `compile_commands.json` at the workspace root, or in a `build/` directory it can find. Generate this file, then run `lsp status` again:

- CMake: `cmake -S . -B build -DCMAKE_EXPORT_COMPILE_COMMANDS=ON && ln -sf build/compile_commands.json .`
- Make (Bear): `bear -- make`
- Meson: this file is already produced in the build directory; make a symbolic link to it from the root

### `typescript-language-server` returns nothing in a monorepo

No `tsconfig.json` resolves for the file, or the wrong one does. This is common when each package has its own `tsconfig.json`, but the root does not. Add a root `tsconfig.json` with `"references"` to each package, or open the agent inside the package directory. Confirm:

```json
{"op": "get hover", "file_path": "packages/<pkg>/src/index.ts", "line": 0, "character": 0}
```

### Install succeeded but binary still not on `PATH`

The install placed the binary in a directory, for example `~/.cargo/bin`, `~/.npm-global/bin`, or `~/go/bin`, that the environment of the MCP server does not see. A shell rc file only affects interactive shells.

Export the directory in the environment that launches the agent, for example launchd on macOS, or your service manager on Linux. Then restart the MCP server. Confirm with `which <binary>` in that same environment.
