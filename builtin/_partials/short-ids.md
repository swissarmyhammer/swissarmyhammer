---
title: Short IDs
description: How to read and reference kanban tasks by their canonical short id
partial: true
---

## Short IDs — reference tasks by short id, never hand-abbreviated prefixes

Every task's stored identity is its full 26-char ULID (e.g. `01KT6SA4911JQPK09YQRC9RB4G`). For humans, each task also has a **short id**: the **last 7 characters of the ULID, lowercased**, shown as `^<short>` (e.g. `^rc9rb4g`). The short id is never stored — it is always derived from the ULID — and it is the canonical short handle.

**Quote the short id from the tool's `short_id` field.** Every task in `get task` / `list tasks` / `next task` output carries a `short_id` field. When you refer to a task in prose, commits, or chat, copy that value (as `^<short>`). **Never hand-abbreviate the ULID by prefix** (`01KT6SA…`): same-session tasks share long leading runs and a prefix like `01KT6SA` collides across sibling cards. The trailing short id is collision-free.

**References resolve forgivingly** — anywhere a task id is accepted (`get`/`move`/`complete`/`update` task, `depends_on`, the `^` filter atom) you may pass any of:

| Input | Resolves by |
|-------|-------------|
| `01KT6SA4911JQPK09YQRC9RB4G` | full ULID — the stored identity |
| `rc9rb4g` | exact short id (the canonical suffix) |
| `^rc9rb4g` | short id with the `^` sigil |
| `01KT6SAM` | unique ULID prefix (git-style) |

Matching is case-insensitive, and the canonical forms win: a full ULID or exact short id always beats a colliding prefix interpretation. A prefix that matches more than one task **does not resolve** — the tool reports the reference as not found (it does not list the matches), so disambiguate by quoting the full 7-char short id. A prefix only works when it is long enough to be unique on the board; the short same-session prefixes (e.g. `01KT6SA`) that this feature exists to avoid are exactly the ambiguous ones. Display is always the short form.

**Example** — the same task, two ways to name it:

- Full ULID (stored identity): `01KT6SA4911JQPK09YQRC9RB4G`
- Short id (what you write): `^rc9rb4g`

Both resolve to that one task; write the short id.
