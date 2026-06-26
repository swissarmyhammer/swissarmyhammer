---
assignees:
- claude-code
depends_on:
- 01KW262DJSSC1JX2FXCDQN0X4E
- 01KW265D8SHMBFYBCZ5QEMBVQ0
position_column: todo
position_ordinal: bf80
project: expect
title: db + file surface adapters
---
## What
Add the two state surfaces: `db` and `file`. Per `ideas/expect.md` §"Surface adapters" (db/file rows) and the locator table.

- New `crates/swissarmyhammer-expect/src/surface/db.rs` (`SurfaceAdapter`):
  - Provision: create a fresh database + load a fixture (per `setup:`); teardown drops it.
  - Drive: run statements; Observe: capture rows/tables. Locator: a SQL query + projection (the locator IS SQL — very stable). Use an in-process Rust DB client (reuse the workspace's sqlite/sqlx if present).
- New `crates/swissarmyhammer-expect/src/surface/file.rs` (`SurfaceAdapter`):
  - Provision: a scratch dir. Drive: write. Observe: files/dirs/content. Locator: path + content (+ sub-locator if structured, e.g. json-path into a file).
- Extend the assertion compiler locator resolution for db (SQL projection) and file (path+content).

## Acceptance Criteria
- [ ] db adapter provisions a fresh DB, runs statements, and a SQL-projection locator observes the expected rows.
- [ ] file adapter writes to a scratch dir and a path+content locator (incl. a structured sub-locator) observes file state.
- [ ] Both are deterministic, run once by default; teardown cleans up.

## Tests
- [ ] db integration test against an in-memory/temp sqlite fixture: statement → SQL-projection observation.
- [ ] file integration test in a tempdir: write → path+content + json-sub-locator observation.
- [ ] `cargo nextest run -p swissarmyhammer-expect db file` passes.

## Workflow
- Use `/tdd`.