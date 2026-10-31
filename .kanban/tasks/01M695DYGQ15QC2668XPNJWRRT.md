---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m69v2xs83kwgapzqbkgjwf5g
  text: |-
    ### implement — changed
    - evidence: Deleted builtin/skills/shell/ and crates/swissarmyhammer-skills/tests/shell_output_guidance.rs (it tested only the shell skill). Edited builtin skills check-sah, coverage, lsp, sah-help, test and builtin/validators/duplication/rules/duplication.md (the example now names kanban/mod.rs init and deinit). Edited README.md and doc/src (agents, integrated-sdlc, skills, tools, quick-start, introduction) to remove the shell skill, the shell tool and the Smart Shell section. Edited apps/swissarmyhammer-cli/tests/in_process_test_utils.rs (removed "shell" from the subprocess list, and fixed the nine review findings). Edited crates/mirdan/src/tool_install.rs: the fake tool now deploys the task skill, and a new test all_skills_install_deploys_the_builtin_set_without_the_removed_skill deploys every builtin skill and proves the shell skill is not deployed.
    - checks: cargo check --workspace --tests clean. cargo nextest run --override-version-check -E 'rdeps(swissarmyhammer-skills) | rdeps(swissarmyhammer-validators) | rdeps(mirdan)': 8530 passed, 0 failed. cargo fmt --check clean. cargo clippy --workspace --all-targets -- -D warnings clean. review file on tool_install.rs: 0 findings. review file on in_process_test_utils.rs: 9 findings, all fixed (derives, lowercase error text, named exit code, anyhow context, one CapturedOutput argument, canonical-copy comment).
    - left alone, out of this card: standards/mcp.md and ideas/ use the shell tool as a reference example; doc/src/concepts/closed-write-surface.md discusses the general shell; crates/swissarmyhammer-directory ShellConfig (XDG_NAME shell) is a leftover of the deleted crate; crates/swissarmyhammer-mcp-proxy tests use the string shell as an arbitrary denied tool name. Consider a follow-up card for ShellConfig and standards/mcp.md.
    - next: /review
  timestamp: 2026-10-31T20:40:43.560429+00:00
- actor: claude-code
  id: 01m69v8bfrd4fdfyqsspgzkb0r
  text: |-
    ### test — green
    - evidence: cargo nextest run --workspace --no-fail-fast: 13943 run, 13943 passed, 0 failed, 0 skipped. cargo clippy --workspace --all-targets -- -D warnings: exit 0, zero warnings. cargo fmt --check: exit 0.
    - next: review
  timestamp: 2026-10-31T20:43:41.432976+00:00
- actor: claude-code
  id: 01m69vckmvqeyfgv3kegye8td1
  text: |-
    ### review — findings
    - evidence: 1 finding — apps/swissarmyhammer-cli/tests/in_process_test_utils.rs:229. Hand-check of skills, validators and docs: zero dangling shell-tool references.
    - next: implementer fixes the open finding, then review runs again.
  timestamp: 2026-10-31T20:46:00.859052+00:00
- actor: claude-code
  id: 01m69vd5dj97yzcnj8m8zz8vja
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 17 files, shell skill deleted, docs and skills cleaned
    - test: green — nextest workspace 13943 passed, clippy and fmt clean
    - commit: 44b3e9b20
    - review: findings — apps/swissarmyhammer-cli/tests/in_process_test_utils.rs:229
  timestamp: 2026-10-31T20:46:19.058625+00:00
- actor: claude-code
  id: 01m69vnwy9btctx45wycc5gxsr
  text: |-
    ### implement — changed
    - evidence: One file changed: apps/swissarmyhammer-cli/tests/in_process_test_utils.rs. Finding code-hygiene/magic-numbers is fixed and flipped to [x]. SUBPROCESS_TIMEOUT_SECS (60) now feeds both Duration::from_secs and the format! message.
    - same cause removed from the whole file: named constants EXIT_CANNOT_EXECUTE (126), EXIT_COMMAND_NOT_FOUND (127), EXIT_TIMED_OUT (124), EXIT_GENERAL_FAILURE (1), SAH_BINARY_NAME ("sah", was 4 copies), SUBPROCESS_DEBUG_PREFIX (was 5 copies). Bare 0 and 2 now use the shared EXIT_SUCCESS and EXIT_ERROR from swissarmyhammer_cli::exit_codes (one import at the top replaces three inner imports).
    - review file on the whole file: first run gave 9 findings, all fixed: CapturedOutput derives Hash and Default; error chains use {e:#} at 4 sites (result_to_captured, execute_in_process, subprocess spawn error, handle_validate_command); new helper run_with_dir_and_context removes the duplicate body of the two public run functions; execute_in_process and parse_cli_args now call capture_error. Second run: 0 findings.
    - checks (run once at the end): cargo check --workspace --tests exit 0; cargo nextest run --override-version-check -p swissarmyhammer-cli: 702 run, 702 passed, 0 skipped; cargo fmt --check clean; cargo clippy --workspace --all-targets -- -D warnings clean.
    - next: /review
  timestamp: 2026-10-31T20:51:05.289441+00:00
- actor: claude-code
  id: 01m69vqj2s6wkj34z4qcsd3hq2
  text: |-
    ### test — green
    - evidence: cargo nextest run --override-version-check -E 'rdeps(swissarmyhammer-cli)' — 702 run, 702 passed, 0 failed, 0 skipped. cargo clippy --workspace --all-targets -- -D warnings — exit 0, 0 warnings. cargo fmt --check — exit 0.
    - next: review
  timestamp: 2026-10-31T20:51:59.705434+00:00
- actor: claude-code
  id: 01m69vv03dvyfkvfvxdcqfqmb8
  text: |-
    ### review — clean
    - evidence: review sha HEAD~1..HEAD (commit 9088cbf07, iteration 2). Counts: findings 0, confirmed 0, refuted 0. The iteration 1 magic-number finding in apps/swissarmyhammer-cli/tests/in_process_test_utils.rs is checked and fixed.
    - next: move task to done.
  timestamp: 2026-10-31T20:53:52.365127+00:00
- actor: claude-code
  id: 01m69vvg5j09x73stp64m6tzxx
  text: |-
    ### finish iteration 2 — clean
    - implement: changed — 1 file
    - test: green — nextest swissarmyhammer-cli 702 passed, clippy and fmt clean
    - commit: 9088cbf07
    - review: clean — zero new findings, prior finding checked, task done
  timestamp: 2026-10-31T20:54:08.818393+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffffcc80
project: remove-shelltool
title: Remove shell references from skills and validators
---
Edit the source in `builtin/`. Never edit `.skills/`; it is generated.

Work:
- Delete `builtin/skills/shell/` (the `shell` skill).
- `builtin/skills/sah-help/SKILL.md:70`: remove `/shell` from the Utilities list.
- `builtin/skills/check-sah/SKILL.md`: remove `shell` from the `compatibility` line (line 5), the `shell` error line (116), and the `shell security check failed` and `detect projects mis-routed` text (142). Keep the other text.
- `builtin/skills/lsp/SKILL.md:50`: change "Run approved installs via `shell`" to "via Bash".
- `builtin/skills/test/SKILL.md:24`: the skill says to save output and read it with `get lines` and `grep history`. Those ops go away. Change it to: redirect the output to a file in the scratchpad, then read or search that file. Keep the rule "do not run the tests again".
- `builtin/skills/test/SKILL.md:93`: replace "the `shell` tool's `timeout`" with a shell `timeout` command.
- `builtin/validators/duplication/rules/duplication.md:60`: it names `tools/shell/mod.rs` as an example. Use another example file.
- Search all of `builtin/` and `doc/` again for `execute command`, `grep history`, `get lines`, `list processes`, `kill process` and `mcp__sah__shell`.
- Check `crates/swissarmyhammer-skills/tests/shell_output_guidance.rs`. Delete it if it only tests the shell skill.
- Fix the fixtures in `FoundationModelsExtras/Tests/.../skills/shell/SKILL.md` only if the Rust change breaks them.

Done when: no skill or validator names the shell tool, and `cargo test -p swissarmyhammer-skills` passes. #remove-shell

## Review Findings (2026-10-31 15:44)

> Scope: `review sha HEAD~1..HEAD` — reviewed the diffs only — lines this change added or modified. 3 file(s) reviewed, 18 not reviewed.

> 14 markdown files were not reviewed: no validator matches them. A clean report on them means nothing could look. The reviewer checked the edited skill and doc files by hand with a search for `get lines`, `grep history`, `execute command`, `/shell`, `list processes`, `kill process`, `mcp__sah__shell`, `` `shell` `` and `shelltool` under `builtin/` and `doc/`. The search found zero matches. The remaining uses of the word "shell" in `builtin/skills/` mean the OS shell and are correct.

- [x] `apps/swissarmyhammer-cli/tests/in_process_test_utils.rs:229` `code-hygiene/magic-numbers` — The timeout value 60 is written twice: in the message string here and in `Duration::from_secs(60)` at unchanged line 223. If the timeout changes, the message reports the wrong duration. Define one `const SUBPROCESS_TIMEOUT_SECS: u64 = 60;` and use it in `Duration::from_secs(...)`. Build the message with `format!` from the same constant.
