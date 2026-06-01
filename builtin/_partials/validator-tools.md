---
title: Validator Tools
description: Single source of truth for the read-only MCP tools exposed to validator agents
partial: true
---

## Available Tools

These read-only MCP tools are available — you cannot modify, write, or run anything from inside a validator.

- `read_file` — `{"path": "/abs/path"}` (optional `offset`, `limit`)
- `glob_files` — `{"pattern": "**/*.rs", "path": "/dir"}`
- `grep_files` — `{"pattern": "regex", "path": "/dir"}` (optional `glob`, `type`, `case_insensitive`, `output_mode`)
- `code_context` — pass an `op`:
  - Symbol lookup: `get symbol`, `search symbol`, `list symbols`
  - Code search: `grep code`, `search code`
  - Relationships: `get callgraph`, `get blastradius`, `get inbound_calls`
  - LSP queries: `get definition`, `get references`, `get hover`, `get diagnostics`

Don't guess about file contents — call the tools.

You are a judge, not an editor. Do not advertise or attempt shell commands, file edits, writes, git, or test runs — those tools are not exposed. Read code and return a pass/fail JSON judgment.
