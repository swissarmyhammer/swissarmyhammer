---
depends_on:
- 01KKCADZV71JJH01V5GVEPQTAX
position_column: done
position_ordinal: ffffffffffc680
title: 'STATUSLINE-M15: languages module'
---
## What
Implement the `languages` module that shows detected project language icons with LSP availability status.

**File**: `swissarmyhammer-statusline/src/modules/languages.rs`

**Source data**:
- `swissarmyhammer-code-context` — query `indexed_files` table for file extensions
- `swissarmyhammer-treesitter` — `LanguageRegistry::global()` for extension→language mapping
- `swissarmyhammer-code-context::lsp_server::find_executable()` for LSP availability

**Default format**: `$icons`

**Config**:
```yaml
languages:
  style: "bold"
  dim_without_lsp: true
  format: "$icons"
```

**Language icon map** (starship-style):
| Language | Icon | LSP server checked |
|----------|------|-------------------|
| rust | 🦀 | `rust-analyzer` |
| python | 🐍 | `pyright`, `pylsp` |
| typescript/tsx | 📜 | `typescript-language-server` |
| javascript | 📜 | `typescript-language-server` |
| go | 🐹 | `gopls` |
| java | ☕ | `jdtls` |
| ruby | 💎 | `solargraph` |
| swift | 🐦 | `sourcekit-lsp` |
| c/cpp | ⚙️ | `clangd` |
| dart | 🎯 | `dart` |

**Logic**:
1. Open code-context workspace as Reader
2. Query distinct extensions from `indexed_files`
3. Map extensions to languages via `LanguageRegistry`
4. Deduplicate (typescript + tsx = one icon)
5. For each language, check if LSP server is in PATH via `find_executable()`
6. Render icons: bright when LSP available, dim when not (if `dim_without_lsp: true`)

**Variables**: `$icons` (space-separated language emoji string)

**Example output**: `🦀 🐍 📜` (rust has LSP, python dimmed if no pyright)

## Acceptance Criteria
- [ ] Uses library APIs (code-context, treesitter, find_executable), NOT shell commands
- [ ] Detects languages from indexed_files extensions
- [ ] Maps extensions to emoji icons
- [ ] Checks LSP availability per language
- [ ] Dims icons when LSP not found (configurable)
- [ ] Hidden when no code-context workspace or no files indexed
- [ ] Format string supports `$icons` variable

## Tests
- [ ] Unit test: extension-to-icon mapping for known languages
- [ ] Unit test: deduplication (ts + tsx = one icon)
- [ ] Unit test: dim styling applied when LSP not found
- [ ] `cargo test -p swissarmyhammer-statusline` passes #statusline