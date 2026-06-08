---
assignees:
- claude-code
depends_on:
- 01KTMK4WAWRXPB094VWB1WF7D0
- 01KTMK56XGB7B46ATYZ65HZ2YE
position_column: todo
position_ordinal: 9b80
project: local-review
title: 'Docs: document `--for review` and `review.model`'
---
## What
Document the new per-tool review model selection so it's discoverable.

- Update the `sah model` command help/description (the `model` command's `description.md` and the `Use` subcommand help text in `apps/swissarmyhammer-cli/src/cli.rs`) to describe `--for review` and that the global default applies when unset.
- Update the doc comment on `wire_review_factories` / `review_concurrency` area in `apps/swissarmyhammer-cli/src/commands/serve/mod.rs` to mention the `review.model` config key alongside `review.concurrency`.
- If there is user-facing config documentation that lists `review.concurrency`, add `review.model` next to it.

Keep wording concrete and free of internal jargon — name the actual config key (`review.model`) and the actual command (`sah model use <name> --for review`).

## Acceptance Criteria
- [ ] `sah model use --help` documents `--for review`.
- [ ] The `review.model` config key is documented wherever `review.concurrency` is.
- [ ] Doc comments in `serve/mod.rs` reference `review.model`.

## Tests
- [ ] `cargo build -p swissarmyhammer-cli` succeeds (any `include_str!`'d description.md compiles).
- [ ] If a test asserts on help/description text, update it; otherwise a doc-only change — verify with the existing `DESCRIPTION` test in the model command module if present.

## Workflow
- Documentation task; no `/tdd` required. Verify the build and any description-text tests stay green.