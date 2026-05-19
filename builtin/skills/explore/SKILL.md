---
name: explore
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

## Why This Skill Exists

The gap between "I don't understand this code" and "I know what to do" is where most bad decisions happen. Claude's default behavior is to read a couple of files top to bottom, grep around, and jump straight to acting. That misses how the code actually connects — and what a change would ripple into.

This skill enforces a structured path through that gap. It uses `code_context` as the primary tool so you find the *right* code fast, trace how it connects, and measure the blast radius — instead of reading files linearly and hoping you saw the important parts.

## What "Done" Means

Exploration is complete when you can explain, concretely:

```
1. HOW IT WORKS    — the mechanism: what calls what, what data flows where
2. WHERE IT LIVES  — the specific files and symbols that participate
3. WHAT IT TOUCHES — the blast radius: what a change here would affect
```

If you can't state all three, you're not done exploring. If you're guessing at any of them, go back to the tool — don't fill the gap with assumptions.

## Process

### 1. Orient — check available layers

Always start here. `code_context` returns results from multiple layers (tree-sitter index, live LSP, or both). Exploration works immediately with whatever layers are available.

```json
{"op": "get status"}
```

Note which layers are active. If tree-sitter indexing is still in progress, live LSP ops (`get definition`, `get hover`, `search workspace_symbol`) work immediately — don't wait. If LSP is unavailable for a language, results come from tree-sitter only. Check `lsp status` if you need to know which languages have LSP support.

If an `ARCHITECTURE.md` exists at the project root, read it now — per the **Architecture Awareness** guidance above, it gives you the system map before you start tracing individual symbols, so you can place what you find inside the documented structure.

### 2. Survey — find the territory

Start broad. Use domain keywords from the user's question to find relevant symbols.

```json
{"op": "search symbol", "query": "<domain keyword>", "max_results": 15}
```

If the index is still building and `search symbol` returns sparse results, use the live alternative:

```json
{"op": "search workspace_symbol", "query": "<domain keyword>"}
```

```json
{"op": "list symbols", "file_path": "<key file>"}
```

**What you're looking for**: the nouns and verbs of the problem. Structs, traits, functions that participate in the behavior you're investigating.

### 3. Trace — follow the execution

Once you've found the key symbols, trace how they connect.

```json
{"op": "get symbol", "query": "<specific symbol>"}
```

Jump to definitions and inspect types without reading entire files:

```json
{"op": "get definition", "file_path": "<file>", "line": <line>, "character": <col>}
```

```json
{"op": "get hover", "file_path": "<file>", "line": <line>, "character": <col>}
```

Trace call relationships in both directions:

```json
{"op": "get callgraph", "symbol": "<symbol>", "direction": "both", "max_depth": 2}
```

```json
{"op": "get inbound_calls", "file_path": "<file>", "line": <line>, "character": <col>}
```

Find every usage of a symbol across the codebase:

```json
{"op": "get references", "file_path": "<file>", "line": <line>, "character": <col>}
```

**What you're looking for**: the path data takes through the system. Who calls what, what depends on what, where the boundaries are. `get inbound_calls` gives you live LSP precision for "who calls this function?", while `get callgraph` uses the indexed call edges for broader traversal.

### 4. Scope — measure the blast radius

Before forming a conclusion about how a change would land, understand what it would touch.

```json
{"op": "get blastradius", "file_path": "<target file>", "max_hops": 3}
```

Supplement blast radius with reference search — blast radius follows call edges, but `get references` also catches type usage, field access, and trait implementations:

```json
{"op": "get references", "file_path": "<file>", "line": <line>, "character": <col>}
```

**What you're looking for**: how far a change propagates. If the blast radius surprises you, you don't understand the code well enough yet — go back to step 3.

### 5. Check how it's tested

Find how the code is already exercised. Tests are the clearest executable description of intended behavior — reading them confirms (or corrects) your understanding, and shows you the patterns and conventions the project relies on.

```json
{"op": "grep code", "pattern": "<symbol or behavior under investigation>", "file_pattern": "test"}
```

Also use Glob/Grep to find test files near the code you're exploring:
- Same directory with a `_test` suffix
- `tests/` directory at project or crate root
- Test modules inside source files (`#[cfg(test)]`, `describe(`, `#[test]`)

**What you're looking for**: confirmation of how the code is *meant* to behave, the test patterns the project follows, and any behavior that has no coverage.

### 6. Conclude — explain what you found

This is the exit gate. State your finding as a concrete explanation:

```
HOW IT WORKS: <the mechanism, in plain terms — what calls what, what flows where>
KEY CODE:     <the files and symbols that matter — file paths>
BLAST RADIUS: <what a change here would touch, or "n/a — investigation only">
```

Then point at the next step — but don't take it yourself. Exploration produces understanding; acting on it is a separate move:

- **To make a change** — hand off to `/tdd` (write the failing test first) or `/implement`.
- **Change too large for one step** — hand off to `/plan` to break it into tasks.
- **Found a bug** — describe it and the behavior that should hold, and suggest `/task` to track it.
- **An architectural question** — present what you found and ask the user; don't guess.

## Using code-context — layered resolution

**code-context is the primary exploration tool.** It provides both **indexed ops** (tree-sitter symbols, call graphs, blast radius) and **live LSP ops** (definitions, hover, references, inbound calls, workspace symbol search). Live ops work immediately — even before the index is fully built.

Results include a `source_layer` field indicating where the data came from:
- **lsp** — full language server precision (types, generics, trait impls)
- **treesitter** — structural parsing from the index (fast, always available after indexing)
- **treesitter+lsp** — combined results from both layers

When you see results from tree-sitter only for a language that should have LSP support, suggest `/lsp` to check whether the language server is installed.

Use raw file reads (Read, Grep, Glob) only for:
- String literals, config values, error messages not in the symbol index
- Files that aren't code (TOML, YAML, JSON, Markdown)
- Confirming exact syntax once code-context has given you the location

**Do not** start exploration by reading files top to bottom. Start with `search symbol` (or `search workspace_symbol` if the index is building) and `get callgraph` to find the right code, then use `get definition` and `get hover` to inspect specifics without reading entire files.

## When to recurse

If the blast radius reveals unexpected dependencies, or the call graph leads to unfamiliar territory, loop back to step 2 with new keywords. Exploration is iterative — but each loop should narrow the focus, not widen it.

## Examples

### Example 1: understanding how a feature works

User says: "explore how the kanban watcher decides which files to re-index"

Actions:
1. Orient with `{"op": "get status"}` — note which layers are active.
2. Survey with `{"op": "search symbol", "query": "watcher", "max_results": 15}` and `{"op": "search symbol", "query": "invalidate"}` — locate `KanbanWatcher::on_event` and `invalidate_file`.
3. Trace with `{"op": "get symbol", "query": "KanbanWatcher::on_event"}` to read the actual source, then `{"op": "get callgraph", "symbol": "invalidate_file", "direction": "inbound", "max_depth": 2}` to see who triggers invalidation.
4. Scope with `{"op": "get blastradius", "file_path": "src/watcher.rs", "max_hops": 3}` — confirms only the indexer and the MCP layer are affected.
5. Check tests via `{"op": "grep code", "pattern": "on_event", "file_pattern": "test"}` — one smoke test covers file creation; nothing covers deletion.
6. Conclude:

   ```
   HOW IT WORKS: on_event receives a FileEvent, matches the EventKind, and calls
                 invalidate_file for created/modified files; the indexer picks up
                 invalidated paths on its next pass.
   KEY CODE:     src/watcher.rs (KanbanWatcher::on_event, invalidate_file)
   BLAST RADIUS: indexer + MCP layer only. Deletion (EventKind::Remove) is not
                 handled — invalidate_file is never called for deleted files.
   ```

Result: Exploration is complete. The user now understands the mechanism and the gap. If they want the deletion gap fixed, that hands off to `/tdd` or `/task` — exploration itself wrote no code.

### Example 2: exploration that reveals work too large for one step

User says: `/explore what it would take to add SSO to the web app`

Actions:
1. Orient, survey auth-related symbols, trace the current login flow.
2. Blast radius on `src/auth/login.rs` shows 40+ call sites across handlers, the session store, and middleware.
3. Recognize this crosses the "single change" threshold and stop exploring.

Result: Escalate to `/plan` rather than forcing a conclusion — exploration correctly identifies that planning, not implementation, is the next step.

## Constraints

- **Don't write code during exploration.** Exploration produces understanding. Acting on that understanding is a separate, deliberate step — hand it off.
- **Don't skip the blast radius.** Jumping from "search symbol" to "I know what to do" skips the step most likely to reveal surprises.
- **Don't read files top to bottom.** That's the default behavior this skill exists to replace. Use `code_context` to find the right code, then inspect only what matters.
- **Don't explore forever.** If you've done 3 loops of steps 2–4 without converging, stop and tell the user what's unclear. Ask for direction.
- **Don't use exploration to avoid acting.** Once you can explain how it works, where it lives, and what it touches, exploration is done — move to planning or implementation.
