---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzhfw1b875rxpnrdaj8y583q
  text: |-
    Picked up. Research + measurement done before writing any file.

    **cargo machete measured, version 0.9.2 (installed already, `/Users/wballard/.cargo/bin/cargo-machete`).**

    - This workspace, default mode: **141 unused dependencies over 40 of 64 package manifests**, in 0.79 s for one whole-tree run.
    - Hand-checked 12 of them, every one real: `swissarmyhammer-common` `indicatif` (named only in a doc comment), `swissarmyhammer-validators` `chrono` and `sha2` (the only greppable hits were the word "sync**hrono**us"), `swissarmyhammer` `serde`/`tokio`/`toml`/`anyhow` (that crate holds three `.rs` files against ~40 dependencies), `model-loader` `sha2`, `swissarmyhammer-fields` `ulid`, `swissarmyhammer-git` `anyhow`/`async-trait`/`tokio`. No false positive in the sample.
    - External project 1, **ripgrep** at HEAD: 1 finding (`grep-index` / `fst`), real — no `.rs` file of that crate names it.
    - External project 2, **tokio-rs/tracing** at HEAD: 6 findings. 5 real. 1 artifact: `tracing-subscriber` / `regex-automata`, an optional dependency the `env-filter` feature turns on with `dep:regex-automata` so that `matchers::BuildError` implements `std::error::Error`; no source names it, and removing it breaks the feature. That is the case `[package.metadata.cargo-machete] ignored` exists for, and the rule body says so.
    - `--with-metadata` is NOT used. The ^teemmch verdict measured that mode and it misreported `tauri-build` for `kanban-app` and `mirdan-app`. Default mode does not: neither app appears in the 141.

    **Suppression proved on a probe crate before it was written into the rule body.** A standalone crate using `libc` and declaring an unused `serde` reports `serde`; adding `[package.metadata.cargo-machete] ignored = ["serde"]` reports nothing. A trailing comment on the same line — `ignored = ["serde"] # feature-only dependency` — still suppresses, so the ^0v7nysz `// periphery:ignore` trap (a reason on the same line silently broke it) does not repeat here. TOML comments are part of the grammar.

    **Two constraints found by measurement that shape the script:**
    1. `cargo machete` reads only a file literally named `Cargo.toml`. Given `./renamed.toml` it reports nothing. A doctor fixture cannot be named `Cargo.toml` (`find_fixture` requires `<rule>.<kind>.<ext>`), so the script normalizes any package manifest not named `Cargo.toml` into a temporary package and maps the findings back — the pattern `builtin/validators/README.md` already documents for `dart analyze`.
    2. A workspace member manifest cannot be copied out of its workspace: `version.workspace = true` then fails with "can't load root workspace" and machete reports nothing. So a real manifest is scanned in place, never copied.
    3. Machete honors `[lib] path`, so a fixture manifest reaches a source file that sits beside it — which the flat fixtures directory needs, because `materialize_fixtures` copies only the top level.

    Script measured over this repo: 141 findings in 2.5 s, empty stderr, exact line numbers for both `dep = { workspace = true }` and `dep.workspace = true` key styles.
  timestamp: 2026-08-08T20:08:46.184278+00:00
- actor: claude-code
  id: 01kzhgz1zanaf4aa309hc945f7
  text: |-
    Implementation landed. Every card requirement is in the diff.

    **New set** `builtin/validators/manifests/` — `VALIDATOR.md`, `rules/unused-dependencies-rust.md`, and three fixtures. `match.files` is `**/Cargo.toml`, one pattern, and the VALIDATOR.md says where `**/package.json` and its siblings go when a rule arrives that reads them.

    **The `**/Cargo.toml` pattern was measured, not assumed.** My first draft carried `Cargo.toml` and `**/Cargo.toml` both, with prose claiming a leading `**/` never matches zero directories. Broke the gate on purpose and the prose was wrong: with only `**/Cargo.toml` the set still matches the root manifest, and with only the bare literal it stops matching `crates/<name>/Cargo.toml`. The redundant literal is gone and the prose now records what the two runs showed.

    **The production-path test caught a real bug the fixture pair could not.** `cargo machete Cargo.toml`, with no directory component, derives an empty package directory and fails with "can't load root workspace at :" — then exits 0 and prints "didn't find any unused dependencies". A single-crate repository would have reported nothing, silently. The script now always hands machete `$dir/Cargo.toml`. The fixture pair never sees this, because a fixture manifest goes through the temporary-package branch. That is the whole reason the acceptance test over a real one-package repository exists.

    **RED verified 8 ways, each restored to GREEN:**
    1. set `match.files` reduced to `**/Cargo.toml` alone — passes, which is what corrected the prose
    2. set `match.files` reduced to `Cargo.toml` alone — "manifests should match the changed manifest 'crates/swissarmyhammer-fields/Cargo.toml'"
    3. set `match.files` pointed at `**/*.rs` — "manifests should match the changed manifest 'Cargo.toml'"
    4. `supersedes: dead-code` added to the rule — "unused-dependencies-rust must supersede nothing, got: [\"dead-code\"]"
    5. `ignored = ["serde"]` removed from the pass fixture — "the pass fixture ... produced 1 finding(s); none are allowed"
    6. `serde` removed from the fail fixture — "the fail fixture ... produced no findings; at least one is required"
    7. the `awk` marker the parser keys on renamed — "the fail fixture ... produced no findings"
    8. `lib.rs.tmpl` removed from the set — "manifests must embed the tool-rule fixture `fixtures/lib.rs.tmpl`"

    Plus the bug above, found as a RED the first time the acceptance test ran: "exactly one finding must be reported in Cargo.toml; got []".

    **Doctor rows verified in a scratch project**, with `HOME` pointed at `/tmp/machete-probe/scratch-home` so nothing under the real home directory was read or written:

        Validator Set · manifests                            applies to this project (builtin)
        Validator Tool Rule · manifests/unused-dependencies-rust   tool present (0.9.2); fixtures pass

    **Rosters covered** — worked out by reading how `code-hygiene` is registered, and confirmed against a sweep of every enumeration in the tree:
    - `crates/swissarmyhammer-validators/src/builtin/mod.rs` — `MANIFESTS_VALIDATOR`, `MANIFESTS_TOOL_RULES`, two new tests, and the set added to the clean-frontmatter check
    - `crates/swissarmyhammer-validators/src/review/tool_rules.rs` — `SHIPPED_UNUSED_DEPENDENCY_RULES`, the fixture-pair test, the production acceptance test; `verify_run_reports_one_finding` took a `set` parameter so the new test reuses it rather than copying it
    - `crates/mirdan/src/builtin_validators.rs` — `manifests` in the embedded-set list, and `test_tool_rule_fixtures_are_embedded` restructured to `(set, fixtures)` so it covers both sets rather than `code-hygiene` alone
    - `crates/swissarmyhammer-validators/src/lib.rs` — `no_rule_matches_a_shipped_fixture_template` walked only `code-hygiene/fixtures`; it now sweeps every `builtin/validators/*/fixtures` directory, so the next set is covered the day it lands
    - `builtin/validators/code-hygiene/VALIDATOR.md` — the `cargo machete` rejection marked superseded in both places it appears, on the `knip`/`periphery` precedent, and the "two of the five verdicts no longer hold" count corrected to three
    - `doc/src/concepts/validators.md` — a Manifests group

    Nothing else needed adding. Loading and embedding are both directory scans, no snapshot file lists sets, and the pinned-install-command guard (`tool_install.rs`, `every_builtin_tool_rule_pins_its_install_commands`) walks `list_rulesets()` and picked the new rule up on its own.

    **Gates:** `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo nextest run --workspace` 13866 passed / 0 failed / 0 skipped.
  timestamp: 2026-08-08T20:27:53.706713+00:00
- actor: claude-code
  id: 01kzhgzh2pg9j0escjq5yfhb4m
  text: |-
    ### implement — changed
    - evidence: 9 files — builtin/validators/manifests/VALIDATOR.md (new), builtin/validators/manifests/rules/unused-dependencies-rust.md (new), builtin/validators/manifests/fixtures/{unused-dependencies-rust.fail.toml.tmpl, unused-dependencies-rust.pass.toml.tmpl, lib.rs.tmpl} (new), builtin/validators/code-hygiene/VALIDATOR.md, crates/swissarmyhammer-validators/src/builtin/mod.rs, crates/swissarmyhammer-validators/src/review/tool_rules.rs, crates/swissarmyhammer-validators/src/lib.rs, crates/mirdan/src/builtin_validators.rs, doc/src/concepts/validators.md. cargo nextest run --workspace: 13866 passed / 0 failed / 0 skipped. cargo fmt --all --check clean. cargo clippy --workspace --all-targets -- -D warnings clean. RED verified 8 ways before GREEN.
    - next: /review
  timestamp: 2026-08-08T20:28:09.174331+00:00
- actor: claude-code
  id: 01kzhh8b9k806zwpzvn1pq2bqy
  text: |-
    ### test — green
    - evidence: cargo nextest run --workspace — 13866 passed, 0 failed, 0 skipped (11 slow, 1 leaky); cargo fmt --all --check clean; cargo clippy --workspace --all-targets -- -D warnings clean
    - next: none
  timestamp: 2026-08-08T20:32:58.163457+00:00
position_column: doing
position_ordinal: '8280'
title: 'manifests validator set: host cargo machete (unused dependencies)'
---
## STANDING ORDER

This card is a decision, not a proposal. Your job is to implement it exactly.
- Do NOT re-reject cargo machete. The ^teemmch rejection was a scoping technicality, and it is reversed on that card.
- Do NOT file a follow-up card in place of doing the work.
- Stop and escalate ONLY when a step is impossible. Report the exact command and its output.

## The work

The dead-code evaluation (^teemmch) rejected `cargo machete` from `code-hygiene` for a scoping reason, not a quality reason: every machete finding names a `Cargo.toml`, and code-hygiene matches source code. Build the set that hosts it:

- New builtin set `manifests/` whose `match.files` covers manifests (`**/Cargo.toml`; leave room for `**/package.json` later).
- Tool rule `unused-dependencies-rust`: `cargo machete` at workspace scope, findings mapped to the `Cargo.toml` that declares the unused dependency. No `supersedes` — no prompt rule covers this today.
- Inline suppression: machete honors `[package.metadata.cargo-machete] ignored = [...]` in the crate manifest. State it in the rule body.
- Pin the machete version in `install.commands`.
- Fixture pair: a fail fixture manifest with a dependency no fixture source uses; a pass fixture whose one dependency is used. Follow the cargo fixture-package shape the code-hygiene fixtures already use.
- The set only fires when a manifest changed — that is the correct trigger for a dependency question.

#tool-validators #dead-code #objectivity