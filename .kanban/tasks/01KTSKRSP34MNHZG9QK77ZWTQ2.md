---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw2xd0vsjzgtps90n78vmrw9
  text: |-
    Picked up; moved to doing. The local lookup in entity-icon.tsx (`kebabToPascal` + `icons[key]` registry access) was byte-identical to lib/icon-name.ts::iconByName, so `iconByName(schema?.entity.icon) ?? LayoutGrid` is exactly equivalent — fallback behavior (missing schema icon or unresolvable name -> LayoutGrid) preserved.

    Done:
    - Added characterization test src/components/entity-icon.browser.test.tsx (3 cases: declared icon resolves to lucide-table; unresolvable name -> LayoutGrid; no entity.icon -> LayoutGrid). Confirmed green on the pre-refactor code first (safety net), then refactored.
    - Refactored entity-icon.tsx: removed `kebabToPascal` + direct `icons` registry import, now imports `iconByName` and renders `iconByName(...) ?? LayoutGrid`. Public API (EntityIconProps) unchanged.

    Verification (all green): `npx tsc --noEmit` exit 0; entity-icon.browser.test.tsx 3/3; icon-name.test.ts + view-icon.node.test.ts 6/6. Consumers (command-palette.tsx, quick-capture.tsx) use the unchanged EntityIcon API; tsc confirms they compile. No dedicated tests exist for those consumers.
  timestamp: 2026-06-26T21:29:32.281791+00:00
- actor: claude-code
  id: 01kw2xh29z0ex1yhg11agznyxb
  text: |-
    Correction: my first write to entity-icon.tsx silently no-op'd (the write echoed the old content with no mutation block) — so the prior "Done" comment was premature; the refactor had not actually landed. Caught by the adversarial double-check, which found `git diff HEAD` on entity-icon.tsx empty.

    Re-applied via an explicit edit. `git diff HEAD` now confirms: removed local `kebabToPascal` + the `icons` lucide import; added `import { iconByName } from "@/lib/icon-name"`; body is now `const Icon = iconByName(schema?.entity.icon) ?? LayoutGrid; return <Icon className={className} />;`. className pass-through preserved.

    Re-verified all green: tsc --noEmit exit 0; entity-icon.browser.test.tsx 3/3 pass. The characterization test is a behavior-preservation guard (passes before and after by design, as expected for a pure refactor).
  timestamp: 2026-06-26T21:31:44.831024+00:00
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffffff580
title: Unify entity-icon.tsx lucide lookup onto lib/icon-name.ts::iconByName
---
`apps/kanban-app/ui/src/components/entity-icon.tsx` carries its own verbatim `kebabToPascal` + lucide `icons` registry lookup — the same pure logic now centralized in `apps/kanban-app/ui/src/lib/icon-name.ts::iconByName` (extracted while working the review warning on 01KTCRY5W2BP7TYTHV4JB9CH8K, which scoped the unification to `fieldIcon`/`viewIcon` only).

## What
Replace the local helper + lookup in `EntityIcon` with `iconByName(iconName) ?? LayoutGrid`, keeping `EntityIcon`'s public API and `LayoutGrid` fallback behavior unchanged.

## Acceptance Criteria
- [ ] `entity-icon.tsx` no longer defines `kebabToPascal` or touches the lucide `icons` registry directly; it delegates to `iconByName`.
- [ ] Fallback behavior unchanged: missing schema icon or unresolvable name still renders `LayoutGrid`.
- [ ] Scoped vitest for entity-icon (and any consumers) + `tsc --noEmit` clean.

#refactor