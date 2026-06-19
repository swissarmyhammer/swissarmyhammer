---
title: Review Column
description: How to ensure the `review` kanban column exists between `doing` and the terminal column
partial: true
---

## Ensure the Review Column Exists

The review workflow needs a column `id: "review"`, `name: "Review"` ordered **after `doing` and immediately before the terminal column** (conventionally `done`) — the board reads `todo → doing → review → done`. Column position is set by the integer `order` field: `review` must have an `order` greater than `doing`'s and less than `done`'s. 


### Procedure

1. List columns: `{"op": "list columns"}` — read each column's `order`.
2. If any column has `id: "review"`, stop — it already exists.
3. Find the terminal column: the one with the highest `order` (conventionally `done`). Call its id `<terminal_id>` and order `<terminal_order>`. `doing` sits at a lower order than this.
4. Shift the terminal column out by one to free its slot: `{"op": "update column", "id": "<terminal_id>", "order": <terminal_order + 1>}`
5. Insert review into the freed slot: `{"op": "add column", "id": "review", "name": "Review", "order": <terminal_order>}`

This places `review` at `<terminal_order>`: above every earlier column (so **after `doing`**) and below the just-shifted terminal (so **before `done`**). Result: `todo → doing → review → done`.
