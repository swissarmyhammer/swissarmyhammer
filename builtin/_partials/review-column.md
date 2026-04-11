---
title: Review Column
description: How to ensure the `review` kanban column exists between `doing` and the terminal column
partial: true
---

## Ensure the Review Column Exists

The review workflow requires a column with id `review` and name `Review` positioned immediately before the terminal column (conventionally `done`). Both `implement` and `review` must ensure this column exists before moving cards.

This procedure is **idempotent** — run it every time; it is a no-op when the column is already in place.

### Procedure

1. List existing columns:

   ```json
   {"op": "list columns"}
   ```

2. If any column has `id: "review"`, stop — nothing to do.

3. Otherwise find the terminal column (the one with the highest `order` — conventionally `done`). Remember its id as `<terminal_id>` and its current order as `<terminal_order>`.

4. Bump the terminal column out of the way by one position:

   ```json
   {"op": "update column", "id": "<terminal_id>", "order": <terminal_order + 1>}
   ```

5. Insert the review column at the vacated position:

   ```json
   {"op": "add column", "id": "review", "name": "Review", "order": <terminal_order>}
   ```

The resulting column order is: `... → doing → review → done` (or whatever the terminal column is).
