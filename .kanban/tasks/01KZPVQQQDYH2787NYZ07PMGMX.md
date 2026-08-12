---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kztrqpxhet8a5xc020kw2qqp
  text: |-
    Picked up. Research done — the card's likely cause is CONFIRMED, but the mechanism is one step further than the card states.

    `load_builtins` loads the builtin layer from `env!("CARGO_MANIFEST_DIR")/../../builtin/validators`, which IS this repository's `builtin/validators/` at runtime. So the builtin fixture root is correct on its own.

    The break is SHADOWING. `ValidatorLoader::fixture_dirs()` maps over `self.rulesets`, a `BTreeMap` keyed by set NAME with one entry per name. `load_all` loads `~/.validators` (the `sah init` user snapshot) after the builtins, and `load_rulesets_directory` does `rulesets.insert(name, ruleset)`. A user set named `code-hygiene` therefore REPLACES the builtin `code-hygiene`, whose `base_path` is `<repo>/builtin/validators/code-hygiene`. The builtin set's `fixtures/` directory disappears from `fixture_dirs()` entirely, so the containment test never sees the repository path.

    Measured on this machine: `~/.validators/` holds `code-hygiene`, `code-security`, `completeness`, `dart`, `duplication`, `js-ts`, `manifests`, `numpy`, `python`, `reuse`, `rust`, `swift`, `test-integrity` — every builtin set name. So EVERY builtin fixture root is lost at runtime here.

    Why the two tests missed it: both run under a temp HOME (`IsolatedTestEnvironment` / `EnvVarGuard`), where `~/.validators` is empty, so nothing shadows the builtins and the exclusion appears to work.

    Same defect hits `review --validators <subset>`: `retain_rulesets` prunes the map, so every unnamed set's fixture root is dropped too.

    Current fixture directories on disk (59bd9ae5c deleted the duplication ones): only `builtin/validators/code-hygiene/fixtures/` and `builtin/validators/manifests/fixtures/` exist.
  timestamp: 2026-08-12T10:36:51.505936+00:00
- actor: claude-code
  id: 01kztscdgkxtyczk40jere7j66
  text: |-
    Implementation landed. RED→GREEN was verified by hand.

    RED (before the fix), and it reproduces the card's measurement word for word:

    ```
    assertion `left == right` failed: the shipped fixture is the one file the run did not review:
    ## Review Findings (2026-06-05 12:00)

    Nothing in scope to review.

      left: []
     right: ["builtin/validators/code-hygiene/fixtures/missing-docs-rust.fail.rs.tmpl"]
    ```

    GREEN after the fix. The whole `swissarmyhammer-validators` suite is 690 passed, 0 failed.

    ## The fix

    `ValidatorLoader` now records the `fixtures/` directory of EVERY set it loads, in a `BTreeSet<PathBuf>` beside `rulesets`, through one private `insert_ruleset` — the single place a set enters the loader. `fixture_dirs()` answers from that set instead of mapping over `rulesets`, so a shadowed set keeps its fixture root and `retain_rulesets` (the `validators` subset modifier) no longer narrows the roster of roots.

    `builtin::builtin_validators_dir()` now names the runtime builtin root, with the doc stating what it is: `<repository>/builtin/validators`, resolved from `CARGO_MANIFEST_DIR` at COMPILE time — the source checkout the engine was built from, never a copy beside the binary and never a `sah init` snapshot. `review/scope/fixtures.rs` module docs and `fixture_dirs` docs both point at it, so a reader can tell which directory the containment test compares against.

    ## Why the test uses `Scope::File` and not a working-tree edit

    The scope is `review file <shipped fixture>` over THIS repository, which is the exact call the card measured. Making the fixture differ from HEAD would mean writing a tracked file of the repository under test. nextest runs each test in its own PROCESS, so `serial_test` cannot serialize that write against `every_shipped_missing_docs_tool_rule_passes_its_fixtures`, which runs tool rules over the same fixture content. A mutated fixture would make those runs flaky. The exclusion runs on the resolved file set before any diff, so the single-file scope proves it.

    ## Blast radius

    `fixture_dirs` has one production caller, `review/scope/fixtures.rs::fixture_roots`. `load_rulesets_directory` has two more callers outside this crate (`swissarmyhammer-cli` install components, `doctor.rs`); the signature is unchanged and both only gain the recorded fixture root. `cargo clippy -p swissarmyhammer-validators --all-targets -- -D warnings` and `-p swissarmyhammer-cli` are clean, and `cargo fmt --all --check` is clean.

    ## Blocked verification, not caused by this change

    `cargo clippy --workspace --all-targets` and the `swissarmyhammer-tools` test binary do not build right now, on UNCOMMITTED work in another shell: `ShellExecuteTool::new()` now returns `Result`, and `crates/swissarmyhammer-tools/tests/integration/file_size_limits.rs:410` still calls it as an infallible constructor. Dirty files are `apps/shelltool-cli/src/commands/{doctor,ops,serve}.rs` and `crates/swissarmyhammer-tools/src/mcp/tools/shell/{mod,state}.rs`. None is touched here, so `review_e2e.rs` could not be re-run.
  timestamp: 2026-08-12T10:48:10.003218+00:00
- actor: claude-code
  id: 01kztscm7v1stc8jh52xm6h3w5
  text: |-
    ### implement — changed
    - evidence: 4 files — crates/swissarmyhammer-validators/src/validators/loader.rs, crates/swissarmyhammer-validators/src/builtin/mod.rs, crates/swissarmyhammer-validators/src/review/scope/fixtures.rs, crates/swissarmyhammer-validators/src/review/drive.rs. `cargo nextest run -p swissarmyhammer-validators`: 690 passed, 0 failed. `cargo fmt --all --check` clean; `cargo clippy -p swissarmyhammer-validators --all-targets -- -D warnings` clean.
    - next: /review
  timestamp: 2026-08-12T10:48:16.891652+00:00
position_column: doing
position_ordinal: '8280'
title: validator-fixture exclusion does not fire at runtime for the builtin layer
---
^4cc5y9b added an exclusion that takes a changed file under a loaded validator set's `fixtures/` directory out of the review work-list, and reports it in `skipped_files` with the reason "validator fixture". The card closed with a clean review.

**It does not fire at runtime for the builtin layer of this repository.**

Measured on 2026-08-10, during the review of ^2syfvyt (commit 758416086), which changes two shipped fixtures:

- `skipped_files` came back EMPTY and `skipped` was 0. Neither `.tmpl` fixture was named.
- The report carries no "N file(s) not reviewed" note.
- `review file builtin/validators/code-hygiene/fixtures/magic-numbers-python.fail.py.tmpl` answers **"Nothing in scope to review."** The engine prints that only when the excluded list is empty (`synthesize.rs:355-360`). So the file left the scope silently, which is the exact behaviour ^4cc5y9b exists to stop.

## The wiring is intact

The reviewer checked each hop rather than assuming: `review/scope/fixtures.rs` runs before any validator pairing (`scope.rs:570`), `ExcludedFile` reaches `WorkList::with_excluded` (`scope.rs:654`), and that reaches `not_reviewed_paths` (`synthesize.rs:310`). Nothing is disconnected.

## The likely cause

`ValidatorLoader::fixture_dirs()` names each LOADED set's `fixtures/` directory. At runtime the builtin layer is not this repository's `builtin/validators/` — it is the compiled-in copy, or a `sah init` snapshot under the user's home. So the fixture root the exclusion compares against never contains the repository path, and the containment test answers false.

## Why the tests did not catch it

^4cc5y9b proved acceptance in two halves, and the halves leave this gap between them:

- `a_changed_builtin_fixture_leaves_the_scope_and_source_stays` builds a loader with `builtin_loader()` and compares against the real repository root. That is not the loader a live review builds.
- `review_e2e_sha_excludes_a_validator_fixture_and_still_reviews_the_source` drives the production tool, but over a PROJECT-layer set at `<temp repo>/.validators/`. The project layer works. The builtin layer is untested end to end.

The card's own implement comment names the reason the halves exist: "the builtin layer's fixture root resolves to this repository's `builtin/validators/`, so a temp-repo e2e cannot host it." That reasoning is what left the gap.

## Done when

- A test drives a REAL review over a change to a shipped `builtin/validators/*/fixtures/*` file, through the same loader a live review builds, and proves the file is reported in `skipped_files` with the reason "validator fixture"
- `review file` on a shipped fixture answers with the exclusion and its reason, never "Nothing in scope to review"
- The runtime builtin fixture root is stated in the code, so a reader can tell which directory the containment test compares against

See [[real-path-tests-not-mocks]]. #tool-validators #objectivity