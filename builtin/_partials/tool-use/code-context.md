---
title: Code Context
description: Code intelligence via the code_context tool
partial: true
---

## Code Context

`code_context` provides indexed, structural code intelligence — faster and more precise than text search for most coding tasks.

**Prefer it over file reads and grep when you need to:**

- **Find a symbol**: `{"op": "get symbol", "query": "MyStruct::new"}` — jumps to definition, fuzzy matched
- **See a file's structure**: `{"op": "list symbols", "file_path": "src/main.rs"}` — table of contents
- **Search symbols**: `{"op": "search symbol", "query": "handler", "kind": "function"}`
- **Pattern search**: `{"op": "grep code", "pattern": "unsafe\\s*\\{", "language": ["rs"]}`
- **Trace calls**: `{"op": "get callgraph", "symbol": "process_request", "direction": "inbound"}`
- **Assess impact**: `{"op": "get blastradius", "file_path": "src/server.rs", "max_hops": 3}`
- **Check index**: `{"op": "get status"}`

**Before modifying code**, run `get callgraph` (inbound) and `get blastradius` to see what depends on it.

**Before reading a file**, run `list symbols` to target specific symbols with `get symbol` instead of pulling the whole file into context.

**Fall back to raw text search** only for quick one-off matches where you already know the exact string.
