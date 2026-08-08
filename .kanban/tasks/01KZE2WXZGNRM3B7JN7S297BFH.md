---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kze5w48qycxfgpe43fyvxrfz
  text: |-
    Picked up. Research done.

    Where the gap is:
    - `crates/swissarmyhammer-validators/src/doctor.rs` — `degraded_fix` falls to `install_fix`, which returns `None` when `install_commands` is empty. `missing-docs-rust` declares no install commands, so a missing clippy gives a Warning row with no fix.
    - `crates/swissarmyhammer-validators/src/validators/types.rs` — `ToolDoctor` carries `check_command` + `check_version_command`; `ToolInstall` carries `commands`. Both use `deny_unknown_fields`, so a new `doctor.fix_hint` key fails to parse until the struct grows the field.
    - `crates/swissarmyhammer-validators/src/review/tool_install.rs` — `install_tool_commands` reads ONLY `spec.install.commands`. It never reads the doctor block beyond `check_command`.

    Plan:
    1. New public newtype `FixHint` in `types.rs`. It is a distinct type, not a `String`: it gets `Display` and `From<String>`, and deliberately NO `AsRef<str>`, no `Deref`, no `into_string`. A hint can never be handed to the shell runner or pushed into `install.commands` without a visible, deliberate conversion. That is the structural half of "never run a fix hint".
    2. `ToolDoctor.fix_hint: Option<FixHint>` — the hint lives on the doctor block, which is a report surface. `ToolInstall` stays the only source of runnable commands.
    3. `ToolRuleStatus.fix_hint` carries it to the doctor row; `degraded_fix` falls back to it when there are no install commands.
    4. `missing-docs-rust.md` states `rustup component add clippy`.

    Tests planned:
    - doctor.rs: load the real builtin sets, take the real `missing-docs-rust` spec, force the tool missing (stand-in for clippy off PATH), render through `to_checks`, assert the row's fix names `rustup component add clippy`.
    - doctor.rs: install commands win over a hint when a rule declares both.
    - tool_install.rs: a spec whose fix hint would create a marker file if it ran — the marker never appears.
    - tool_install.rs: the builtin pin guard stays non-vacuous and a fix hint never enters the guarded command set.
  timestamp: 2026-08-07T13:16:20.119305+00:00
- actor: claude-code
  id: 01kze6djzbd1ebw5e07bya480y
  text: |-
    Implementation landed.

    What changed:
    - `crates/swissarmyhammer-validators/src/validators/types.rs` — new public newtype `FixHint(String)` plus `ToolDoctor.fix_hint: Option<FixHint>`. `FixHint` has `Display` and `From<String>` and NO `AsRef<str>`, no `Deref<Target = str>`, no owned-string conversion, so it cannot go where a command is expected. It sits on the doctor block; `ToolInstall.commands` stays the only source of runnable commands. That is the structural half of "the lifecycle never runs a hint".
    - `crates/swissarmyhammer-validators/src/doctor.rs` — `ToolRuleStatus.fix_hint` carries it; `degraded_fix` is now `install_fix(rule).or_else(|| fix_hint_fix(rule))`. New `FIX_LEAD_IN` constant so both fixes read the same sentence.
    - `builtin/validators/code-hygiene/rules/missing-docs-rust.md` — states `fix_hint: "rustup component add clippy"`, and the body says the hint is what doctor shows.
    - `builtin/validators/README.md` — the `doctor.fix_hint` key and the Doctor section.
    - `crates/swissarmyhammer-validators/src/review/tool_install.rs` — module doc records that every command comes from `install.commands`.

    RED verified for both acceptance criteria, not assumed:
    - Removing `.or_else(|| fix_hint_fix(rule))` makes `test_toolchain_component_row_names_its_fix_hint` fail with "a degraded row must carry a fix".
    - Removing the `fix_hint` line from the shipped rule makes BOTH `test_toolchain_component_row_names_its_fix_hint` and `a_builtin_fix_hint_would_not_pass_the_pin_guard` fail.

    Tests added:
    - `doctor::tests::test_toolchain_component_row_names_its_fix_hint` — loads the REAL builtin `code-hygiene/missing-docs-rust`, forces only its doctor check command to fail (a test cannot uninstall the host's clippy), and renders through the real `check_tool_rule` + `to_checks`. The row is a Warning whose fix names `rustup component add clippy`.
    - `doctor::tests::test_install_commands_win_over_a_fix_hint` — a rule with both shows the install command, never the hint.
    - `doctor::tests::test_missing_fixtures_outrank_a_fix_hint` — the fixture-pair fix is not displaced.
    - `review::tool_install::tests::the_install_lifecycle_never_runs_a_fix_hint` — the hint is a shell command that would create a marker file; `ensure_tool_installed` (both halves, with an agent) leaves the marker absent.
    - `review::tool_install::tests::every_builtin_tool_rule_pins_its_install_commands` — kept, now non-vacuous: it asserts the guard saw at least one real builtin install command.
    - `review::tool_install::tests::a_builtin_fix_hint_would_not_pass_the_pin_guard` — every builtin hint fails `install_command_pins_version`, which is exactly why a toolchain component states a hint and not a command.

    Verification: `cargo test -p swissarmyhammer-validators` 503 passed, 0 failed. `cargo clippy --workspace --all-targets` clean, zero warnings. `cargo fmt --all -- --check` clean. `cargo test -p swissarmyhammer-cli --lib -- commands::install commands::doctor` 29 passed (the CLI doctor consumes `to_checks` directly, so `sah doctor` gets the fix with no CLI change).

    ARCHITECTURE.md needs no update: the change adds a fact field to the existing `swissarmyhammer-validators::doctor` fact producer, with no new module and no new dependency edge.

    ### implement — changed
    - evidence: 8 files — crates/swissarmyhammer-validators/src/validators/types.rs, crates/swissarmyhammer-validators/src/validators/mod.rs, crates/swissarmyhammer-validators/src/doctor.rs, crates/swissarmyhammer-validators/src/review/tool_install.rs, crates/swissarmyhammer-validators/src/review/tool_rules.rs, crates/swissarmyhammer-validators/src/review/drive.rs, builtin/validators/code-hygiene/rules/missing-docs-rust.md, builtin/validators/README.md
    - next: /review
  timestamp: 2026-08-07T13:25:52.235465+00:00
- actor: claude-code
  id: 01kze7fkjsad2rztw1wq5g2x1z
  text: |-
    ### review — clean
    - evidence: review sha HEAD~1..HEAD (49c4be016) — 0 findings, 0 confirmed, 0 refuted; 18 validator/file pairs attempted, 0 failed, 0 skipped
    - next: task moved to done; no Review Findings section appended because the pass found nothing
  timestamp: 2026-08-07T13:44:26.969546+00:00
- actor: claude-code
  id: 01kze7g5q7yjp85ghmg1g4kdkp
  text: |-
    ### finish iteration 1 — clean
    - implement: changed — 8 files; FixHint is a newtype that cannot reach a command position, degraded_fix falls back to it, missing-docs-rust states "rustup component add clippy". Both acceptance criteria have RED-verified tests.
    - test: green — cargo nextest run --workspace 13761 passed, doc tests 59 passed 0 failed, fmt clean, clippy clean
    - commit: 49c4be016
    - review: clean — 0 findings, 18 pairs attempted; task moved to done
  timestamp: 2026-08-07T13:44:45.543043+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffc180
title: Doctor has no fix hint for a toolchain-component tool rule
---
A tool rule whose tool ships with the language toolchain declares no `install.commands`, because `install_command_pins_version` requires a version pin and a `rustup` component has no package version to pin. `missing-docs-rust` is the first such rule.

The cost: when clippy is missing, `sah doctor` shows the row as a warning with the degradation detail and the prompt-fallback note, but `degraded_fix` returns `None`, so the user gets no command to run.

Work:
- Give a tool rule a way to state the fix a person runs when there is no pinnable package — for example a `doctor.fix_hint` string that `degraded_fix` falls back to.
- `missing-docs-rust` states `rustup component add clippy`.
- The install lifecycle must not run a fix hint. It is text for a person, never a command the engine tries.

Acceptance:
- With clippy off PATH, the doctor row for `missing-docs-rust` names `rustup component add clippy`.
- `install_command_pins_version` still guards every real install command.

#tool-validators