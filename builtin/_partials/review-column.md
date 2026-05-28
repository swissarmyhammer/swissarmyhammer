---
title: Review Column
description: How to ensure the `review` kanban column exists between `doing` and the terminal column
partial: true
---

## Ensure the Review Column Exists

The review workflow needs a column `id: "review"`, `name: "Review"` positioned immediately before the terminal column (conventionally `done`). Both `implement` and `review` must ensure it exists before moving tasks.

**Idempotent** — run every time; a no-op when already in place.

### Procedure

1. List columns: `{"op": "list columns"}`
2. If any has `id: "review"`, stop.
3. Find the terminal column (highest `order` — usually `done`). Call its id `<terminal_id>` and order `<terminal_order>`.
4. Shift terminal out: `{"op": "update column", "id": "<terminal_id>", "order": <terminal_order + 1>}`
5. Insert review: `{"op": "add column", "id": "review", "name": "Review", "order": <terminal_order>}`

Result: `... → doing → review → done`.
