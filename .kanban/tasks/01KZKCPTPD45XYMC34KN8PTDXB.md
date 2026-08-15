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
- actor: claude-code
  id: 01m00wffpht5m928vdrq5pqs4y
  text: |-
    ### review — clean
    - evidence: `review sha 5687cb92a~1..5687cb92a`. counts: 0 findings, 0 confirmed, 0 refuted, 9 attempted, 0 failed. 4 files reviewed, 5 not reviewed (all validator fixtures, excluded by category).
    - scope: the report scope line reads "reviewed the diffs only — lines this change added or modified". No finding landed on a pre-existing line, because there are no findings.
    - renames: git reports the three moves as R094 / R094 / R099. The engine detected them as renames, not as a delete plus an add. `resolve_sha` builds a rename map with `find_renames` (git2 Delta::Renamed and Delta::Copied), then reads the BEFORE side from the source path. The renamed rule and its two fixtures therefore diffed as their real 1-line delta. The engine did NOT offer their whole content as new lines.
    - honesty: 0 findings dropped for a false premise. No finding asserted anything, so no line needed confirmation.
    - next: none. Task moves to done.
  timestamp: 2026-08-14T19:37:42.865661+00:00
- actor: claude-code
  id: 01m00wgwdshhspbq6346q90h9n
  text: |-
    ### finish iteration 1 — clean. Card done.
    - implement: changed — 10 files. The card named 3 places holding references; the real set was **8 files, 11 references**. Beyond the card's list: the `dead-code` fallback rule lists its six siblings by name, a fixture `go.mod.tmpl` header names the rule it serves, three roster constants across the validators crate, and mirdan's fixture install roster by filename.
    - Two ordering details that a plain find-and-replace would have got wrong: `SHIPPED_RULES_THAT_READ_A_GO_FILE` is SORTED, so the entry moved from after `no-commented-code` to after `dead-code`; and mirdan's `CODE_HYGIENE_FIXTURES` pair moved into the dead-code group after `dead-code-rust` to match the order the other rosters keep.
    - `missing-docs-go.md` states a COUNT of 25 rules that read a `.go` file, not their names — a rename does not move a count, and the count was re-verified as still correct. Nothing under `crates/mirdan/retired-validators/` carries the old name, so no byte-frozen snapshot changed.
    - TDD: the roster was renamed first and watched fail with `` `dead-code-go` must be a shipped tool rule for ["go"] ``, then the rule was renamed and the fixture test passed against the REAL staticcheck run (51.3s, not skipped).
    - test: green — cargo nextest run --workspace 14199 passed, 0 failed, 0 skipped. fmt and clippy clean. The old name survives only in `.kanban/` records and gitignored logs.
    - commit: 5687cb92a
    - review: clean — 0 findings, 9 attempted, 0 failed. Task moved to done.

    **This commit incidentally proved ^apb04az's rename detection on a real rename.** Git scored the three moves R094, R094 and R099. The engine's `resolve_sha` calls `range_rename_sources`, collects every `Delta::Renamed | Delta::Copied` into a new-path → old-path map, and reads each file's BEFORE side through `base_path`, so the renamed rule diffed as its ONE changed frontmatter line rather than as a whole new file. That whole-content re-offer is exactly the defect ^apb04az set out to fix, and it did not happen.

    One note for the commit step, caught by the tester: `git mv` staged the three files with their PRE-EDIT content while the working tree held the corrected frontmatter, so they needed re-adding before commit or the staged diff would not have matched what was tested.
  timestamp: 2026-08-14T19:38:28.665397+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffff8280
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