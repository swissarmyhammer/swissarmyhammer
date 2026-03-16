---
position_column: done
position_ordinal: s8
title: 'git tool: get diff with sem-core'
---
## What
Add `get diff` operation to the existing `git` MCP tool. Entity-level semantic diff using `sem-core`. Supports `left`/`right` as file paths, `file@ref` git versions, or inline text via `left_text`/`right_text`. Smart defaults when params omitted.

Files: `swissarmyhammer-tools/src/mcp/tools/git/diff/mod.rs`, `swissarmyhammer-tools/src/mcp/tools/git/mod.rs`

Spec: `ideas/code-context-architecture.md` — "git tool: expanded operations" section.

## Acceptance Criteria
- [ ] `get diff` operation registered on the `git` tool
- [ ] Parses `file@ref` syntax to extract file path + git ref
- [ ] Smart defaults: omit everything → dirty files vs HEAD; clean tree → branch vs parent
- [ ] `left_text`/`right_text` for inline code comparison (requires `language` param)
- [ ] Uses `sem-core` three-phase matching: exact ID → structural hash → fuzzy similarity
- [ ] Classifies entities: Added, Deleted, Modified, Renamed, Moved, Unchanged
- [ ] `include_callers` fans out via `code_context` call graph (optional, default true)
- [ ] Output includes before/after source text for modified entities

## Tests
- [ ] Unit test: diff file@HEAD vs working tree, detect modified function
- [ ] Unit test: function moved within file → classified as Moved, not Modified
- [ ] Unit test: `left_text` + `right_text` pure text comparison
- [ ] Unit test: smart default with no args on dirty tree diffs all dirty files
- [ ] Unit test: `language` required error when both sides are inline text without it
- [ ] `cargo test -p swissarmyhammer-tools`