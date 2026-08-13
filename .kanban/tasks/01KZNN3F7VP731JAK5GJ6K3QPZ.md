---
assignees:
- claude-code
position_column: todo
position_ordinal: ffb180
title: 'kanban list tasks: the tag parameter is ignored and returns the whole board'
---
`list tasks` accepts a `tag` parameter and then does not filter on it.

## What happened

```
list tasks { tag: "objectivity" }
→ count: 10, total: 71
```

71 is the whole board. The result holds tasks with `tags: []`, which cannot carry `objectivity`. The same call with `tag: "tool-validators"` returned 68 the same way.

The filter expression is correct, and gives the answer the `tag` parameter should give:

```
list tasks { filter: "#tool-validators and #READY" }
→ count: 8, total: 8
```

## Why it matters

The failure is silent. The call returns `ok` with a large result, so a caller reads the whole board as the answer to a narrow question. A caller that reads only the first page draws a conclusion from 10 unrelated tasks.

## Work

- Make `list tasks` honor `tag`. Translate it to the filter atom `#<name>`, so one code path answers both.
- If `tag` is to be dropped instead, reject it with an error. Never accept a parameter and ignore it.
- Add a test that gives `tag` a name held by 3 of 20 tasks and asserts a count of 3.
- Check `search tasks` for the same defect. Its `tag` parameter takes the same shape.

## Done when

`list tasks { tag: X }` and `list tasks { filter: "#X" }` return the same tasks. #bug #kanban