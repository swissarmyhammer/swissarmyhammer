---
assignees:
- claude-code
position_column: todo
position_ordinal: b880
title: add task / update task silently discard the tags array
---
`add task` and `update task` both accept a `tags` array, return `ok: true`, and apply nothing. The array is discarded without an error or a warning.

A `tags` array must give the same result as calling `tag task` once per tag.

## Reproduction

```
add task { title: "...", column: "todo", tags: ["01KJZEPKJ35S76KF7E9HS5742J", "01KT7375T468PE35B87WY042DQ"] }
→ { "ok": ..., "tags": [] }          ← tag ids, dropped

update task { id: "^t7ebyn8", tags: ["bug", "init", "mirdan"] }
→ { "ok": true }
get task { id: "^t7ebyn8" }
→ { "tags": [] }                     ← tag names, dropped
```

The single-tag op works:

```
tag task { id: "^t7ebyn8", tag: "bug" }   → applied
```

So the caller must make N calls where the schema advertises one. `tag task` with a plural `tags` array fails loudly with `parse error: missing required field: tag`, which is correct behavior. The `add`/`update` path is the silent one.

## Why this matters

The response says `ok: true`. A caller has no way to know the tags were lost except by a follow-up `get task`. An agent that trusts the acknowledgement writes untagged cards and never learns. Silent input loss is worse than rejection.

Both forms were dropped, so the cause is not id-versus-name resolution:
- full ULIDs, on `add task`
- plain tag names, on `update task`

## Required change

1. Make `tags` on `add task` and `update task` apply, with the same resolution and the same create-if-absent behavior that `tag task` uses. Route both through one shared code path so the two can never disagree again.
2. Accept the forgiving shapes the board already accepts elsewhere: a single tag, a JSON array, or a stringified JSON array. Accept a full ULID, a short id, and a tag name.
3. An unresolvable tag must be an error, not a silent no-op — the same rule the board already states for `depends_on`.
4. Audit the sibling collection fields on `add task` and `update task` for the same defect — `assignees`, `depends_on`, `attachments`. Report what is affected. Do not assume `tags` is the only one.

## Acceptance

- `add task` with a `tags` array returns a task carrying those tags. Test must fail before the change.
- `update task` with a `tags` array replaces the tag set and `get task` confirms it.
- Equivalence test: a card built with one `add task { tags: [a, b, c] }` and a card built with three `tag task` calls end with the same tag set.
- An unknown/unresolvable tag ref returns an error, and the task is unchanged.
- Whatever the audit in step 4 finds gets a test too, or its own card. #bug #kanban