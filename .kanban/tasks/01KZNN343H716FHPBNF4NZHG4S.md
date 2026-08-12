---
assignees:
- claude-code
position_column: todo
position_ordinal: ffb080
title: 'kanban tag task: an array of tags becomes one hyphen-joined tag'
---
`tag task` does not split an array. It joins the array into one tag name.

## What happened

```
tag task { task: 01KZKCPTPD45XYMC34KN8PTDXB, tag: ["tool-validators", "objectivity"] }
```

The call reported `ok: true`. It created **one** tag named `tool-validators-objectivity` and applied that tag to the task. It did not apply `tool-validators`. It did not apply `objectivity`.

The tool contract says the opposite. It says the `tags` field takes a single tag, a JSON array, or a stringified JSON array, and that a bad reference is an error and never a silent no-op.

## A second defect in the same call

`tag task` does not accept `tags` at all:

```
tag task { task: <id>, tags: ["tool-validators", "objectivity"] }
→ MCP error -32603: tag task: parse error: missing required field: tag
```

Only `add task` and `update task` accept `tags`. The documentation gives `tag` as a one-element alias for `tags`, so the two fields must both work on `tag task`, and both must split an array.

## The blast radius

Run `list tags`. The board holds 218 tags. Many are wreckage from this defect class:

`1,`  `2,`  `3,`  `fi`  `f`  `4):`  `2:`  `BLOCK"`  `BLOCKED"`  `init-doctor):`  `[serial(cwd)]);`  `15012](https://github.com/ggml-org/llama.cpp/issues/15012))`

These are fragments of prose and of code, not tags. Each one is a silent write of a tag the caller did not ask for.

This is the same defect class as ^4t0ke4q, where `depends_on` silently drops a stringified array.

## Work

- Make `tag task` accept `tags` and `tag`, and make both split an array into one tag for each element.
- Make an unresolvable reference an error, not a silent write of a joined name.
- Add a test that applies two tags in one call and asserts two tags on the task.
- Add a test that asserts no tag name holds a comma, a quotation mark, a bracket, or a colon.
- Sweep the 218 tags on this board. Delete every fragment tag and correct the tasks that carry one.

## Done when

One call applies two separate tags, and the sweep leaves no fragment tag on the board. #bug #kanban