---
assignees:
- claude-code
depends_on:
- 01KNJDAAZQ4B6PFGR9N1Z7A7YJ
position_column: done
position_ordinal: ffffffffffffffffffffff8a80
position_swimlane: null
title: 'FILTER-5: Add virtual tags to filter editor autocomplete (shared CM6 infra)'
---
## What

Extend the existing `useMentionExtensions` hook to optionally include virtual tags in `#` completions, controlled by a prop/option. The filter editor passes `includeVirtualTags: true`; body editors and tag editors do not. **No new editor component** — the filter editor already uses the shared CM6 infrastructure.

### Design

The existing autocomplete pipeline:
1. `useMentionExtensions()` reads mentionable types from schema, builds completion sources per prefix
2. `createMentionCompletionSource(prefix, search)` creates a `CompletionSource` for a given sigil
3. All sources compose into one `autocompletion()` extension

The change is narrow: when `includeVirtualTags` is true, the `#` (tag) completion source should also offer virtual tag names (READY, BLOCKED, BLOCKING) alongside real board tags. Virtual tags come from the `VirtualTagRegistry` on the backend — the frontend already receives them as part of entity data.

### Approach
- Add an `options` parameter to `useMentionExtensions`: `{ includeVirtualTags?: boolean }`
- When true, augment the tag search results with virtual tag entries (distinct styling — e.g. italic or different icon to distinguish computed from real tags)
- Virtual tag names come from a backend query or are hardcoded initially (they're defined in `swissarmyhammer-kanban/src/virtual_tags.rs`: READY, BLOCKED, BLOCKING)
- The `FilterEditor` component passes `includeVirtualTags: true` to the hook
- Body editors (`text-editor.tsx`) and tag editors (`multi-select-editor.tsx`) pass nothing (defaults to false)

### Files to modify
- `kanban-app/ui/src/hooks/use-mention-extensions.ts` — add `includeVirtualTags` option, merge virtual tag entries into `#` completion source when enabled
- `kanban-app/ui/src/components/filter-editor.tsx` — call `useMentionExtensions({ includeVirtualTags: true })` and include in extensions
- `kanban-app/ui/src/lib/cm-mention-autocomplete.ts` — no changes needed if virtual tags are injected as additional search results

### What NOT to do
- Do NOT create a separate editor component for the filter bar
- Do NOT fork `useMentionExtensions` into a filter-specific version
- Do NOT add virtual tags to non-filter autocomplete contexts

### Also wire `@` and `^` completions for filter context
The filter editor also needs `@user` and `^ref` completions. These use the same `createMentionCompletionSource` pattern:
- `@` prefix → actors from `actor.list` / entity store
- `^` prefix → card IDs from entity store (show title as detail)

These should also be gated behind a filter-editor option (e.g. `includeFilterSigils: true`) since body editors don't use `@` or `^` as completion triggers (body editors use `#` for tags only via the mentionable types system).

## Acceptance Criteria
- [ ] Filter editor: typing `#` shows real tags AND virtual tags (READY, BLOCKED, BLOCKING)
- [ ] Filter editor: typing `#RE` filters to show READY
- [ ] Filter editor: typing `@` shows actors
- [ ] Filter editor: typing `^` shows card IDs with title detail
- [ ] Body editor: typing `#` shows real tags only (no virtual tags)
- [ ] Tag editor: typing `#` shows real tags only (no virtual tags)
- [ ] Virtual tags visually distinguishable from real tags in completion list (italic, icon, or label)
- [ ] No new editor components created

## Tests
- [ ] `kanban-app/ui/src/hooks/__tests__/use-mention-extensions.test.ts` — test with `includeVirtualTags: true` returns virtual tag entries
- [ ] `kanban-app/ui/src/hooks/__tests__/use-mention-extensions.test.ts` — test with default options does NOT return virtual tags
- [ ] `npm test` in kanban-app passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.