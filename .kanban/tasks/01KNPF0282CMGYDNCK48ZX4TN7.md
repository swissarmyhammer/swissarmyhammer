---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffffff080
title: Fix `gt`/`gT` vim keybindings for perspective navigation
---
## What

`gt` and `gT` vim keybindings do not trigger `perspective.next` / `perspective.prev`. The command palette equivalents work fine.

**Root cause**: The vim `SEQUENCE_TABLES` in `kanban-app/ui/src/lib/keybindings.ts:69-77` only contains entries for `gg`, `dd`, and `zo`. The `g→t` (perspective.next) and `g→Shift+T` (perspective.prev) entries are missing, even though the YAML command definitions (`swissarmyhammer-commands/builtin/commands/perspective.yaml:118-128`) correctly declare `vim: "gt"` and `vim: "gT"`.

The scope binding path (`extractScopeBindings`) also cannot resolve these because:
1. It reads `"gt"` / `"gT"` as literal single-key strings, but `normalizeKeyEvent` never produces multi-char strings from a single keypress.
2. `perspective.next` / `perspective.prev` have no `scope` in their YAML definitions, so they aren't scope commands.

**Fix**: Add `t` and `Shift+T` entries under the existing `g` key in `SEQUENCE_TABLES.vim`:

```typescript
g: { g: "nav.first", t: "perspective.next", "Shift+T": "perspective.prev" },
```

Note: `normalizeKeyEvent` produces `"Shift+T"` for a Shift+T keypress (line 127-129 of keybindings.ts), so the second-key lookup for `gT` must use `"Shift+T"` as the key.

**Files to modify**:
- `kanban-app/ui/src/lib/keybindings.ts` — add sequence entries
- `kanban-app/ui/src/lib/keybindings.test.ts` — add tests for the new sequences

## Acceptance Criteria
- [ ] Pressing `g` then `t` in vim mode dispatches `perspective.next`
- [ ] Pressing `g` then `Shift+T` in vim mode dispatches `perspective.prev`
- [ ] Existing `gg` sequence still works (no regression)
- [ ] Sequence timeout (500ms) applies to the new sequences

## Tests
- [ ] Add test in `kanban-app/ui/src/lib/keybindings.test.ts`: "handles vim gt sequence → perspective.next"
- [ ] Add test in `kanban-app/ui/src/lib/keybindings.test.ts`: "handles vim gT (Shift+T) sequence → perspective.prev"
- [ ] Run `cd kanban-app/ui && npx vitest run src/lib/keybindings.test.ts` — all tests pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.