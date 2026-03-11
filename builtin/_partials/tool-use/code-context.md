---
title: Code Context
description: Code intelligence via the code_context tool
partial: true
---

## Code Context

Use the `code_context` tool for code navigation and understanding. It provides indexed, structural code intelligence that is faster and more precise than raw text search for most coding tasks.

**Prefer `code_context` over file reads and text search when you need to:**

- **Find a symbol**: `{"op": "get symbol", "query": "MyStruct::new"}` — jumps to definition with source text, multi-tier fuzzy matching
- **Explore a file's structure**: `{"op": "list symbols", "file_path": "src/main.rs"}` — table of contents before reading
- **Search symbols by name**: `{"op": "search symbol", "query": "handler", "kind": "function"}` — fuzzy search across the full index
- **Search code by pattern**: `{"op": "grep code", "pattern": "unsafe\\s*\\{", "language": ["rs"]}` — regex with language/path filters
- **Trace call chains**: `{"op": "get callgraph", "symbol": "process_request", "direction": "inbound"}` — who calls what
- **Assess change impact**: `{"op": "get blastradius", "file_path": "src/server.rs", "max_hops": 3}` — what could break
- **Check index health**: `{"op": "get status"}` — run first if unsure whether indexing is complete

**Before modifying code**, use `get callgraph` (inbound) and `get blastradius` to understand what depends on the code you're changing. This prevents accidental breakage.

**Before reading a file**, use `list symbols` to get a structural overview. This saves context by letting you target specific symbols with `get symbol` instead of reading entire files.

**Fall back to raw text search** only for quick one-off string matches where you already know the exact text and don't need structural understanding.
