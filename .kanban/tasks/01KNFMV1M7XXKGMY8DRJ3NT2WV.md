---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffc380
title: 'NIT: AppModeScopeWrapper uses anonymous inline prop type instead of named interface'
---
**File**: kanban-app/ui/src/components/app-mode-container.tsx\n\n**What**: `function AppModeScopeWrapper({ children }: { children: ReactNode })` uses an anonymous inline object type for props. The JS/TS review guidelines require named prop interfaces: \"Every component gets a `interface FooProps` co-located above it.\"\n\n**Suggestion**: Extract `interface AppModeScopeWrapperProps { children: ReactNode }` and use it.\n\n**Subtasks**:\n- [ ] Add named AppModeScopeWrapperProps interface\n- [ ] Verify fix by running tests #review-finding