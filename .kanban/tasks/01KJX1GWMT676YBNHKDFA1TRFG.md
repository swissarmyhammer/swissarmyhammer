---
position_column: done
position_ordinal: a1
title: CommandScope provider and scope chain resolution
---
Phase 1 deliverable from app-architecture.md.

React context-based command scope system. Commands live in scopes that nest with the component tree. Resolution walks up from the focused element's nearest scope.

## What to build

### CommandScope React context provider
- `<CommandScope commands={[...]}>` wraps a subtree
- Each scope registers commands (id, name, description, keys, execute, available)
- Scopes nest — child providers shadow parent providers

### Scope chain resolution
- Given a command id, walk up from current scope to root
- Deepest matching command wins (shadowing)
- `available: false` blocks upward walk (blocking)
- Unregistered commands pass through to parent

### useAvailableCommands() hook
- Returns all commands visible from the current scope
- Grouped by scope depth (global, view, grid, etc.)
- Used by the command palette and :help

### Command interface
```typescript
interface Command {
  id: string
  name: string
  description?: string
  keys?: { vim?: string; cua?: string; emacs?: string }
  execute: () => Promise<void>  // or sync
  available?: boolean  // default true
}
```

## Files
- `ui/src/lib/command-scope.tsx` — CommandScope provider, useCommandScope, useAvailableCommands
- Tests for scope resolution, shadowing, blocking, pass-through

## Checklist
- [ ] CommandScope provider component
- [ ] Scope chain resolution logic
- [ ] Shadowing (deeper scope same command id wins)
- [ ] Blocking (available: false stops upward walk)
- [ ] Pass-through (unregistered commands walk to parent)
- [ ] useAvailableCommands() hook
- [ ] useExecuteCommand(id) hook
- [ ] Tests for all resolution behaviors
- [ ] Run test suite