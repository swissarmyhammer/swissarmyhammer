---
assignees:
- wballard
depends_on:
- 01KNZ42426TGY4AVSDMQMRFQGS
position_column: done
position_ordinal: ffffffffffffffffffffffffffffff9780
project: pill-via-cm6
title: Autocomplete dropdown shows display name, writes slug
---
## What

Align the CM6 autocomplete dropdown with the new widget behavior. Today the dropdown shows `$my-cool-project` (slug) as the primary label and `My Cool Project` as detail text. But after the widget card lands, selecting a completion immediately shows `$My Cool Project` in the editor. The dropdown should preview what you'll actually see.

**File to modify:**
- `kanban-app/ui/src/lib/cm-mention-autocomplete.ts` — in `createMentionCompletionSource`, change the `Completion` object:
  - `label: ${prefix}${r.displayName}` — primary text in dropdown matches the widget's visible text
  - `apply: ${prefix}${r.slug}` — what actually gets written to the buffer (CM6 supports `apply` as a string that replaces `label` on insertion)
  - `detail: r.slug` — secondary hint showing the underlying slug
  - `info`: keep the colored dot + display name tooltip (unchanged)

**Also update in `multi-select-editor.tsx`:** the same autocomplete source is used there. The dropdown should match — show display name, write slug.

**No backend changes.** The `search_mentions` response already returns both `display_name` and the slug is derived client-side via `slugify(display_name)`.

## Acceptance Criteria
- [ ] Autocomplete dropdown primary label shows `${prefix}${displayName}` (e.g. `$My Cool Project`)
- [ ] Selecting a completion writes `${prefix}${slug}` to the buffer (e.g. `$my-cool-project`)
- [ ] Detail text shows the slug as a secondary hint
- [ ] Info popup still shows colored dot + display name
- [ ] Works in both description editor and multi-select editor contexts

## Tests
- [ ] Update `kanban-app/ui/src/hooks/__tests__/use-mention-extensions.test.ts` — if any existing tests assert the completion label shape, update them
- [ ] Add a unit test for `createMentionCompletionSource`: mock a search that returns `{ slug: "my-project", displayName: "My Project", color: "ff0000" }`, trigger a completion context, assert the returned `Completion` has `label: "$My Project"` and `apply: "$my-project"`
- [ ] Run: `bun test cm-mention-autocomplete use-mention-extensions` — all pass
- [ ] Smoke: `bun run dev`, type `$` in a description editor, confirm the dropdown shows display names and the buffer gets the slug on selection

## Workflow
- Use `/tdd` — write the unit test for the new label/apply shape, watch it fail, update `createMentionCompletionSource`, watch it pass.
