---
assignees:
- wballard
depends_on:
- 01KNZ42426TGY4AVSDMQMRFQGS
- 01KNZ432XEGXX0VWPFSDXXXG32
position_column: done
position_ordinal: ffffffffffffffffffffffbf80
project: pill-via-cm6
title: Migrate MarkdownDisplay to TextViewer + CM6 mention widgets
---
## What

Rewrite `MarkdownDisplay` so description-field viewing uses a read-only CM6 (via `TextViewer`) with the markdown language and full mention extensions. Eliminates the `ReactMarkdown` + `remark-mentions` + `MentionPill` pipeline for description viewing — pills in description bodies now render via the same CM6 widgets as pills in the editable description editor.

**Files to modify:**
- `kanban-app/ui/src/components/fields/displays/markdown-display.tsx` — replace the `<ReactMarkdown>` + custom `mentionComponents` block with `<TextViewer>` + `extensions` composed from:
  - `markdown({ base: markdownLanguage })` — markdown highlighting (same language extension as the editor)
  - `useMentionExtensions()` — all mention decorations, widgets, tooltips
  - A new **checkbox plugin** (see below) to preserve interactive task-list checkboxes
- `kanban-app/ui/src/lib/cm-markdown-checkbox.ts` (new) — CM6 ViewPlugin that finds `- [ ]` / `- [x]` patterns via MatchDecorator, replaces each with a widget containing an `<input type="checkbox">`. Widget's change handler computes the original source index (by counting occurrences) and calls a facet-provided `onToggle(sourceIndex)` callback.
- `kanban-app/ui/src/components/fields/displays/markdown-display.tsx` — `handleCheckboxChange` logic moves into the new plugin's facet. Pass the `text → toggleCheckbox(text, index)` transform through.

**What goes away (but only after the cleanup card — do not delete in this card):**
- Usage of `remark-mentions` in `markdown-display.tsx`
- Usage of `MentionPill` in `markdown-display.tsx`
- `ReactMarkdown` import in `markdown-display.tsx`

**Compact mode is untouched.** The `mode === "compact"` branch still renders truncated plain text — no CM6 needed, and a tiny editor per row on the board view would be wasteful.

**CSS verification:** The `prose prose-sm dark:prose-invert max-w-none` wrapper class currently scopes Tailwind typography. CM6 will render its own markdown styling via the language extension. Either (a) keep the prose wrapper to style headings/paragraphs consistently, or (b) drop it and rely on the CM6 theme. Investigate which yields the right look; default to keeping the wrapper unless it produces visible breakage.

## Acceptance Criteria
- [ ] `MarkdownDisplay` full mode renders via `<TextViewer>` with markdown language + mention extensions + checkbox plugin
- [ ] Compact mode unchanged (still truncated plain text)
- [ ] Mention pills in description bodies show clipped display names (via widget) — not slugs
- [ ] Task list checkboxes are clickable and toggle the underlying markdown source correctly via `onCommit`
- [ ] Visual appearance of headings, paragraphs, code blocks, lists matches or improves on the previous ReactMarkdown output
- [ ] No regression in existing `markdown-display.test.tsx` — update assertions for new DOM structure but preserve the same behavioral coverage

## Tests
- [ ] `kanban-app/ui/src/lib/cm-markdown-checkbox.test.ts` (new) — unit test the checkbox plugin: doc with two checkboxes, click the second, assert the facet callback fires with sourceIndex=1
- [ ] Update `kanban-app/ui/src/components/fields/displays/markdown-display.test.tsx` — render a description containing `#bug-fix $my-project ^task-title`, assert the rendered DOM contains the three clipped display names (e.g. `#Bug Fix`, `$My Project`, `^Task Title`) via their pill `textContent`
- [ ] Update the same file — render a description with `- [ ] todo` + `- [x] done`, simulate a click on the first checkbox, assert `onCommit` is called with the toggled markdown source
- [ ] Update the same file — compact mode test still passes unchanged
- [ ] Run: `bun test markdown-display cm-markdown-checkbox` — all pass
- [ ] Smoke: `bun run dev`, inspect a task card with a description containing mentions and checkboxes; verify visual parity and checkbox interactivity

## Workflow
- Use `/tdd` — start with the checkbox plugin unit test (isolated, fastest feedback). Then write the updated markdown-display tests. Then implement the migration. Watch each test flip from red to green.
