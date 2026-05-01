---
assignees:
- claude-code
depends_on:
- 01KNESD0630BPJG3BK84KEJMJP
position_column: done
position_ordinal: ffffffffffffffffffffffffb580
title: 'LSP-SKILL: Update lsp skill for live ops awareness'
---
## What

The lsp skill (`builtin/skills/lsp/SKILL.md`) currently only handles LSP server installation/diagnosis. With live LSP ops, the skill's **trigger description** needs updating so Claude knows that:

1. **LSP status affects live op availability** — when `source_layer` reports degraded results, Claude should think "maybe I need to install an LSP server" and reach for `/lsp`. The skill description should mention this connection.

2. **Update the description/trigger** — Currently triggers on "lsp", "language servers", "check lsp". Should also trigger when Claude notices degraded results (e.g., `SourceLayer::TreeSitter` on ops that should have live LSP). Add trigger patterns like "code intelligence not working", "can't go to definition", "no type info available".

3. **Remove index readiness gate** — Step 1 says "If the index has zero files or is still building, tell the user to wait." Should instead check `code_context` with `{"op": "lsp status"}` directly — LSP server installation doesn't depend on the index.

4. **Add post-install verification** — After installing an LSP server, suggest the user try a live op (e.g., `get_hover` on a known file) to confirm it's working end-to-end, not just installed.

### Files to modify
- `builtin/skills/lsp/SKILL.md` — the source of truth

## Acceptance Criteria
- [ ] Skill description mentions connection to live LSP ops and degraded results
- [ ] Trigger patterns expanded for "no code intelligence" scenarios
- [ ] Index readiness gate removed from step 1
- [ ] Post-install verification suggests trying a live op
- [ ] Skill still handles the core install/diagnose flow

## Tests
- [ ] Skill renders correctly
- [ ] Skill description matches trigger patterns

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

#lsp-live