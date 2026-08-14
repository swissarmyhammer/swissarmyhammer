---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m00vrxbs37qawjjys64g5kef
  text: |-
    Research: searched the whole repo, not only the three places the card names. Found 11 text references to `unused-code-go` plus the 3 files to rename.

    The card names `builtin/validators/README.md`. That file holds NO reference — it never named the rule. The card's list was otherwise short by 5 places:

    - `builtin/validators/code-hygiene/fixtures/go.mod.tmpl` — the header comment names the rule the module serves.
    - `builtin/validators/code-hygiene/rules/dead-code.md` — the fallback rule lists its six siblings by name.
    - `crates/swissarmyhammer-validators/src/builtin/mod.rs` — `CODE_HYGIENE_DEAD_CODE_TOOL_RULES` roster.
    - `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs` — `SHIPPED_RULES_THAT_READ_A_GO_FILE`, a SORTED roster, so the entry moves from after `no-commented-code` to after `dead-code`.
    - `crates/mirdan/src/builtin_validators.rs` — `CODE_HYGIENE_FIXTURES`, a fixture roster by filename.

    Checked and found clean, so nothing to change:
    - `crates/mirdan/retired-validators/` — the byte-frozen snapshots carry no `unused-code-go`. Untouched.
    - `missing-docs-go.md` — it states a COUNT of 25 rules that read a `.go` file, not the names. A rename does not move that count.
    - `builtin/validators/README.md` — no reference.

    `unused-code-go` still appears in `.kanban/` task records (this card's own title among them) and in gitignored runtime logs (`.sah/mcp.*.log`, `.shell/log`). Those are history, not references to the rule.
  timestamp: 2026-08-14T19:25:23.193604+00:00
- actor: claude-code
  id: 01m00vs7dxf0c6beyxwew4ww5g
  text: |-
    ### implement — changed
    - evidence: 9 files. 3 renamed with `git mv` — `builtin/validators/code-hygiene/rules/unused-code-go.md` → `dead-code-go.md`, `fixtures/unused-code-go.pass.go.tmpl` → `dead-code-go.pass.go.tmpl`, `fixtures/unused-code-go.fail.go.tmpl` → `dead-code-go.fail.go.tmpl`. 6 edited for the 11 references — `builtin/validators/code-hygiene/VALIDATOR.md`, `builtin/validators/code-hygiene/rules/dead-code.md`, `builtin/validators/code-hygiene/fixtures/go.mod.tmpl`, `crates/swissarmyhammer-validators/src/builtin/mod.rs`, `crates/swissarmyhammer-validators/src/review/tool_rules/tests.rs`, `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs`, plus `crates/mirdan/src/builtin_validators.rs`.
    - TDD: renamed the rule in the `SHIPPED_DEAD_CODE_RULES` roster FIRST. RED — `every_shipped_dead_code_tool_rule_passes_its_fixtures` failed with "`dead-code-go` must be a shipped tool rule for [\"go\"]". After the rename, GREEN in 33.7 s against the real staticcheck run.
    - `cargo nextest run -p swissarmyhammer-validators -p mirdan`: 1244 passed, 0 failed, 0 skipped. `cargo fmt --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - next: ready for `/review`.
  timestamp: 2026-08-14T19:25:33.501309+00:00
position_column: doing
position_ordinal: '8280'
title: rename unused-code-go to dead-code-go
---
Every dead-code tool rule uses the name `dead-code-<lang>`. Go is the one exception: `unused-code-go`.

The name is the only defect. The rule works — it declares `supersedes: dead-code` like the others.

## Work

- Rename `builtin/validators/code-hygiene/rules/unused-code-go.md` to `dead-code-go.md`.
- Set `name: dead-code-go` in the frontmatter.
- Rename the two fixtures: `unused-code-go.pass.go.tmpl` and `unused-code-go.fail.go.tmpl`.
- Find and correct every reference to the old name. Look in `builtin/validators/README.md`, `builtin/validators/code-hygiene/VALIDATOR.md`, and the crate tests.

## Done when

- No file or text says `unused-code-go`.
- The fixture test for `dead-code-go` passes. #tool-validators #objectivity