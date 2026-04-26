---
assignees:
- claude-code
depends_on:
- 01KNESD0630BPJG3BK84KEJMJP
position_column: done
position_ordinal: ffffffffffffffffffffffffb880
title: 'LSP-SKILL: Update explore skill for layered resolution and live LSP ops'
---
## What

The explore skill (`builtin/skills/explore/SKILL.md`) is the primary skill that triggers when Claude is reading or understanding code. It currently only references index-based ops and gates on index readiness. It needs a major update to:

1. **Remove the index readiness gate** — Step 1 says "If TS indexed < 90%, wait and re-check. Don't explore with a stale index." This contradicts the layered design. Exploration should work immediately with whatever layers are available.

2. **Add live LSP ops to the exploration process** — The new ops are powerful exploration primitives:
   - **Step 3 (Trace)**: Add `get_definition` — "jump to where this symbol is defined" instead of just reading callgraphs. Add `get_hover` — "what type is this? what's the signature?" without reading the whole file. Add `get_references` — "who uses this symbol across the codebase?"
   - **Step 2 (Survey)**: Add `workspace_symbol_live` as an alternative to `search_symbol` when the index is still building.
   - **Step 3 (Trace)**: Add `get_inbound_calls` alongside `get_callgraph` — "who calls this function?" with live LSP precision.
   - **Step 4 (Scope)**: `get_references` supplements `get_blastradius` — blast radius uses call edges, references catches type/field usage too.

3. **Update the "Using code-context" section** — Currently says code-context is the primary tool and it uses the index. Should explain that code-context now has both indexed AND live ops, and live ops work even before the index is ready.

4. **Add layer-awareness guidance** — Tell Claude that results include `source_layer` and to note when operating from a lower layer. e.g., "Results from tree-sitter only — LSP not available for this language. Consider `/lsp` to install the server."

### Files to modify
- `builtin/skills/explore/SKILL.md` — the source of truth (`.skills/` is generated)

## Acceptance Criteria
- [ ] Index readiness gate removed — exploration works immediately
- [ ] `get_definition`, `get_hover`, `get_references` appear in the exploration process
- [ ] `get_inbound_calls` added alongside `get_callgraph` for tracing
- [ ] `workspace_symbol_live` mentioned as alternative to `search_symbol`
- [ ] "Using code-context" section updated for layered model
- [ ] Skill still triggers on "explore", "investigate", "how does X work"
- [ ] Skill guides Claude to note `source_layer` and suggest `/lsp` when degraded

## Tests
- [ ] Skill renders correctly (no Jinja syntax errors)
- [ ] Skill description still matches the trigger patterns in system prompt

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

#lsp-live