---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffff9b80
title: Migrate to unified radix-ui package
---
## What

Migrate from 5 individual `@radix-ui/react-*` packages to the unified `radix-ui` mono package. This is a no-API-change dependency consolidation — imports change, behavior does not.

**Automated approach:** Run `pnpm dlx shadcn@latest migrate radix` from `kanban-app/ui/`. This rewrites imports and updates `package.json` automatically.

**Manual fallback** if the CLI doesn't work — 5 files need import changes:

| File | Old import | New import |
|------|-----------|------------|
| `kanban-app/ui/src/components/ui/tooltip.tsx:2` | `import * as TooltipPrimitive from "@radix-ui/react-tooltip"` | `import { Tooltip as TooltipPrimitive } from "radix-ui"` |
| `kanban-app/ui/src/components/ui/popover.tsx:2` | `import * as PopoverPrimitive from "@radix-ui/react-popover"` | `import { Popover as PopoverPrimitive } from "radix-ui"` |
| `kanban-app/ui/src/components/ui/dropdown-menu.tsx:2` | `import * as DropdownMenuPrimitive from "@radix-ui/react-dropdown-menu"` | `import { DropdownMenu as DropdownMenuPrimitive } from "radix-ui"` |
| `kanban-app/ui/src/components/ui/separator.tsx:2` | `import * as SeparatorPrimitive from "@radix-ui/react-separator"` | `import { Separator as SeparatorPrimitive } from "radix-ui"` |
| `kanban-app/ui/src/components/ui/button.tsx:2` | `import { Slot } from "@radix-ui/react-slot"` | `import { Slot } from "radix-ui"` |

Then in `kanban-app/ui/package.json`:
- Remove: `@radix-ui/react-dropdown-menu`, `@radix-ui/react-popover`, `@radix-ui/react-separator`, `@radix-ui/react-slot`, `@radix-ui/react-tooltip`
- Add: `"radix-ui": "^1.0.0"` (or whatever the current version is)

## Acceptance Criteria
- [ ] No `@radix-ui/react-*` packages in `package.json`
- [ ] Single `radix-ui` dependency in `package.json`
- [ ] All 5 import sites updated
- [ ] App builds without errors (`pnpm build`)
- [ ] All UI components using Radix primitives still render correctly

## Tests
- [ ] Run `cd kanban-app/ui && pnpm vitest run` — all tests pass
- [ ] Run `cd kanban-app/ui && pnpm tsc --noEmit` — no type errors
- [ ] Manual: verify tooltip, popover, dropdown menu, separator, and button slot all work