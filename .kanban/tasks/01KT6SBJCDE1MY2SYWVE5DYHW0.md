---
assignees:
- claude-code
depends_on:
- 01KT6SAXCBZFE6S0DEPZDJSQAA
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffec80
project: short-ids
title: 'Short IDs: CM6 — ^ autocomplete inserts short id'
---
Typing `^` in CM6 editors offers a task picker. Search matches the query against the task title, its short id, OR its full ULID. The dropdown is labeled by title; the accepted completion inserts `^<short>` (never the long form, never a title-slug).

## Background (from scoping)
- Autocomplete stack exists and is generic: `cm-mention-autocomplete.ts` (`createMentionCompletionSource(prefix, search)`, `apply` writes `${prefix}${slug}`, label previews `${prefix}${displayName}`), wired in `hooks/use-mention-extensions.ts` via `buildAsyncSearch("task")` → Tauri `search_mentions`.
- `^`/task autocomplete is currently active only in the filter editor (`filter-editor.tsx`, `includeFilterSigils: true`) and inserts a title-slug. The description editor (`markdown.tsx`) gets the schema-loop source but inserts a title-slug too.

## Scope
- Search (3-way): `search_mentions(entity_type=task)` matches the query against title (substring, as today) AND short-id prefix AND full-ULID prefix/exact. So `^au` finds by title; `^8rf` or a pasted full ULID find by id. (Backend match extension lives in short-ids-mention-identity.)
- Dropdown label = task title (+ color), so the list is human-pickable.
- `apply` inserts `^<short>` (the canonical short id), regardless of whether the match was by title, short id, or full ULID. Never inserts a long ULID or title-slug.
- Activate the task `^` completion source in the description editor (`markdown.tsx`), not just the filter editor.

## Acceptance
- In a task description, typing `^` + a title word lists tasks by title; accepting inserts `^<short>`, which renders as a pill (via short-ids-cm6-pills).
- Typing `^` + short-id chars, or pasting a full ULID after `^`, surfaces the matching task; accepting still inserts `^<short>`.
- Filter editor `^` autocomplete also inserts short ids and the resulting filter matches (via short-ids-filter-eval).

Depends on short-ids-mention-identity.