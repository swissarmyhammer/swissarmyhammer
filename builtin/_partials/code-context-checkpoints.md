---
title: Code-Context Checkpoints
description: Required code_context checkpoints shared across exploring, implementing, testing, and reviewing code
partial: true
---

## Code-Context Checkpoints

The `code_context` tool is structural code intelligence — indexed symbol lookup,
call graphs, and blast-radius analysis backed by tree-sitter and live LSP. It is
not optional background. Treat the checkpoints below as gates: hitting them is
part of doing the task, not extra work on top of it.

Do not read files top to bottom, and do not guess where a symbol lives or who
calls it. `code_context` answers those questions precisely and cheaply.

- **Before reading a file** — `{"op": "list symbols", "file_path": "<file>"}` for a
  table of contents, then `{"op": "get symbol", "query": "<symbol>"}` to pull only
  the code you need. Reading a whole file is the fallback, not the default.
- **Before changing a symbol** — `{"op": "get blastradius", "file_path": "<file>"}`
  and `{"op": "get callgraph", "symbol": "<symbol>", "direction": "inbound"}`. If the
  result surprises you, you do not yet understand the change well enough to make it.
- **After changing a signature or behavior** — re-check the inbound callers the
  blast radius surfaced, and confirm each one still holds.
- **When a test or build fails** — `{"op": "get callgraph", "symbol": "<failing
  symbol>"}` to see what the failure actually reaches before you start fixing it.
- **To find code by name or pattern** — `search symbol` / `grep code` instead of
  raw text search; they query the index, with kind and language filters.

If `{"op": "get status"}` shows indexing incomplete, the live LSP ops
(`get definition`, `get hover`, `get references`, `search workspace_symbol`) still
work immediately — do not wait on the index. If callgraph or blast radius comes
back empty for code that clearly compiles, the language server is missing or
warming up: check `{"op": "lsp status"}` and invoke `/lsp` if needed.

Fall back to raw Read/Grep/Glob only for non-code files (TOML, YAML, Markdown),
string literals and config values not in the symbol index, or confirming exact
syntax once code_context has already given you the location.
