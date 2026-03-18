---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffd480
title: Codebase mapping skill with Mermaid diagrams
---
New `/map` skill that uses our code-context (treesitter chunks, symbols, call edges) to generate a rich, visual overview of a codebase.

**Output:** `ARCHITECTURE.md` at repo root — terminal gets a summary, file gets the full details with Mermaid diagrams.

**Diagrams to include:**
- **Architecture diagram** — modules/crates/packages and their dependency relationships
- **Call graph** — key entry points and what they call (uses lsp_call_edges)
- **File tree heatmap** — which areas are largest/most complex
- **Symbol index** — major types, traits, interfaces grouped by module

**How it works:**
1. Ensure code-context is populated (trigger indexing if needed)
2. Query ts_chunks and lsp_symbols for structural data
3. Query lsp_call_edges for relationships
4. Use LLM to synthesize findings into narrative + diagrams
5. Write `ARCHITECTURE.md` at repo root with ```mermaid blocks
6. Print summary to terminal with link to the file

**Key differentiator vs GSD's `/map-codebase`:**
- GSD uses pure LLM file reading — expensive, slow, context-heavy
- We use pre-indexed structural data from treesitter — fast, accurate, cheap
- We produce actual diagrams, not just prose summaries
- Diagrams render natively in GitHub, VS Code, Obsidian, etc.

**Diagram types to support:**
- `graph TD` for dependency/architecture diagrams
- `sequenceDiagram` for key flows
- `classDiagram` for type hierarchies
- `pie` for composition/size breakdowns

**Stretch:** Option to scope to a subdirectory or specific module.