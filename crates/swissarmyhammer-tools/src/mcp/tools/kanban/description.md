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
