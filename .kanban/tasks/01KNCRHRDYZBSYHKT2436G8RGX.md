---
assignees:
- claude-code
depends_on:
- 01KNC7N6PKWCAXPXXT2CZ4228P
position_column: todo
position_ordinal: 8a80
position_swimlane: container-refactor
title: Extract AppModeContainer as first child of WindowContainer
---
## What

Extract an `AppModeContainer` that manages the application interaction mode (normal, command, search) as the **first container inside WindowContainer**, wrapping everything including the toolbar/NavBar. The mode governs the entire UI surface — keybinding interpretation, visual indicators, and command availability all depend on the current mode.

**Current state:** `AppModeProvider` is a simple React `useState` wrapper in `kanban-app/ui/src/lib/app-mode-context.tsx` (42 lines). It's nested deep inside the provider soup in App.tsx (line 560, inside UIStateProvider). `AppShell` reads it for keybinding dispatch. `ModeIndicator` reads it for display. The mode is frontend-only — not in Rust UIState.

**Target:**
1. `AppModeContainer` wraps immediately inside `WindowContainer`, ABOVE everything else including NavBar and the toolbar
2. Owns `CommandScopeProvider moniker="mode:{current_mode}"` so commands can be mode-aware
3. The mode should be part of Rust UIState (currently `keymap_mode` is in UIState but `app_mode` is not) — add a `ui.mode.set` command in Rust
4. `ModeIndicator` becomes a presenter reading from this container's context
5. Mode transitions (normal → command → search → normal) are command-driven through Rust

**Why first inside Window:** The mode affects everything — which keybindings are active, whether the toolbar shows a search field, whether navigation commands are available. It must wrap the NavBar, not sit below it.

**Container tree update:**
```
WindowContainer          window:{label}
  AppModeContainer       mode:{mode}        — NEW: wraps everything, mode-aware scope
    RustEngineContainer  engine
      BoardContainer     board:{id}
        ...
```

**Files to create/modify:**
- `kanban-app/ui/src/components/app-mode-container.tsx` (NEW)
- `kanban-app/ui/src/components/app-mode-container.test.tsx` (NEW) — TDD
- `swissarmyhammer-kanban/src/commands/ui_commands.rs` — add `SetAppModeCmd`
- `swissarmyhammer-commands/src/ui_state.rs` — add `app_mode` to `WindowState`
- `kanban-app/ui/src/lib/app-mode-context.tsx` — may become unnecessary or simplified
- `kanban-app/ui/src/App.tsx` — remove `AppModeProvider`, add `AppModeContainer`

## TDD Process
1. Write `app-mode-container.test.tsx` FIRST with failing tests
2. Tests verify: mode context is available, CommandScopeProvider has mode moniker, mode transitions dispatch to Rust, ModeIndicator reads mode correctly, keybinding behavior changes with mode
3. Write Rust test for `SetAppModeCmd` in `ui_commands.rs`
4. Implement until tests pass
5. Refactor

## Acceptance Criteria
- [ ] `AppModeContainer` exists, wraps immediately inside WindowContainer
- [ ] Mode is in Rust UIState, not just React state
- [ ] `ui.mode.set` command exists in Rust with tests
- [ ] Mode transitions go through command dispatch
- [ ] ModeIndicator still works
- [ ] Keybinding dispatch respects mode
- [ ] AppShell reads mode from the container context

## Tests
- [ ] `app-mode-container.test.tsx` — all pass (written first, RED → GREEN)
- [ ] `cargo test -p swissarmyhammer-kanban -- ui_commands::tests` — SetAppModeCmd passes
- [ ] Run `cd kanban-app && pnpm vitest run` — all tests pass #container-refactor