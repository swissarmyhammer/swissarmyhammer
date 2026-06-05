---
assignees:
- claude-code
depends_on:
- 01KT6R6HR3KJT6JVNDRAJV8V4T
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffea80
project: short-ids
title: 'Short IDs: mention identity — task ^ slug = short id (backend)'
---
Shared backend glue for the CM6 cards: make the task `^` mention key on the 7-char short id (not slugify(title)), resolve BOTH short and full-ULID forms, and make task search match title + short id + full ULID. Both CM6 cards sit on this.

## Background (from scoping)
The CM6 mention stack is generic over prefix and supports `^`/task, but keys on `slugify(displayName)`:
- `task.yaml` (`crates/swissarmyhammer-kanban/builtin/entities/task.yaml`) declares `mention_prefix: "^"`, `mention_display_field: title`, NO `mention_slug_field` → slug defaults to `slugify(title)`.
- `search_mentions` (`apps/kanban-app/src/commands.rs`) returns title-derived slugs and matches the query against title only; frontend `buildMentionMetaMap` keys the metaMap by `slugify(title)`.

## Roles (two of short id, two of title)
- short id → mention slug (inserted token) AND pill label.
- title → autocomplete dropdown label AND pill hover tooltip.
So each task mention carries: `{ short_id, title, color }`.

## Scope
- `search_mentions(entity_type=task)` returns `{ slug: short_id, display: title, color }`.
- Extend its query matching to title (substring, as today) + short-id prefix + full-ULID prefix/exact (consumed by the autocomplete card's 3-way search).
- Frontend task metaMap keyed by short id, carrying title + color, so `MentionWidget` can label with the short id and tooltip with the title, and autocomplete can label the dropdown with the title.
- Resolver accepts BOTH 7-char short id and full 26-char ULID (full → last-7 → lookup), reusing the core resolver. Single resolution semantics shared by pills (forgiving display), tool API (forgiving input), and autocomplete.

## Out of scope
- Inline pill shape-matching / tooltip wiring → short-ids-cm6-pills
- Autocomplete dropdown/apply behavior → short-ids-cm6-autocomplete

## Acceptance
- `search_mentions(entity_type=task)` returns short-id slugs + title display names, and matches a query by title, by short id, and by full ULID.
- Frontend resolves a short id OR a full ULID → task title/color without a title-slug round-trip.

Depends on core derivation/resolver.