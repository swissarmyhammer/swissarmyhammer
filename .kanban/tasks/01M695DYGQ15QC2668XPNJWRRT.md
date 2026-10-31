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
position_column: doing
position_ordinal: '8480'
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