---
assignees:
- claude-code
depends_on:
- 01KTC7HAF68EAYYV9ZMEB0NGX2
- 01KTC7K7GVZ8FXQ4D16ANM0FNJ
- 01KTC7KR16K7KEWDDDXBQT5S13
- 01KTC7M92FTPD6T76Z4TW4HQV3
position_column: todo
position_ordinal: '8880'
project: remove-prompts
title: 'Final verification: workspace builds, tests green, no prompt surface remains'
---
## What
Verify the prompt feature is fully excised and nothing skills/workflows/agents depend on was broken. This is the closing gate — no new code beyond fixing fallout the verification surfaces.

Checks to run from repo root `/Users/wballard/github/swissarmyhammer/swissarmyhammer`:
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
- [ ] `cargo build --workspace --all-targets` exits 0.
- [ ] `cargo nextest run --workspace` (or `cargo test --workspace`) is fully green.
- [ ] `sah --help` shows no `prompt` command.
- [ ] No `Cargo.toml` references `swissarmyhammer-prompts`.
- [ ] Skills, agents, and `{% include %}` partials still render (proven by their integration tests passing).

## Tests
- [ ] Capture and paste the final `cargo nextest run --workspace` summary line (N passed; 0 failed).
- [ ] Capture the `sah --help` output showing no prompt subcommand.
- [ ] Capture the residue grep outputs (empty).

## Workflow
- Use `/really-done` — run every verification command fresh and paste evidence before declaring complete. No completion claim without output.