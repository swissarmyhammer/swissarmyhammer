---
assignees:
- claude-code
depends_on:
- 01KTMK4WAWRXPB094VWB1WF7D0
- 01KTMK56XGB7B46ATYZ65HZ2YE
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff8d80
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

## Review Findings (2026-06-08 22:45)

Acceptance criteria are all satisfied by the live binary and docs (verified: build exit 0; `sah model use --help` prints `--for review` + `review.model` + the global-default fallback; serve/mod.rs doc comments reference `review.model`; model CLI parsing tests green). The findings below are about a divergent second copy of the help text and do not block the criteria.

### Warnings
- [x] `apps/swissarmyhammer-cli/src/cli.rs` (`ModelSubcommand::Use` `long_about`) — There are now two copies of the `model use` long help: the live `sah` binary renders `MODEL_USE_LONG_ABOUT` in `dynamic_cli.rs` (updated, documents `--for review` and `review.model`), but the static clap `long_about` on `ModelSubcommand::Use` in `cli.rs` was NOT updated and still never mentions `--for review`/`review.model`. The task body explicitly named `cli.rs` as a place to update. The integration test `test_model_use_help_text_content` (tests/integration/model_cli_parsings.rs) parses via `Cli::try_parse_from`, so it exercises the *stale* `cli.rs` text and only asserts generic phrases — nothing guards the live help. Fix: either point the static `long_about` at the same shared constant as `dynamic_cli.rs` (single source of truth) so the two cannot drift, or at minimum add the `--for review` paragraph to the `cli.rs` block. Strengthen the test to assert `--for review` so the live-facing text is covered.
  - RESOLVED (2026-06-08): Made `MODEL_USE_LONG_ABOUT` the single source of truth in `cli.rs` (the module `build.rs` compiles standalone, so the constant must live there rather than in `dynamic_cli.rs`). `dynamic_cli.rs` now `use`s `crate::cli::MODEL_USE_LONG_ABOUT`, and the static derive `long_about` references it directly — the two copies are now one constant and cannot drift. Strengthened `test_model_use_help_text_content` to assert the help contains both `--for review` and `review.model`. Verified: `cargo build -p swissarmyhammer-cli` exit 0 (incl. build-script standalone compile of cli.rs), `cargo test -p swissarmyhammer-cli model` 89 passed / 0 failed, live `sah model use --help` renders `--for review` + `review.model`.

### Nits
- [x] `doc/src/concepts/validators.md` (line 11, "Built-in Validators") — Pre-existing text still says SwissArmyHammer "ships with four validator sets", but the builtin tree has many more (dead-code, duplication, reuse, data-driven, complexity, function-length, magic-numbers, missing-docs, naming, no-commented-code, injection, etc.). Not introduced by this task, but the surrounding sections were rewritten right next to it, making the stale count more conspicuous. Consider updating or softening the count.
  - RESOLVED (2026-06-08): Replaced the stale "four validator sets" count with an accurate, drift-free description: the groups shown are illustrative rather than exhaustive, and the authoritative list is the builtin validator tree itself.

Notes (not findings):
- The implementer's decision to leave `doc/src/reference/sah-cli.md` unedited is correct: `build-support/doc_gen.rs` regenerates it via `clap-markdown` from the same dynamic command tree (`MODEL_USE_LONG_ABOUT`), so it is genuinely a separate doc-gen run, not a gap in this task.
- The validators.md changes are broader than the scoped "new section": the implementer also rewrote the What/How/Setting Up/Creating/Locations sections to drop the obsolete Claude-Code-hook / `avp init` / `.avp/validators/` framing. This is corrective accuracy work (the old text described validators as live hooks, which no longer matches the rules-as-data review-pipeline architecture) and the new wording is accurate; flagged only for visibility, not as a defect.