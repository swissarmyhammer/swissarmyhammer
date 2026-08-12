---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzq5vfjhjht9p272qyf9d891
  text: |-
    Survey done, tool by tool, with versions.

    **Rust — no usable tool. The old claim is CORRECTED, not confirmed.**
    - clippy 0.1.97 (8bab26f4f6 2026-07-14) / rustc 1.97.1. `clippy-driver -Whelp` = 1114 lines, every group, `restriction` included. 69 lines match `literal|magic|numeric|number|constant`; none asks for a NAME. They read representation, suffix or type: `decimal-literal-representation`, `default-numeric-fallback`, `unreadable-literal`, `large-digit-groups`, `inconsistent-digit-grouping`, `unusual-byte-groupings`, `mixed-case-hex-literals`, `zero-prefixed-literal`, `separated-literal-suffix`, `unseparated-literal-suffix`, `mistyped-literal-suffixes`, `excessive-precision`, `lossy-float-literal`, `non-ascii-literal`.
    - dylint (cargo-dylint 6.0.3) ships `examples/supplementary/unnamed_constant`: "Checks for unnamed constants, aka magic numbers." So a Rust lint EXISTS, and "no healthy Rust lint reports an unnamed literal" was wrong as written.
    - It was INSTALLED AND RUN in an isolated CARGO_HOME/RUSTUP_HOME (no machine changed). Measured at default `threshold: 10`: reports `age > 18`, `part * 100`, `with_capacity(3600)`, `return 42;`, `n * 3600`; silent on `word << 8`, `n * 4096`, `with_capacity(65535)`, a bare `42` tail expression, `n * 7`, and both declarations. It passes any value whose bits form one run, so every power of two and all-ones mask is invisible; it reports `100`, which the prompt rule carves out; `threshold` is its only key.
    - Cost measured: nightly-2026-05-28 + `rustc-dev` = 2.4 GB, `cargo install cargo-dylint@6.0.3 dylint-link@6.0.3` = 22 s, and the lint builds from a git checkout because it is on crates.io under no name.
    - Verdict: Rust keeps the prompt rule, and the survey is now the evidence.

    **Dart — a usable tool. The old claim is REFUTED.**
    - SDK linter 3.11.0 (Flutter 3.41.2): 263 rules; none reports an unnamed literal; "magic" appears nowhere. `lints` 6.1.0 (34 + 55 rules) and `flutter_lints` 6.0.0 (10 rules) are selections of the same 263.
    - `dart_code_metrics` 5.7.6: `sdk: '>=2.18.0 <3.0.0'`, so it cannot be activated on Dart 3.11.0.
    - `solid_lints` 0.3.3 (`custom_lint` 0.8.1 plugin) has `no_magic_number`. The plugin is a dependency of the PROBE PACKAGE the rule writes, never of the project under review — the same shape `missing-docs-dart` already uses. That is what makes the old objection wrong.
    - `dart_code_linter` 4.1.9 measured beside it and NOT taken: over `dart-lang/http` at a9176ac (324 files) solid_lints = 683 findings / 13 s, dart_code_linter = 653 / 5 s, 645 shared. The 8 only dart_code_linter reports are all default parameter values (a prompt carve-out); of the 38 only solid_lints reports, 15 are `100` and 23 are literals inside a declaration initializer that dart_code_linter drops.
    - `dcm` stays rejected (commercial), as recorded before.

    Written: `magic-numbers-dart.md` + fail/pass fixtures, registered in `builtin/mod.rs`, roster row + acceptance test `the_shipped_dart_magic_numbers_tool_rule_reports_every_fail_fixture_line`. `magic-numbers.md` and `VALIDATOR.md` now carry the survey.
  timestamp: 2026-08-11T01:09:08.817590+00:00
- actor: claude-code
  id: 01kzq6e95sq84v0dkdgx2v435w
  text: |-
    ### implement — changed
    - evidence: 8 files. New: `builtin/validators/code-hygiene/rules/magic-numbers-dart.md`, `fixtures/magic-numbers-dart.fail.dart.tmpl`, `fixtures/magic-numbers-dart.pass.dart.tmpl`. Changed: `builtin/validators/code-hygiene/VALIDATOR.md` (the whole survey), `rules/magic-numbers.md` (Rust claim corrected, Dart moved to the tool list), `crates/swissarmyhammer-validators/src/builtin/mod.rs`, `.../review/tool_rules/tests.rs`, `.../review/tool_rules/tests/shipped.rs`.
    - commands: `cargo nextest run -p swissarmyhammer-validators magic_numbers` = 6 passed; `cargo fmt --all`; `cargo clippy --workspace --all-targets -- -D warnings` = clean; `cargo nextest run --workspace` = 14043 run, 14039 passed, 4 failed — the 4 are the ^bh5ncd0 set (three `review_e2e` + the stdio transport timeout), unchanged by this work. `^c9pb2f3` passed in this run.
    - next: /review
  timestamp: 2026-08-11T01:19:24.857965+00:00
- actor: claude-code
  id: 01kzq6x6n989q3vn1kt8tcnbny
  text: |
    ### review — findings

    - scope: `review sha HEAD~1..HEAD` (d6a1d101c), 8 validators, 0 failed, 0 skipped
    - evidence: 2 findings — builtin/validators/code-hygiene/VALIDATOR.md:153, builtin/validators/code-hygiene/rules/magic-numbers-python.md:84
    - engine: 1 finding, dropped. It asked to deduplicate `dead_code_work` against `complexity_work` in crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs. Both functions are at the same lines in HEAD~1, thus the commit did not touch them. The review skill drops a finding that asks to refactor test code that already existed.
    - both recorded findings come from the card's own objectivity check: a claim in a rule body must be true. The Dart rule landed, and it made one sibling paragraph false and one survey sentence disagree with the rule it describes.
    - checked and correct: the roster at VALIDATOR.md:28-33 names all five tool rules; magic-numbers.md:45 adds Dart to the superseded languages; the Rust paragraph at magic-numbers.md:47 matches the survey; magic-numbers-swift.md:43 "Swift alone expresses it" stays true, because Dart cannot express the shift carve-out; the Dart rule's claims about the Go, TypeScript, Swift and Python allow-lists match those four files; VALIDATOR.md:199 "Dart takes no tool rule" reads in the complexity section and stays true.
    - next: correct the two documents, then review again.
  timestamp: 2026-08-11T01:27:33.801401+00:00
- actor: claude-code
  id: 01kzq6y873vqrq1hkyg4fdvccy
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 8 files; the survey refuted the Dart claim and corrected the Rust claim
    - test: green — cargo nextest run -p swissarmyhammer-validators magic_numbers, 6 passed; workspace 14039 passed, 4 failed (the known ^bh5ncd0 set)
    - commit: d6a1d101c — 8 files, 538 insertions
    - review: findings — builtin/validators/code-hygiene/VALIDATOR.md:153, builtin/validators/code-hygiene/rules/magic-numbers-python.md:84
  timestamp: 2026-08-11T01:28:08.163495+00:00
- actor: claude-code
  id: 01kzq78r0c96ffx6hq68cnvhxq
  text: |-
    Both findings share one cause: the fifth tool rule landed, and three sentences kept a count or a tool name the evidence no longer supports. The cause was removed from each whole file, not from the two cited lines.

    **Finding 1 — VALIDATOR.md.** The trap sentence named `dart_code_linter`, which the rule never runs. It now names `dart run custom_lint` in both halves, and it states that the command is the one `magic-numbers-dart` runs.

    A second sentence in the same file carried the same shape of staleness. The complexity section stated "Dart takes no tool rule". That was true when Dart had no rule of any kind. It now states "Dart takes no COMPLEXITY tool rule", and it points at the magic-number rule above. The other Dart sentences were re-read and each one holds: the roster names five rules, the survey bullets name `solid_lints` as the tool taken and `dart_code_linter` as the tool measured and refused, and the DCM heading already said "complexity".

    **Finding 2 — magic-numbers-python.md.** The heading, the lead sentence and the closing sentence each counted four rules. The section now reads "This rule and `magic-numbers-dart` are the two of five that cannot allow `100`", "Three of the five tools take a usable value allow-list", and "`ruff` and `solid_lints` are the two tools of the five that give no usable value allow-list". The two failures differ, so the body states each one: `ruff` has no allow-list key, and `solid_lints` 0.3.3 has a key its own parser cannot read.

    The same file carried a fourth stale count in a later section — "the narrowest of the four `magic-numbers-*` rules". It now reads five, and it names what `no_magic_number` reads.

    **A third file carried the same cause.** `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:1124` said Swift "is the one rule of the four that restores BOTH halves". Swift is still the only rule that restores both, because Dart restores neither, so the claim needed the count and not the verdict. It now reads five.

    Read for staleness and left alone, because each stays true: `magic-numbers-dart.md` "`magic-numbers-python` states the same shape of gap for the same shape of reason"; `magic-numbers-swift.md` "Swift alone expresses it"; every other "four" in the rules directory counts values or config keys, never rules.
  timestamp: 2026-08-11T01:33:52.012914+00:00
- actor: claude-code
  id: 01kzq78xvgk3ym2jr2xp8mj1qz
  text: |-
    ### implement — changed
    - evidence: 3 files — builtin/validators/code-hygiene/VALIDATOR.md, builtin/validators/code-hygiene/rules/magic-numbers-python.md, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs. Both review findings are corrected and checked.
    - commands: `touch crates/swissarmyhammer-validators/src/builtin/mod.rs`; `cargo fmt --all`; `cargo nextest run -p swissarmyhammer-validators` = 581 run, 581 passed, 0 skipped; `cargo clippy --workspace --all-targets -- -D warnings` = clean, exit 0. `git status` outside `.kanban` shows exactly those 3 files.
    - next: /review
  timestamp: 2026-08-11T01:33:58.000851+00:00
- actor: claude-code
  id: 01kzq7prrr5knwapsdpywer595
  text: |
    ### review — clean

    - scope: `review sha HEAD~1..HEAD` (commit 9050b6dbd)
    - engine counts: 6 findings, 6 confirmed, 1 refuted. All 6 name test code that already existed in `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs` (the `dead_code_work` / `complexity_work` pair, and three repeated closures). The review rule that drops changes to test code that already existed removes all 6. Zero findings stay.
    - prior finding 1 is closed: `builtin/validators/code-hygiene/VALIDATOR.md` now gives the `--root-folder` silent-zero trap to `dart run custom_lint`, the command the rule runs. The `dart_code_linter` name stays only in the rejection record and in the "Why `solid_lints` and not `dart_code_linter`" section, where it is correct.
    - prior finding 2 is closed: `builtin/validators/code-hygiene/rules/magic-numbers-python.md` reads five in all four places — the heading "the two of five that cannot allow `100`", "Three of the five tools take a usable value allow-list", "`ruff` and `solid_lints` are the two tools of the five", and "narrowest of the five `magic-numbers-*` rules".
    - truth sweep is complete. No sentence in the `code-hygiene` set still counts four magic-number rules, still gives the trap to the wrong tool, or still says Dart has no rule:
      - `VALIDATOR.md` — "Five tool rules supersede it" and the list names all five. "Dart takes no COMPLEXITY tool rule". "Rust keeps the `magic-numbers` prompt rule".
      - `rules/magic-numbers.md` — the prompt rule names Python, TypeScript and JavaScript, Go, Swift, and Dart, and keeps Rust.
      - `tests/shipped.rs` — "the one rule of the five".
      - `SHIPPED_MAGIC_NUMBERS_RULES` in `crates/swissarmyhammer-validators/src/review/tool_rules/tests.rs` holds all five rules, Dart included, so the counts have evidence.
      - Every other "four" and "five" in the set counts literals, lint codes, findings or complexity gates, not magic-number rules.
    - next: none. The card moves to done.
  timestamp: 2026-08-11T01:41:31.544859+00:00
- actor: claude-code
  id: 01kzq7qjn7vtm6pvc3z6p7k8y4
  text: |-
    ### finish iteration 2 — clean
    - implement: changed — 3 files; the stale count and the wrong trap owner are corrected, and a third file held the same stale count
    - test: green — cargo nextest run -p swissarmyhammer-validators, 581 passed; cargo clippy --workspace --all-targets -D warnings clean
    - commit: 9050b6dbd — 3 files
    - review: clean — 6 engine findings, all in pre-existing test code, all dropped by the written exception; both prior findings verified closed; card moved to done
  timestamp: 2026-08-11T01:41:58.055635+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffdf80
title: magic numbers go objective for rust and dart
---
Go, Python, Swift and TypeScript each have a `magic-numbers-<lang>` tool rule that supersedes the `magic-numbers` prompt rule. Rust and Dart have none, so an LLM still measures and decides for them.

## Survey first

Neither language has an obvious stock lint. Enumerate the full tool space before you report a gap:

- Rust: every clippy lint, the restriction group included, and any external lint crate
- Dart: the `lints` and `flutter_lints` packages, and any analyzer plugin

Record what you found, tool by tool, with the version you tested.

## Then

For each language that has a tool, write `builtin/validators/code-hygiene/rules/magic-numbers-<lang>.md` to the contract in `builtin/validators/README.md`:

- `match.files` and `match.project_types` for the language
- `supersedes: magic-numbers`
- a `tool.run` shell script that writes its own config to a temp path, never the project's own lint config
- `doctor` and `install` blocks, with the tool version pinned
- a pass fixture and a fail fixture
- a measurement on a real repository: finding count, run time, and whether every finding is true

The exemption must be an inline suppression or a config key, never prose.

If a language has no tool, say so with the survey as evidence, and leave it on the prompt rule.

## Done when

Rust and Dart have a magic-number tool rule, or the card records why no tool can give one. #tool-validators #objectivity

## Review Findings (2026-08-10 20:21)

- [x] `builtin/validators/code-hygiene/VALIDATOR.md:153` — the survey gives the `--root-folder` silent-zero trap to `dart_code_linter`: "`--root-folder` does not move where `dart_code_linter` reads its configuration". The rule file gives the same trap to a different tool. `builtin/validators/code-hygiene/rules/magic-numbers-dart.md:156` opens the section with "`dart run custom_lint` reads the configuration of the package it runs in", and the bullet below it records the measurement. `magic-numbers-dart` runs `dart run custom_lint` with `solid_lints` (`magic-numbers-dart.md:27`), and it never runs `dart_code_linter` — that tool is the rejected alternative (`magic-numbers-dart.md:111`, "Why `solid_lints` and not `dart_code_linter`"). A trap "found and answered inside `magic-numbers-dart`" must name the tool the rule runs. Correct the survey sentence to name `custom_lint`.
- [x] `builtin/validators/code-hygiene/rules/magic-numbers-python.md:84` — the new Dart rule makes the sibling heading false. The heading reads "This is the one rule of the four that cannot allow `100`". There are now five `magic-numbers-*` tool rules, and `magic-numbers-dart` also cannot allow `100`: `magic-numbers-dart.md:78` states "The rule therefore states no `allowed` key and keeps the built-in default, which is `[-1, 0, 1]`", and `magic-numbers-dart.md:83` states "So `part * 100` REPORTS". The body carries the same error twice: "The other three tools each take a value allow-list" and "`ruff` is the one tool of the four that offers no value allow-list at all". The two files disagree head-on, because `magic-numbers-dart.md:84` states "`magic-numbers-python` states the same shape of gap for the same shape of reason". Correct the heading and the body for five rules and for two tools that offer no usable value allow-list.
