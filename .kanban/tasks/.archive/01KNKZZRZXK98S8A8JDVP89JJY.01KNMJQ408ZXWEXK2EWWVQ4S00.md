---
assignees:
- claude-code
depends_on:
- 01KNKZYSYT8W352AP4KZFYVH1G
position_column: todo
position_ordinal: 947f80
title: Keyboard navigation between perspectives in the perspective tab bar
---
## What

Add keyboard commands to cycle between perspectives in the `PerspectiveTabBar`, plus number-key shortcuts to jump directly to a perspective by position.

### Current state

- `PerspectiveTabBar` (`kanban-app/ui/src/components/perspective-tab-bar.tsx`) renders a horizontal list of `PerspectiveTab` buttons. Each tab is wrapped in a `CommandScopeProvider` with moniker `perspective:{id}`, but has **no FocusScope**, no `claimWhen` predicates, and no keyboard navigation.
- Perspective switching happens via `setActivePerspectiveId` which dispatches `ui.perspective.set` through the command system (`kanban-app/ui/src/lib/perspective-context.tsx:55-61`).
- Rust-side perspective commands are in `swissarmyhammer-commands/builtin/commands/perspective.yaml` — there are no `perspective.next` / `perspective.prev` commands.
- Global keybindings in `kanban-app/ui/src/lib/keybindings.ts` have no perspective-related entries.

### Approach

**Rust side** — add two commands to `swissarmyhammer-commands/builtin/commands/perspective.yaml`:
- `perspective.next` — switch to the next perspective in the tab bar (wrapping)
- `perspective.prev` — switch to the previous perspective (wrapping)

These are UI-only commands — the frontend handles the cycling logic since it knows the filtered perspective list and active index. The Rust YAML registration makes them visible in the command palette with keybindings.

**Frontend side** — changes to two files:
1. `kanban-app/ui/src/components/perspective-tab-bar.tsx` — register `perspective.next` and `perspective.prev` as `CommandDef` entries via a `CommandScopeProvider` wrapping the entire tab bar. The execute handlers cycle `setActivePerspectiveId` through the `filteredPerspectives` array. Also add number-key commands (`perspective.goto.1` through `perspective.goto.9`) for direct jumps.
2. `kanban-app/ui/src/lib/keybindings.ts` — add global keybindings:
   - vim: `gt` → `perspective.next`, `gT` → `perspective.prev` (matches vim tab conventions)
   - cua: `Mod+]` → `perspective.next`, `Mod+[` → `perspective.prev`
   - Optionally vim `1`–`9` for `perspective.goto.N` (these are scope-level keys, not global, to avoid conflict with `0`/`$` board nav)

### Files to modify
- `swissarmyhammer-commands/builtin/commands/perspective.yaml` (add next/prev commands)
- `kanban-app/ui/src/components/perspective-tab-bar.tsx` (register commands with execute handlers)
- `kanban-app/ui/src/lib/keybindings.ts` (add vim `gt`/`gT` sequences and cua `Mod+]`/`Mod+[`)

## Acceptance Criteria
- [ ] `perspective.next` command cycles to the next perspective (wrapping from last to first)
- [ ] `perspective.prev` command cycles to the previous perspective (wrapping from first to last)
- [ ] Commands appear in the command palette
- [ ] Vim keybindings `gt` / `gT` switch perspectives (matching vim's tab-next/tab-prev convention)
- [ ] CUA keybindings `Cmd+]` / `Cmd+[` switch perspectives
- [ ] When only one perspective exists, next/prev are no-ops (no error)

## Tests
- [ ] `kanban-app/ui/src/components/perspective-tab-bar.test.tsx` — test that executing `perspective.next` cycles from first to second perspective, wraps from last to first; test `perspective.prev` wraps from first to last; test no-op with single perspective
- [ ] `kanban-app/ui/src/lib/keybindings.test.ts` — test that `g` then `t` resolves to `perspective.next` in vim mode; test that `g` then `T` resolves to `perspective.prev`
- [ ] `cargo nextest run -p swissarmyhammer-commands` — verify perspective.next and perspective.prev parse from YAML

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.