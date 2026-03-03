---
title: Add CM6 tag autocomplete
position:
  column: done
  ordinal: b3
---
When typing `#` followed by characters in the CM6 editor, show a completion dropdown with existing tags and a "Create new" option.

**New file: ui/src/lib/cm-tag-autocomplete.ts**
- Export `tagCompletionSource(tags: Tag[])` returning a CM6 `CompletionSource`
- Trigger: `context.matchBefore(/#[\w][\w/\-]*/u)` (simplified from full Unicode regex)
- Filter: substring match, prefix matches sorted first
- "Create #fragment" option when no exact match exists
- Custom render per option: colored dot next to tag name, description as detail text

**EditableMarkdown integration (editable-markdown.tsx):**
- Import `autocompletion` from `@codemirror/autocomplete`
- Add `autocompletion({ override: [tagCompletionSource(tags)] })` to extensions
- Only in multiline mode (descriptions, not titles)

**Files:** `ui/src/lib/cm-tag-autocomplete.ts` (new), `ui/src/components/editable-markdown.tsx`

- [ ] Create cm-tag-autocomplete.ts with CompletionSource
- [ ] Custom rendering with colored dots
- [ ] Add "Create new" option
- [ ] Wire into EditableMarkdown multiline extensions
- [ ] Visual verification in running app
- [ ] npm run build passes