---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kz2bbr569jm5z1bnte18t0yc
  text: |-
    Picked up. TDD RED step is done first, before any deletion.

    Added `apps/swissarmyhammer-cli/tests/integration/model_command_removed.rs` with two tests, modeled on the sibling `prompt_command_removed.rs` (the same card pattern was used when `sah prompt` was removed):
    - `test_model_subcommand_is_unrecognized`
    - `test_help_does_not_list_model_subcommand`

    Registered in `tests/integration/mod.rs`.

    Verified RED: `cargo nextest run -p swissarmyhammer-cli -E 'test(model_command_removed)'` gives 2 failed, 0 passed. The failure is the correct one — `sah model` currently exits 0 and prints the "Current Model" table.

    Note on the help test: it matches subcommand *lines* (`model ` at the start of a trimmed line), not a bare substring. The `--model` global flag stays for card ^hm82t0z, so its help line must not trip the assertion.

    ARCHITECTURE.md was checked: it does not document the `sah model` subcommand, so no architecture update belongs to this card.
  timestamp: 2026-08-02T23:01:21.702367+00:00
- actor: claude-code
  id: 01kz2c69758gfzd9wngb01c40p
  text: |-
    GREEN. Implementation landed.

    **Key discovery that changed the plan: `completions/`, `docs/sah.1`, and `doc/src/reference/sah-cli.md` are all machine-generated** by `apps/swissarmyhammer-cli/build.rs` from `src/cli.rs`. Do NOT hand-edit them — remove `Commands::Model` from `cli.rs` and they regenerate on the next build. `docs/sah.1` is also gitignored (`.gitignore` has `docs/*.1`). The card said to edit completions and the man page by hand; that would have been wrong and would have been overwritten.

    **Second discovery: `commands/registry.rs` has no `model` reference.** It is the init/deinit component registry (ProjectStructure + KanbanTool), not a CLI-command registry. The real registration lives in three places the card did not name: `cli.rs` (static clap derive), `dynamic_cli.rs` (runtime command tree), and `main.rs` (dispatch arm + handler).

    **Third discovery: the surface was wider than the card listed** — five whole `model_*` test files, an in-crate test asserting the subcommand set, and a synthetic clap tree in `main.rs` that named `model use`.

    ### What was removed
    - `apps/swissarmyhammer-cli/src/commands/model/` — all 6 files.
    - `cli.rs` — the `Commands::Model` variant, `pub enum ModelSubcommand`, `pub const MODEL_USE_LONG_ABOUT`, and the two `long_about` mentions.
    - `dynamic_cli.rs` — `build_model_command`, `MODEL_COMMAND_LONG_ABOUT`, the `use crate::cli::MODEL_USE_LONG_ABOUT`, the `add_content_commands` call, and two doc comments.
    - `main.rs` — the dispatch arm and `handle_model_command`.
    - `commands/mod.rs` — `pub mod model;`.
    - Five test files: `model_cli_parsings.rs`, `model_commands.rs`, `model_list_units.rs`, `model_performance_edge_casess.rs`, `model_use_case_integration.rs`, plus their `mod` lines.
    - `dynamic_cli_tests.rs` — `test_build_model_command`, and `"model"` dropped from `expected_commands`.

    ### Two dead-code warnings the deletion exposed, both fixed at the cause
    1. `parse_output_format` in `main.rs` — its only caller was `handle_model_command`. Removed.
    2. `ArgSpec::required` in `dynamic_cli.rs` — its only production user was the `sah model use <name>` positional. Removed the whole knob (builder method, struct field, initializer, and the now-unreachable `build()` branch), not just the line rustc flagged. Two pre-existing builder tests asserted on the removed field, so their `required` lines were dropped; the rest of each chain still runs.

    ### Hand-written prose corrected (not deleted)
    - `doc/src/concepts/validators.md` — told the user to set the review model with `model use`. Now says to set `review.model` in `.sah/sah.yaml`; the YAML example below it already showed that shape.
    - `scripts/review-verify/README.md` — dropped the `sah init && sah model use qwen` alternative, kept the `--model qwen` flag path.
    - `crates/swissarmyhammer-config/src/model.rs` — two doc comments named `sah model` display and `sah model set <x>` as resolution paths. Reworded to the config keys.

    ### Scope held
    `--model` global flag, `extract_model_flag`, and the `--model` runtime arg in `dynamic_cli.rs` are all untouched, as card ^hm82t0z requires. `builtin/models/*.yaml`, llama-agent, swissarmyhammer-agent, and the embedding stack were not touched.

    ### Verification
    - `cargo fmt --all` — clean.
    - `cargo clippy --workspace --all-targets -- -D warnings` — exit 0, zero warnings.
    - `cargo nextest run --workspace` — 14963 passed, 0 skipped.
    - The two new tests went RED (2 failed) before the deletion and GREEN (2 passed) after.
    - `grep -rn "sah model" doc/ docs/ completions/` — no matches.
    - `./target/debug/sah --help` — subcommand list is serve, init, deinit, doctor, validate, agent, statusline, completion, tool, help. No `model`.
    - No `model_*` test remains except the new `model_command_removed`.
    - No stray `.kanban` / `.shell` / `.sah` directories created.
  timestamp: 2026-08-02T23:15:51.141101+00:00
- actor: claude-code
  id: 01kz2cwctjnc9xq4smhvgwrt92
  text: |-
    Double-check verdict was REVISE with two findings. Both are now closed.

    **Finding 1 — the two new regression tests exercise the wrong command tree, so the card's acceptance criterion is unverified.**

    Verified and true. `run_sah_command_in_process` routes through `parse_cli_args`, which calls `swissarmyhammer_cli::cli::Cli::try_parse_from` — the clap **derive** tree in `cli.rs`. The real binary instead uses `main.rs::build_and_parse_cli` → `CliBuilder::build_cli_with_warnings()` — the **dynamic** tree in `dynamic_cli.rs`. The two trees are free to disagree, and `build_model_command`, `MODEL_COMMAND_LONG_ABOUT`, and the `add_content_commands` registration all lived in the dynamic tree. `test_build_cli_basic_structure` was the only test that builds the dynamic tree, and `assert_commands_exist` only ever asserts presence — so after swapping `"model"` for `"agent"` in that array, nothing asserted `model`'s absence from the surface the card's criterion actually names.

    Fix: added `test_build_cli_has_no_model_command` to `apps/swissarmyhammer-cli/src/dynamic_cli_tests.rs`, next to `test_build_cli_basic_structure`. It builds the CLI the same way and asserts no subcommand is named `model`.

    Proved it is not vacuous rather than assuming: temporarily injected `Command::new("model")` into `add_content_commands` and ran. The new test FAILED with `the built CLI still offers a `model` subcommand: ["serve", "init", "deinit", "doctor", "validate", "model", "agent", "statusline", "completion", "tool"]` — while the two `model_command_removed` tests **both stayed green**. That is exactly the gap the finding described, now closed. Injection reverted and confirmed gone.

    **Finding 2 — the new test file is untracked while the `mod` declaration that needs it is already staged-visible.**

    Verified and true. `tests/integration/mod.rs` was a tracked modification declaring `mod model_command_removed;`, while the file itself was `??`. A `git commit -a` or `git add -u` would have produced a tree where `cli_tests` does not compile. Fixed with `git add apps/swissarmyhammer-cli/tests/integration/model_command_removed.rs`; `git status --porcelain` now reports zero `??` entries repo-wide.

    Items the verifier independently cleared: removing `ArgSpec::required` loses no capability (production required args come from the MCP-schema path at `dynamic_cli.rs`, not `ArgSpec`); removing `parse_output_format` is correct; the `tests_relax_required_tool_args` rename does not weaken that test, because relaxation keys only on `STDIN_ARGS_SUBCOMMAND`; the prose edits are accurate, and `drive.py` really does pass `--model qwen` to `sah serve`; the five deleted test files lost no live coverage of the surviving config resolution.

    ### Re-verification after the fix
    - `cargo fmt --all` — clean.
    - `cargo clippy --workspace --all-targets -- -D warnings` — exit 0, zero warnings.
    - `cargo nextest run --workspace` — 14965 passed, 0 skipped (up from 14963 by the new test in two binaries).
    - Zero untracked files repo-wide.
  timestamp: 2026-08-02T23:27:55.730904+00:00
- actor: claude-code
  id: 01kz2cwnt5h64vgjgqrhggym8z
  text: |-
    ### implement — changed
    - evidence: 20 files. Deleted: `apps/swissarmyhammer-cli/src/commands/model/` (6 files), `tests/integration/model_{cli_parsings,commands,list_units,performance_edge_casess,use_case_integration}.rs`. Added: `tests/integration/model_command_removed.rs`. Modified: `src/cli.rs`, `src/commands/mod.rs`, `src/dynamic_cli.rs`, `src/dynamic_cli_tests.rs`, `src/main.rs`, `src/mcp_integration.rs`, `tests/integration/mod.rs`, `completions/{_sah,sah.bash,sah.fish}` (build.rs-generated), `doc/src/reference/sah-cli.md` (build.rs-generated), `doc/src/concepts/validators.md`, `scripts/review-verify/README.md`, `crates/swissarmyhammer-config/src/model.rs`. `cargo nextest run --workspace` = 14965 passed, 0 skipped; `cargo clippy --workspace --all-targets -- -D warnings` = exit 0.
    - next: ready for `/review`. Left in `doing`.
  timestamp: 2026-08-02T23:28:04.933382+00:00
- actor: claude-code
  id: 01kz2d6zwgg7bc7fn227pcn58n
  text: |-
    ### test — green
    - evidence: `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` exit 0, 0 warnings; `cargo nextest run --workspace` — 14965 passed, 0 failed, 0 skipped.
    - Scope check: `crates/swissarmyhammer-config/src/model.rs` diff is doc-comment only (removes stale references to `sah model`/`sah model set`), no logic change. `--model` global flag and `extract_model_flag` in `main.rs` untouched. No diffs in `builtin/models/`, `crates/llama-agent`, `crates/swissarmyhammer-agent`, or the embedding stack. No scope violation.
    - Generated-file check: `apps/swissarmyhammer-cli/build.rs` regenerates `completions/`, `docs/sah.1`, `doc/src/reference/sah-cli.md` from `src/cli.rs::Cli::command()`. Forced a rebuild (`touch cli.rs && cargo build -p swissarmyhammer-cli`); `git diff --stat` on those paths before and after the forced rebuild was identical — the committed generated output matches a fresh build exactly.
    - Binary check: `sah --help` lists no `model` subcommand (only the `--model` global flag remains). `sah model` exits 2 with `error: unrecognized subcommand 'model'`.
    - next: ready for review.
  timestamp: 2026-08-02T23:33:42.928315+00:00
- actor: claude-code
  id: 01kz2d9y3zmp39cftfyc7fc72d
  text: |-
    ### finish iteration 1 — done
    - implement: changed — 20 files. Deleted `commands/model/` (6 files) and 5 `model_*` test files; added `tests/integration/model_command_removed.rs`.
    - test: green — cargo fmt --all --check clean; cargo clippy --workspace --all-targets -D warnings 0 warnings; cargo nextest run --workspace 14965 passed / 0 failed / 0 skipped; forced build.rs rebuild produced byte-identical generated output; `sah --help` shows no model subcommand; `sah model` exits 2 with "unrecognized subcommand".
    - commit: 990f24b71 feat(cli): remove the `sah model` command
    - review: SKIPPED by user instruction for every card in the drop-llama-agent project this session.
    - Scope verified clean: the `crates/swissarmyhammer-config/src/model.rs` edit is doc-comment only (stale `sah model` references in `review_agent_name_from` docs). The `--model` flag, `extract_model_flag`, all `builtin/models/*.yaml`, llama-agent, swissarmyhammer-agent, and the embedding stack are untouched for card ^hm82t0z.
    - THREE CARD ERRORS worth carrying to the remaining cards: (1) `completions/`, `docs/sah.1`, and `doc/src/reference/sah-cli.md` are GENERATED by `apps/swissarmyhammer-cli/build.rs` from `src/cli.rs` — hand-editing them gets overwritten; (2) `commands/registry.rs` had no `model` entry, registration lived in `cli.rs`, `dynamic_cli.rs`, and `main.rs`; (3) the regression test must target the DYNAMIC clap tree in `dynamic_cli.rs`, not the derive tree in `cli.rs`, because that is what the real binary parses.
    - Two dead-code warnings the deletion exposed were fixed at the cause: `parse_output_format` in main.rs and `ArgSpec::required` in dynamic_cli.rs.
    - next: card 3 of the project, ^6s0py85 — drop the llama executor branch from swissarmyhammer-agent.
  timestamp: 2026-08-02T23:35:19.423196+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffff9180
project: drop-llama-agent
title: Remove the `sah model` CLI command
---
## What

Delete the `sah model` command. It exists to pick between chat models, and
Claude becomes the only chat executor.

- Delete `apps/swissarmyhammer-cli/src/commands/model/` — `mod.rs`,
  `list.rs`, `show.rs`, `use_command.rs`, `display.rs`, `description.md`
  (1873 lines).
- Remove the `model` entry from
  `apps/swissarmyhammer-cli/src/commands/registry.rs` and the `mod model;`
  declaration in `apps/swissarmyhammer-cli/src/commands/mod.rs`.
- Remove the `model` subcommand from the shell completions under
  `completions/` and from the man page `docs/sah.1`.
- Remove the `sah model` section from the book under `doc/src/`. Find it with
  `grep -rn "sah model" doc/ docs/ completions/`.

Keep the `--model` global flag handling in
`apps/swissarmyhammer-cli/src/main.rs` (`extract_model_flag`, line 290) for
now. The follow-up card that collapses the chat model configuration decides
its fate, and removing it here would break that card's callers.

## Corrections found during implementation

Two items above are wrong. Recorded here so the next reader does not repeat them.

- `completions/`, `docs/sah.1`, and `doc/src/reference/sah-cli.md` are
  **machine-generated** by `apps/swissarmyhammer-cli/build.rs` from
  `src/cli.rs`. Do not hand-edit them. Remove `Commands::Model` from `cli.rs`
  and they regenerate on the next build. `docs/sah.1` is gitignored.
- `commands/registry.rs` has **no** `model` reference; it is the init/deinit
  component registry, not a CLI-command registry. Registration actually lives
  in `cli.rs` (static clap derive), `dynamic_cli.rs` (runtime command tree),
  and `main.rs` (dispatch arm plus handler).

### Subtasks

- [x] Delete the `commands/model/` directory.
- [x] Remove the registration and the module declaration.
- [x] Remove the completions, man page, and book entries.
- [x] Confirm no reference remains.

## Acceptance Criteria

- [x] `apps/swissarmyhammer-cli/src/commands/model/` does not exist.
- [x] `grep -rn "sah model" doc/ docs/ completions/` returns nothing.
- [x] `sah --help` does not list a `model` subcommand.
- [x] `cargo clippy -p swissarmyhammer-cli --all-targets -- -D warnings` exits
      0 with zero warnings.

## Tests

- [x] Add a test in `apps/swissarmyhammer-cli/tests/` that runs the CLI with
      `--help` and asserts the output does NOT contain a `model` subcommand.
      Follow the pattern of the existing CLI tests in that directory.
- [x] Add a test that running `sah model` exits non-zero with an
      unknown-subcommand error.
- [x] Run `cargo nextest run -p swissarmyhammer-cli` — all tests pass and no
      test named `model_*` remains.

## Workflow

- Use `/tdd` — write the two CLI-surface tests first. They fail while the
  command still exists, and pass after it is deleted. #llama-agent #cli #cleanup