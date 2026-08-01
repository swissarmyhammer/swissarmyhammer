Kanban board operations for task management. This is the best way to keep a TODO list for a project.

## Task dependencies

On `add task` and `update task`, `depends_on` is the canonical input for task
dependencies. It is forgiving about both shape and id format:

- Shape: a single ref, a JSON array of refs, or a stringified JSON array
  (`"[\"01K…\"]"`) all work.
- Id format: each ref may be a full ULID, a 7-char short id, `^<short>`, a
  unique ULID prefix, or lowercase — every form resolves to the canonical full
  ULID before it is stored.

An unresolvable ref is an error, not a silent no-op.

`blocked_by` is **derived** — it is the unsatisfied subset of `depends_on`
(reported by `get task`/`list tasks`) and is **not** directly settable. To
change what a task is blocked by, set `depends_on`.

## Task tags

On `add task` and `update task`, `tags` applies tags. It is as forgiving as
`depends_on`:

- Shape: a single tag, a JSON array, or a stringified JSON array all work. The
  singular `tag` is accepted as a one-element alias.
- Ref format: each entry may be a tag name, a full tag ULID, `^<short>`, or a
  7-char short id. A name that names no tag yet creates it; an **id** reference
  that names no tag is an error, not a silent no-op.

`tags` on `add task` adds to whatever `#tag` markers the description carries.
`tags` on `update task` **replaces** the whole set — an empty array clears every
tag. One `add task { tags: [a, b, c] }` gives the same result as one `add task`
plus three `tag task` calls; both run the same code.

Tags are stored as `#tag` markers in the description, so editing the description
is the other way to change them. Because of that, replacing the tag set rewrites
the description: the old markers are removed from wherever they sat and the new
ones are added at the end. The prose is kept.

## Assignees and attachments

`assignees` (both ops) and `attachments` (`update task`) take the same forgiving
input **shapes** as `tags`. Both **replace** the whole list on `update task`, so
an empty array unassigns / detaches everything. The singular `assignee` is
accepted as a one-element alias.

Each `assignees` entry is an actor id exactly as `add actor` registered it —
there is no short form or `^` sigil, because an actor id is a slug, not a ULID.
An id that names no actor is an error, not a silent no-op: the whole write is
rejected, so `add task` creates nothing and `update task` leaves the stored list
alone. Register the actor with `add actor` first.

The top-level `actor` key is not an assignee. It names the caller, and `add task`
auto-assigns it only as a convenience. An unregistered `actor` never fails the
create — it is left off the new task instead.

`attachments` entries are source file paths to attach; the metadata objects
`get task` returns are also accepted, so a task read can be sent straight back.
