---
name: explore
profiles:
  - code-context
description: Understand how unfamiliar code works before planning or changing it — its structure, behavior, data flow, and the blast radius of a change. Use when the user says "explore", "investigate", "how does X work", "why does X happen", "where is X handled", "what calls X", "what would it take to change X", or whenever you need to understand code before acting on it. Drives exploration with the code_context MCP tool — symbol search, callgraph traversal, and blast-radius analysis — instead of reading files top to bottom.
license: MIT OR Apache-2.0
compatibility: Requires the `code_context` MCP tool for symbol search, callgraph traversal, and blast-radius analysis during exploration.
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

{% include "_partials/code-context-checkpoints" %}
{% include "_partials/architecture-awareness" %}

# Explore

Understand code well enough to explain how it works and what a change would touch.

{% if arguments %}
## Focus

> {{arguments}}
{% endif %}

Use `code_context` to find the *right* code fast, trace connections, and measure blast radius — not linear reading.

## Done Means

Exploration is complete when you can explain:

```
1. HOW IT WORKS    — what calls what, what data flows where
2. WHERE IT LIVES  — specific files and symbols
3. WHAT IT TOUCHES — blast radius: what a change would affect
```

Can't state all three? Not done. Guessing at any? Back to the tool — don't fill the gap with assumptions.

## Process

### 1. Orient — check layers

```json
{"op": "get status"}
```

Note which layers are active. Live LSP ops (`get definition`, `get hover`, `search workspace_symbol`) work immediately — don't wait for indexing. If LSP unavailable, results come from tree-sitter. Check `lsp status` to see per-language coverage.

If `ARCHITECTURE.md` exists at the project root, read it now (per the Architecture Awareness guidance) — it gives the system map before tracing individual symbols.

### 2. Survey — find the territory

Broad first. Use domain keywords:

```json
{"op": "search symbol", "query": "<domain keyword>", "max_results": 15}
```

If the index is building and `search symbol` is sparse, use the live alternative:

```json
{"op": "search workspace_symbol", "query": "<domain keyword>"}
{"op": "list symbols", "file_path": "<key file>"}
```

**Looking for**: the nouns and verbs of the problem — structs, traits, functions that participate.

### 3. Trace — follow execution

```json
{"op": "get symbol", "query": "<specific symbol>"}
```

Jump to definitions and types without reading whole files:

```json
{"op": "get definition", "file_path": "<file>", "line": <line>, "character": <col>}
{"op": "get hover", "file_path": "<file>", "line": <line>, "character": <col>}
```

Call relationships both directions:

```json
{"op": "get callgraph", "symbol": "<symbol>", "direction": "both", "max_depth": 2}
{"op": "get inbound_calls", "file_path": "<file>", "line": <line>, "character": <col>}
```

All usages:

```json
{"op": "get references", "file_path": "<file>", "line": <line>, "character": <col>}
```

**Looking for**: the path data takes through the system. `get inbound_calls` is live LSP precision for "who calls this"; `get callgraph` uses indexed edges for broader traversal.

### 4. Scope — measure the blast radius

```json
{"op": "get blastradius", "file_path": "<target>", "max_hops": 3}
```

Supplement with `get references` — blast radius follows call edges, but references also catch type usage, field access, and trait impls.

**Looking for**: how far a change propagates. If the radius surprises you, you don't understand the code yet — back to step 3.

### 5. Check tests

Tests are the clearest executable spec — they confirm understanding and show project patterns.

```json
{"op": "grep code", "pattern": "<symbol or behavior>", "file_pattern": "test"}
```

Also use Glob/Grep for test files near the code:
- Same dir with `_test` suffix
- `tests/` at project/crate root
- Inline test modules (`#[cfg(test)]`, `describe(`, `#[test]`)

**Looking for**: intended behavior, the project's test patterns, behavior with no coverage.

### 6. Conclude — explain

Exit gate. State concretely:

```
HOW IT WORKS: <mechanism in plain terms — what calls what, what flows where>
KEY CODE:     <files and symbols — paths>
BLAST RADIUS: <what a change touches, or "n/a — investigation only">
```

Then point at the next step — but don't take it. Exploration produces understanding; acting is separate:

- **Make a change** → `/tdd` (failing test first) or `/implement`
- **Too large for one step** → `/plan`
- **Found a bug** → describe it + expected behavior, suggest `/task`
- **Architectural question** → present findings, ask the user — don't guess

## Layered Resolution

`code-context` is primary. Indexed ops (tree-sitter symbols, callgraphs, blast radius) plus **live LSP ops** (definitions, hover, references, inbound calls, workspace symbol search). Live ops work before the index is fully built.

Results include `source_layer`:
- **lsp** — full language-server precision (types, generics, trait impls)
- **treesitter** — structural parsing from the index (fast, always available after indexing)
- **treesitter+lsp** — combined

Tree-sitter-only for a language that should have LSP? Suggest `/lsp`.

Use raw Read/Grep/Glob only for:
- String literals, config, error messages not in the symbol index
- Non-code files (TOML, YAML, JSON, Markdown)
- Confirming exact syntax after code-context gave you the location

**Don't** start by reading files top to bottom. Start with `search symbol` (or `search workspace_symbol` while indexing) and `get callgraph`; use `get definition`/`get hover` to inspect specifics.

## When to Recurse

If blast radius reveals surprises or the callgraph leads to new territory, loop back to step 2 with new keywords. Each loop should *narrow* focus, not widen it.

## Examples

**Understanding a feature:** User says "explore how the kanban watcher decides which files to re-index".

1. Orient with `get status` — note active layers.
2. Survey: `search symbol "watcher"`, `search symbol "invalidate"` → `KanbanWatcher::on_event`, `invalidate_file`.
3. Trace: `get symbol "KanbanWatcher::on_event"`, then `get callgraph "invalidate_file"` inbound, depth 2.
4. Scope: `get blastradius "src/watcher.rs" max_hops 3` → indexer + MCP layer only.
5. Tests: `grep code "on_event"` in `test` → smoke test covers creation; nothing covers deletion.
6. Conclude:

   ```
   HOW IT WORKS: on_event receives a FileEvent, matches the EventKind, and calls
                 invalidate_file for created/modified files; the indexer picks up
                 invalidated paths on its next pass.
   KEY CODE:     src/watcher.rs (KanbanWatcher::on_event, invalidate_file)
   BLAST RADIUS: indexer + MCP layer only. Deletion (EventKind::Remove) is not
                 handled — invalidate_file is never called for deleted files.
   ```

Exploration complete. Deletion gap → `/tdd` or `/task`.

**Exploration reveals work too large:** `/explore what it would take to add SSO`. Orient, survey auth symbols, trace login flow. Blast radius on `src/auth/login.rs` shows 40+ call sites. Stop — escalate to `/plan` rather than force a conclusion.

## Constraints

- **Don't write code during exploration.** Hand off.
- **Don't skip blast radius.** It's where surprises surface.
- **Don't read files top to bottom.** Use `code_context` to find the right code, inspect what matters.
- **Don't explore forever.** 3 loops without convergence → stop, say what's unclear, ask the user.
- **Don't use exploration to avoid acting.** Once you can explain how/where/what-it-touches, move to planning or implementation.
