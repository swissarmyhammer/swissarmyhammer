---
title: Card Report
description: The one way to show a kanban card to the user — read the card first, then render the fixed block, and never report from memory
partial: true
---

## Show the Card

Report from the card, not from memory. Read the card first:

```json
{"op": "get task", "id": "<id>"}
{"op": "list comments", "task_id": "<id>"}
```

Then show this block. Keep the order of the fields. Omit a line only when the
field is empty.

```
^<short> — <title>
column: <column> · project: <project> · tags: <tag>, <tag>
subtasks: <checked>/<total>
findings: <open> open of <total>
last: <step> — <outcome>
```

- `^<short>` — copy the `short_id` field from the tool output. Do not shorten
  the ULID yourself.
- `subtasks` — count the `- [x]` and `- [ ]` items in the description.
- `findings` — count the unchecked items in the `## Review Findings` sections.
- `last` — the newest step record in the comments.

Show the block at three moments:

1. when you pick up a card,
2. when a step changes the card,
3. when you report the result to the user.

Put the prose after the block. Keep the prose to the facts: what changed, what
the tests said, and what is still open. The user reads the block for the state,
and the prose for the reason.
