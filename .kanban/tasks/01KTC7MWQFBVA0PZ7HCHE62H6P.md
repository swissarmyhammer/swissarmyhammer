---
assignees:
- claude-code
depends_on:
- 01KTC7HAF68EAYYV9ZMEB0NGX2
- 01KTC7K7GVZ8FXQ4D16ANM0FNJ
- 01KTC7KR16K7KEWDDDXBQT5S13
- 01KTC7M92FTPD6T76Z4TW4HQV3
- 01KTM2Q4K959W6774Y7EY0XF7K
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffffb80
project: remove-prompts
title: 'Final verification: workspace builds, tests green, no prompt surface remains'
---
## What
Verify the prompt feature is fully excised and nothing skills/workflows/agents depend on was broken. This is the closing gate — no new code beyond fixing fallout the verification surfaces.

NOTE: actual workspace for this work is `/Users/wballard/github/swissarmyhammer/swissarmyhammer-prompts` (task originally said `.../swissarmyhammer`). All commands run there.

Checks to run from repo root:
- `cargo build --workspace --all-targets` — must succeed.
- `cargo clippy --workspace --all-targets` — no new warnings about unused prompt imports/dead code.
- Full suite: `cargo nextest run --workspace` (or `cargo test --workspace`) — green.
- `cargo run -p swissarmyhammer-cli -- --help` — confirm no `prompt` subcommand and no "managing prompts" tagline.
- Residue grep: `grep -rn "swissarmyhammer[_-]prompts" crates apps --include=Cargo.toml` returns nothing (crate fully renamed/merged).
- Source residue grep: `grep -rIn "PromptLibrary\|PromptLoader\|PromptResolver\|PromptFilter\|builtin/prompts\|is_prompt_visible" crates apps` — only expected survivors (the renamed render library type, if the rename task chose to keep the type name) remain; nothing references the removed concepts.
- Confirm skill rendering still works end-to-end: run the skill `use` integration test and an agent render test.
- Confirm `builtin/_partials/` still loads (skills/agents partials intact).

If any check fails, fix the specific fallout (likely a missed import, a leftover re-export, or a doc grep guard) — do not expand scope.

## Acceptance Criteria
- [x] `cargo build --workspace --all-targets` exits 0. (verified: `Finished dev profile in 57.39s`, EXIT 0)
- [x] `cargo nextest run --workspace` (or `cargo test --workspace`) is fully green. (`14681 tests run: 14681 passed (101 slow), 2 skipped`, EXIT 0)
- [x] `sah --help` shows no `prompt` command. (Commands list has no `prompt`; only mention of word is `validate`'s description "Validate prompt files and skills")
- [x] No `Cargo.toml` references `swissarmyhammer-prompts`. (grep returned empty, exit 1)
- [x] Skills, agents, and `{% include %}` partials still render (proven by their integration tests passing). (skill_e2e use tests + skills_rendering_test agent test + all_skills_render_test all PASS; builtin/_partials/ present and wired via build.rs)

## Tests
- [x] Final `cargo nextest run --workspace` summary line: `Summary [ 334.332s] 14681 tests run: 14681 passed (101 slow), 2 skipped` — EXIT 0. No failures; no flake re-runs needed.
- [x] `sah --help` output captured: Commands = serve, init, deinit, doctor, validate, model, agent, statusline, completion, tool, help. No `prompt` subcommand, no "managing prompts" tagline. Tagline = "SwissArmyHammer - The only coding assistant you'll ever need".
- [x] Residue grep outputs:
  - `grep -rn "swissarmyhammer[_-]prompts" crates apps --include=Cargo.toml` → EMPTY (exit 1, no matches).
  - Source grep `PromptLibrary|PromptLoader|PromptFilter|builtin/prompts|is_prompt_visible` → only expected survivors: `is_prompt_visible` (swissarmyhammer-common + claude-agent/llama-agent live MCP/slash-command callers, deliberately KEPT) and two history doc comments in avp-common about the old `builtin/prompts` path (templates now embedded via include_str!). No `PromptLibrary`/`PromptLoader`/`PromptFilter` hits at all.
- [x] clippy `--workspace --all-targets` produced zero warning/error lines (EXIT 0).
- [x] `builtin/_partials/` loads: dir present (architecture-awareness, coding-standards, skills, validators, etc.), wired in templating build.rs `source_dir("../../builtin/_partials")`.

## Workflow
- Use `/really-done` — run every verification command fresh and paste evidence before declaring complete. No completion claim without output. DONE — all evidence captured fresh in this run.