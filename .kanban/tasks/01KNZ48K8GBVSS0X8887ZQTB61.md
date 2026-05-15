---
assignees:
- wballard
depends_on:
- 01KNZ45W47G3CYH64ZVV5KXTSP
- 01KNZ46V6W1JGY5C491A8R9BVN
- 01KNZ47NWKY0DQPWRNPPHMDBDN
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffa780
project: pill-via-cm6
title: Delete MentionPill, remark-mentions, and dead helpers
---
## What

Delete the legacy pill-rendering code paths now that every consumer has migrated to the CM6 pipeline. This is the final consolidation step — after this card, there is exactly one way pills render in the app.

**Files to delete:**
- `kanban-app/ui/src/components/mention-pill.tsx`
- `kanban-app/ui/src/components/mention-pill.node.test.tsx`
- `kanban-app/ui/src/lib/remark-mentions.ts`
- Any test file that only tests `remark-mentions` (check `lib/remark-mentions.test.ts` or similar)

**Files to verify have no remaining references:**
- Run a full-repo grep for `MentionPill`, `remark-mentions`, `remarkMentions`, `MentionPillNode`, `mention-pill` — all should come back empty except inside the files being deleted
- Check `kanban-app/ui/src/components/fields/displays/markdown-display.tsx` — the migration card should have already removed the `ReactMarkdown` + `remarkMentions` imports; confirm and clean up any leftover empty imports or dead code

**Files to update:**
- Any file that imported `briefSlug` from `mention-pill.tsx` — none expected, but grep to confirm. If found, inline the 4-line function or move it to `lib/slugify.ts` as a named export.

**Bun lockfile / dependency check:**
- After deleting `remark-mentions.ts`, check whether `remark-gfm` or `react-markdown` is still used anywhere else. If `remark-mentions.ts` was the only remaining consumer of either, remove them from `kanban-app/ui/package.json` dependencies. If other code still uses them (e.g. tooltip markdown rendering), keep them.

**Final sweep:**
- `kanban-app/ui/src/lib/mention-finder.ts` — still used by `cm-mention-decorations.ts`, keep it.
- `kanban-app/ui/src/lib/cm-mention-tooltip.ts` — still used, keep it.
- `kanban-app/ui/src/lib/cm-mention-autocomplete.ts` — still used, keep it.
- `kanban-app/ui/src/hooks/use-mention-extensions.ts` — still used, keep it.

## Acceptance Criteria
- [ ] `mention-pill.tsx`, `mention-pill.node.test.tsx`, `remark-mentions.ts` all deleted
- [ ] Full-repo grep for `MentionPill` / `remarkMentions` / `mention-pill` returns zero hits
- [ ] `package.json` cleaned up if react-markdown / remark-gfm are no longer used anywhere
- [ ] `bun run typecheck` passes
- [ ] `bun run lint` passes
- [ ] Full test suite passes (`bun test`)

## Tests
- [ ] Run full test suite: `bun test` from `kanban-app/ui/`
- [ ] Run typecheck: `bun run typecheck`
- [ ] Run lint: `bun run lint`
- [ ] Smoke: `bun run dev`, open the board, inspect a task card with mentions, tags, depends_on, and a column indicator. Confirm every pill renders with the clipped display name via CM6. Confirm no console errors.

## Workflow
- Run `rg -l 'MentionPill|remarkMentions|remark-mentions|mention-pill'` before deleting to get the complete list of files that reference the doomed identifiers
- Delete files in one commit, verify the test suite, then land
- If anything still imports from the deleted files, go back to the prior migration cards and fix the gap there — do not reintroduce a shim in this card