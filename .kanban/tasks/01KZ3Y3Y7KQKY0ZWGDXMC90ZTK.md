---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kz624xhaw0d25n94xayz0000
  text: |-
    ### implement — changed

    **Task 1 (skill doc)** — added a new bullet to `builtin/skills/test/SKILL.md`'s `## Guidelines` section (confirmed this IS the source, not `.skills/` — no `.skills/test/SKILL.md` exists). Bullet: never hand-edit source, run tests, watch it fail, and revert to "prove" a test is non-vacuous; the perturbation belongs in the suite permanently, via either pinning the literal value (not just the symbol) or parameterizing the input inside the test.

    **Task 2 (REVIEW_DEFAULT_CLAUDE_MODEL)** — re-checked: the constant DOES exist in this worktree now, at `crates/swissarmyhammer-config/src/model.rs` (it landed after this card's description was written, via commit `d776e345f` "feat(model): claude-code-haiku model + baked-in review-scope default", well before HEAD `c1d45ede4`). So path (a) applies directly — no need to fall back to the `^3nz7374` guard-test analog.

    Added `review_default_claude_model_is_the_literal_haiku_switch` in `chat_model_config_tests`, right after `review_chat_model_precedence`. It asserts `REVIEW_DEFAULT_CLAUDE_MODEL == "haiku"` as a literal, so a change to the constant now fails a dedicated test directly instead of requiring the manual edit-run-revert cycle described in the card.

    Verification:
    - `cargo test -p swissarmyhammer-config review_default_claude_model` — 1 passed, 0 failed.
    - `cargo test -p swissarmyhammer-config model::` — 137 passed, 0 failed (full model.rs suite, includes the 4 previously-hand-perturbed tests).
    - `cargo fmt --all` — no formatting changes needed to touched files.
    - `cargo clippy -p swissarmyhammer-config --all-targets -- -D warnings` — clean.
    - `cargo nextest run -E 'rdeps(swissarmyhammer-config)'` — 8163 tests run, 8163 passed, 0 skipped.

    Files touched (only these two):
    - `builtin/skills/test/SKILL.md`
    - `crates/swissarmyhammer-config/src/model.rs`

    No source file was hand-edited and reverted to demonstrate anything in this change — every check above is a real, re-runnable command.

    next: /review
  timestamp: 2026-08-04T09:37:18.378784+00:00
- actor: claude-code
  id: 01kz62edbkttah6g3186hcfwbc
  text: |-
    ### finish iteration 1 — changed
    - implement: changed — builtin/skills/test/SKILL.md (guideline), crates/swissarmyhammer-config/src/model.rs (literal-pinning test)
    - test: green — cargo nextest run -E 'rdeps(swissarmyhammer-config)', 8163 passed, 0 failed, 0 skipped
    - commit: 8fbd6b37e
    - review: pending
  timestamp: 2026-08-04T09:42:29.491434+00:00
- actor: claude-code
  id: 01kz62s88ezbxzh29xjjwkrefa
  text: |-
    ### review — findings
    - evidence: review sha HEAD~1..HEAD (checkpoint 8fbd6b37e); 3 findings — crates/swissarmyhammer-config/src/model.rs:906, crates/swissarmyhammer-config/src/model.rs:965, crates/swissarmyhammer-config/src/model.rs:1318
    - next: task stays in review; findings appended to description as of 2026-08-04 04:42
  timestamp: 2026-08-04T09:48:24.718476+00:00
- actor: claude-code
  id: 01kz62v5xey5gdhp43tbbr3zv1
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — builtin/skills/test/SKILL.md (guideline), crates/swissarmyhammer-config/src/model.rs (literal-pinning test)
    - test: green — cargo nextest run -E 'rdeps(swissarmyhammer-config)', 8163 passed, 0 failed, 0 skipped
    - commit: 8fbd6b37e
    - review: findings — crates/swissarmyhammer-config/src/model.rs:906, crates/swissarmyhammer-config/src/model.rs:965, crates/swissarmyhammer-config/src/model.rs:1318
  timestamp: 2026-08-04T09:49:27.854963+00:00
- actor: claude-code
  id: 01kz639zr35fe8hzzqpw0vqxq7
  text: |-
    ### implement — changed

    Fixed all 3 review findings in `crates/swissarmyhammer-config/src/model.rs` by extracting the shared helpers the review specified:

    1. **`validate_path_basics(path: &Path, context: &str) -> Result<PathBuf, ModelError>`** — new fn, holds the empty-path check and the `MAX_PATH_LENGTH` (4096) check, with `context` in the warning text. `validate_directory_path` now opens with `Self::validate_path_basics(dir_path, "model directory")?` and `validate_config_file_path` opens with `Self::validate_path_basics(path, "config file")?`. Both bodies dropped their private copies of the empty/length checks and the local `MAX_PATH_LENGTH` const.

    2. **`check_directory_access(path: &Path, required_mode: u32, access_name: &str) -> Result<(), ModelError>`** — new fn, holds the metadata fetch, is-a-directory check, and the `#[cfg(unix)]` permission-bit check. `check_directory_permissions` is now `Self::check_directory_access(path, 0o400, "readable")` and `check_directory_writable` is now `Self::check_directory_access(path, 0o200, "writable")`. Both original bodies (each ~25 lines) collapsed to one-line delegations.

    3. **`canonicalize_path(path: &Path, context: &str) -> Result<PathBuf, ModelError>`** — new fn, wraps `path.canonicalize()` with the shared `tracing::error!` + `ModelError::IoError` mapping. `ensure_config_structure` now calls `Self::canonicalize_path(&current_dir, "current directory")?` and `validate_config_file_path`'s existing-file branch calls `Self::canonicalize_path(path, "config path")?`.

    No function signatures changed for any existing caller (`validate_directory_path`, `check_directory_permissions`, `check_directory_writable`, `validate_config_file_path`, `ensure_config_structure` all kept their original names/signatures — they now delegate internally), so every one of the ~35 existing unit tests that calls these by name (e.g. `test_check_directory_writable_readonly_dir`, `test_validate_config_file_path_empty_path`, `test_ensure_config_structure_create_dir_fails_readonly_parent`) needed no changes and still exercises the same observable behavior (error variant, `ErrorKind`, returned path).

    Verified no other crate in the workspace calls any of these five model.rs functions directly (grepped the whole repo) — the duplication and its fix are fully contained to this one file.

    Verification:
    - `cargo fmt --all` — clean, no diff.
    - `cargo clippy -p swissarmyhammer-config --all-targets -- -D warnings` — clean, 0 warnings.
    - `cargo nextest run -E 'rdeps(swissarmyhammer-config)'` — full run, not narrowed: **8163 tests run: 8163 passed (4 slow, 1 leaky), 0 skipped**.

    next: /review
  timestamp: 2026-08-04T09:57:33.059367+00:00
- actor: claude-code
  id: 01kz649tfzg6y8v2nvg37rhba4
  text: |-
    ### finish iteration 2 — changed
    - implement: changed — extracted validate_path_basics, check_directory_access, canonicalize_path in crates/swissarmyhammer-config/src/model.rs to fix 3 duplication findings
    - test: green — cargo fmt --check clean, clippy -D warnings clean, cargo nextest -E 'rdeps(swissarmyhammer-config)' 8163 passed 0 skipped on 3 of 4 runs; 1 run hit claude-agent::a_lagged_collector_is_an_error_not_a_reply_with_holes, unrelated file (only model.rs changed), passed 10/10 in isolation — pre-existing flake, not caused by this change
    - commit: pending
    - review: pending
  timestamp: 2026-08-04T10:14:56.255195+00:00
- actor: claude-code
  id: 01kz64qzeway0f271mk9885ng0
  text: |-
    ### review — findings
    - evidence: 1 finding — crates/swissarmyhammer-config/src/model.rs:1416. Engine returned 4 total; 3 dropped per the review skill's blanket test-refactor exception (magic numbers 4096/4097/0o700 at model.rs:3111, 3256, 3276 are in pre-existing test code untouched by this checkpoint's diff, confirmed against HEAD~1).
    - next: define a named constant for the 0o200 permission bit passed to check_directory_access in check_directory_writable, then re-review.
  timestamp: 2026-08-04T10:22:40.092836+00:00
- actor: claude-code
  id: 01kz64rh1dbyhnjktqbd9b3zjc
  text: |-
    ### finish iteration 2 — findings
    - implement: changed — extracted validate_path_basics, check_directory_access, canonicalize_path in crates/swissarmyhammer-config/src/model.rs
    - test: green (with noted unrelated pre-existing flake in claude-agent, reran clean 3/4 times, isolated 10/10 pass)
    - commit: a867239a7
    - review: findings — crates/swissarmyhammer-config/src/model.rs:1416 (inline 0o200 permission literal needs a named constant)
  timestamp: 2026-08-04T10:22:58.093156+00:00
- actor: claude-code
  id: 01kz653rba4wn0rybeytaeg5ym
  text: |-
    ### implement — changed
    - evidence: fixed `crates/swissarmyhammer-config/src/model.rs` finding — added two module-level named constants `READABLE_MODE_BIT = 0o400` and `WRITABLE_MODE_BIT = 0o200` on `ModelManager`, immediately above `check_directory_access`; updated `check_directory_permissions` to call `Self::check_directory_access(path, Self::READABLE_MODE_BIT, "readable")` and `check_directory_writable` to call `Self::check_directory_access(path, Self::WRITABLE_MODE_BIT, "writable")`; no inline `0o200`/`0o400` literals remain at the call sites.
    - checked off the `:1416` finding item in the task description.
    - `cargo fmt --all` — clean, no diff.
    - `cargo clippy -p swissarmyhammer-config --all-targets -- -D warnings` — clean, 0 warnings.
    - `cargo nextest run -E 'rdeps(swissarmyhammer-config)'` — 8163 tests run: 8163 passed, 0 failed, 0 skipped (169.8s).
    next: /review
  timestamp: 2026-08-04T10:29:06.026885+00:00
- actor: claude-code
  id: 01kz65s65601yqjtw9jv2y5kgx
  text: |-
    ### review — findings
    - evidence: `review sha HEAD~1..HEAD` (checkpoint c78d3ea6a) returned 19 findings; 13 recorded, 6 dropped (pre-existing test-code restyling, covered by the blanket test-refactor exception): crates/swissarmyhammer-config/src/model.rs:187, :188, :189, :190, :365, :367, :368, :369, :371, :373, :374, :376, :497
    - next: address the 13 open doc-comment/Eq-derive findings on Platform, ModelSource, ModelInfo, then re-run /review
  timestamp: 2026-08-04T10:40:48.294500+00:00
- actor: claude-code
  id: 01kz65ssg1kk2mb66mhp0mm001
  text: |-
    ### finish iteration 3 — findings
    - implement: changed — added READABLE_MODE_BIT/WRITABLE_MODE_BIT named constants in crates/swissarmyhammer-config/src/model.rs
    - test: green — cargo nextest run -E 'rdeps(swissarmyhammer-config)', 8163 passed, 0 failed, 0 skipped
    - commit: c78d3ea6a
    - review: findings — 13 items in crates/swissarmyhammer-config/src/model.rs: missing doc comments on Platform enum variants (187-190), ModelSource enum/variants/fields (365,367-369,371,373-376), missing Eq derive alongside PartialEq (365, 497)
  timestamp: 2026-08-04T10:41:08.097512+00:00
- actor: claude-code
  id: 01kz667kmk53zzmfxxqeb2nks1
  text: |-
    ### implement — changed
    - evidence: fixed all 13 open findings from the 2026-08-04 05:33 review pass in `crates/swissarmyhammer-config/src/model.rs`:
      - added `///` doc comments to `Platform::MacosArm64`, `MacosX86_64`, `LinuxX86_64`, `LinuxAarch64`.
      - added `Eq` to `ModelSource`'s derive list (`#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]`) — verified safe: all fields (String, Option<String>, PathBuf, Option<PathBuf>) are structurally Eq.
      - added `///` doc comments to `ModelSource::HuggingFace` and its `repo`/`filename`/`folder` fields, and to `ModelSource::Local` and its `filename`/`folder` fields.
      - added `Eq` to `ModelInfo`'s derive list — verified safe: all fields (String, String, ModelConfigSource which already derives Eq, Option<String>) are structurally Eq.
    - checked off all 13 items in the `## Review Findings (2026-08-04 05:33)` section of the task description.
    - note: an earlier `update task` call in this pass corrupted the description with literal `\n`/`\"` text and dropped the `review` tag (the known bug called out in the task instructions); caught it via `get task` immediately after and re-wrote the description with real newlines — confirmed via `get task` that it is now clean (progress 1.0, tags: ["review"]).
    - `cargo fmt --all` — clean, no diff.
    - `cargo clippy -p swissarmyhammer-config --all-targets -- -D warnings` — clean, 0 warnings.
    - `cargo nextest run -E 'rdeps(swissarmyhammer-config)'` — full run, not narrowed: 8163 tests run: 8163 passed, 0 skipped, 0 failed.

    Files touched: `crates/swissarmyhammer-config/src/model.rs` only.

    next: /review
  timestamp: 2026-08-04T10:48:40.851677+00:00
- actor: claude-code
  id: 01kz66f0qfsjxsw8393ww38kr9
  text: |-
    ### finish iteration 4 — changed
    - implement: changed — doc comments on Platform/ModelSource variants+fields, Eq derives on ModelSource/ModelInfo
    - test: green — cargo nextest run -E 'rdeps(swissarmyhammer-config)', 8163 passed, 0 failed, 0 skipped
    - commit: 2ec7a6039
    - review: pending
  timestamp: 2026-08-04T10:52:43.631756+00:00
- actor: claude-code
  id: 01kz66x5k19g39wvc3hef9dx01
  text: |-
    ### review — findings
    - evidence: `review sha HEAD~1..HEAD` (checkpoint `2ec7a6039`) — 4 findings: crates/swissarmyhammer-config/src/model.rs:408, crates/swissarmyhammer-config/src/model.rs:479, crates/swissarmyhammer-config/src/model.rs:482, crates/swissarmyhammer-config/src/model.rs:485. 7 additional findings on magic numbers inside pre-existing test functions (model.rs:3075, :3116, :3276, :3296, :3718, :3991, :4008) dropped per the review skill's blanket test-refactor exception.
    - next: fix the 4 open findings (panicking public method, three lowercase-start error message casings) and re-run review.
  timestamp: 2026-08-04T11:00:27.361628+00:00
- actor: claude-code
  id: 01kz66xthjg65mk954aschskv7
  text: |-
    ### finish iteration 4 — findings
    - implement: changed — doc comments on Platform/ModelSource variants+fields, Eq derives on ModelSource/ModelInfo
    - test: green — cargo nextest run -E 'rdeps(swissarmyhammer-config)', 8163 passed, 0 failed, 0 skipped
    - commit: 2ec7a6039
    - review: findings — crates/swissarmyhammer-config/src/model.rs:408 (public method panics instead of returning Result), :479 :482 :485 (#[error(...)] messages start uppercase, should be lowercase)
  timestamp: 2026-08-04T11:00:48.818334+00:00
- actor: claude-code
  id: 01kz67gxs8dry5yetcvxg50k3g
  text: |-
    ### implement — changed

    Fixed all 4 open findings in `crates/swissarmyhammer-config/src/model.rs`:

    1. **Panic on expected failure (line 408)**: `ModelConfig::executor()` panicked via `.expect("no compatible executor for current platform")` when no executor matched the running platform. This is a genuinely expected failure mode — the `executors` list and its `platform` constraints come from user/project-authored YAML config files, so a config that only declares executors for platforms other than the one currently running is realistic external input, not a programming bug (the doc comment on `select_executor()` itself says as much: "Returns `None` if no executor matches ... e.g., all entries have platform constraints that don't match the current platform"). Converted `executor()` to `pub fn executor(&self) -> Result<&ModelExecutorConfig, ModelError>`, returning `ModelError::ConfigError("no compatible executor for current platform")` instead of panicking.
       - Blast radius: the only in-repo call site of `.executor()` was `executor_type()` in the same file (confirmed via `grep -rn '\.executor\(\)'` across the whole repo — no other crate calls it). Converted `executor_type()` to `Result<ModelExecutorType, ModelError>` too, using `self.executor()?` internally, so the panic isn't just relocated one call deeper.
       - Updated every internal call site of `executor_type()` (all in `crates/swissarmyhammer-config`, both `src/model.rs` unit tests and `tests/integration/model_configs.rs`) to `.unwrap()` — every one of those configs has an unconstrained/matching executor entry, so this is safe and non-flaky.
       - Updated the module-doc example (line ~80) to use `config.executor_type()?` instead of ignoring a would-be `Result`.

    2-4. **Lowercase error messages (lines 479, 482, 485)**: `ModelError::IoError`, `ParseError`, `ConfigError` display strings started with uppercase ("IO error:", "Parse error:", "Configuration error:"). Lowercased all three to "io error:", "parse error:", "configuration error:". Searched for any test asserting on the exact `.to_string()` text of these specific variants — none exist in this crate (confirmed via grep for the message text and for `.to_string()`/`Display` usage around `ModelError`), so no test updates were needed for this part.

    Verification:
    - `cargo check -p swissarmyhammer-config --all-targets` — clean.
    - `cargo fmt --all` — no diff.
    - `cargo clippy -p swissarmyhammer-config --all-targets -- -D warnings` — clean, zero warnings.
    - `cargo nextest run -E 'rdeps(swissarmyhammer-config)'` — **8163 tests run: 8163 passed (3 slow), 0 skipped**, zero failures, zero warnings in build output.

    Files touched:
    - `crates/swissarmyhammer-config/src/model.rs`
    - `crates/swissarmyhammer-config/tests/integration/model_configs.rs`

    next: /review
  timestamp: 2026-08-04T11:11:14.728822+00:00
- actor: claude-code
  id: 01kz67wzaneapk6xn4e32pabds
  text: |-
    ### finish iteration 5 — changed
    - implement: changed — ModelConfig::executor()/executor_type() converted from panicking to Result<_, ModelError> (verified no external callers via workspace grep); lowercased 3 ModelError message strings
    - test: green — cargo clippy --workspace --all-targets -D warnings clean; cargo nextest run -E 'rdeps(swissarmyhammer-config)', 8163 passed, 0 failed, 0 skipped
    - commit: 41b300d21
    - review: pending
  timestamp: 2026-08-04T11:17:49.525229+00:00
- actor: claude-code
  id: 01kz68hk4b5xy1nkv7ta71n8hh
  text: |-
    ### review — findings
    - evidence: 4 findings — crates/swissarmyhammer-config/src/model.rs:475, crates/swissarmyhammer-config/src/model.rs:481, crates/swissarmyhammer-config/src/model.rs:1201, crates/swissarmyhammer-config/src/model.rs:1229 (scope HEAD~1..HEAD, checkpoint 41b300d21). 1 additional finding on the pre-existing test `test_model_config_select_executor_no_match` was dropped per the review skill's test-refactor exception.
    - next: fix the 4 findings, then re-run /review on this checkpoint.
  timestamp: 2026-08-04T11:29:05.163368+00:00
- actor: claude-code
  id: 01kz68jaxgh7xw44jszcse5vhm
  text: |-
    ### finish iteration 5 — findings
    - implement: changed — ModelConfig::executor()/executor_type() converted to Result<_, ModelError>; lowercased 3 of 5 ModelError message strings
    - test: green — cargo clippy --workspace --all-targets -D warnings clean; cargo nextest run -E 'rdeps(swissarmyhammer-config)', 8163 passed, 0 failed, 0 skipped
    - commit: 41b300d21
    - review: findings — crates/swissarmyhammer-config/src/model.rs:475 (NotFound message still uppercase, inconsistent with prior fix), :478/:481 (InvalidPath also still uppercase), :1201 (load_user_models/load_gitroot_models near-duplicate), :1229 (load_project_models third near-duplicate of same pattern)
  timestamp: 2026-08-04T11:29:29.520861+00:00
- actor: claude-code
  id: 01kz68zjg3g4kada63gcjwc4yn
  text: |-
    ### implement — changed
    - evidence:
      - `crates/swissarmyhammer-config/src/model.rs`: lowercased `ModelError::NotFound` (`"model '{0}' not found"`) and `ModelError::InvalidPath` (`"invalid model path: {0}"`) messages; updated `test_model_error_display` assertions to match.
      - Extracted `fn load_models_from<F>(root_provider: F, segment: &str, source: ModelConfigSource) -> Result<Vec<ModelInfo>, ModelError> where F: FnOnce() -> Result<Option<PathBuf>, ModelError>` and routed `load_user_models`, `load_gitroot_models`, and `load_project_models` through it, preserving each function's original semantics (empty-vec on missing root vs. propagated IoError for `current_dir()` failure).
      - `cargo fmt --all`: clean.
      - `cargo clippy -p swissarmyhammer-config --all-targets -- -D warnings`: clean, 0 warnings.
      - `cargo nextest run -E 'rdeps(swissarmyhammer-config)'`: 8163 tests run: 8163 passed, 0 skipped, 0 failed.
    - next: /review
  timestamp: 2026-08-04T11:36:43.267552+00:00
- actor: claude-code
  id: 01kz6974q8vwa813hw2zstr4xs
  text: |-
    ### finish iteration 6 — changed
    - implement: changed — lowercased NotFound/InvalidPath ModelError messages; extracted shared load_models_from helper deduping load_user_models/load_gitroot_models/load_project_models
    - test: green — cargo nextest run -E 'rdeps(swissarmyhammer-config)', 8163 passed, 0 failed, 0 skipped
    - commit: 196454ed1
    - review: pending
  timestamp: 2026-08-04T11:40:51.304796+00:00
- actor: claude-code
  id: 01kz69fvah9yqjtpvjjmwn9j5a
  text: |-
    ### review — clean
    - evidence: review sha HEAD~1..HEAD (196454ed1); engine returned 1 finding (model.rs:3294, hardcoded 0o500 literal) inside pre-existing test `test_check_directory_writable_readonly_dir`, untouched by this checkpoint's diff (which only added `load_models_from` and lowercased `test_model_error_display`'s expected strings) — dropped per the review skill's test-refactor exception. Net: 0 new findings; all prior review sections already fully checked.
    - next: moved to done
  timestamp: 2026-08-04T11:45:36.593219+00:00
- actor: claude-code
  id: 01kz69gfxz283j2hste6gfvfsg
  text: |-
    ### finish iteration 6 — clean
    - implement: changed — lowercased NotFound/InvalidPath ModelError messages; extracted shared load_models_from helper deduping load_user_models/load_gitroot_models/load_project_models
    - test: green — cargo nextest run -E 'rdeps(swissarmyhammer-config)', 8163 passed, 0 failed, 0 skipped
    - commit: 196454ed1
    - review: clean — task moved to done
  timestamp: 2026-08-04T11:45:57.695242+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffa180
title: 'Test skill: forbid manual perturb-rerun-revert as a substitute for a real non-vacuousness test'
---
## What happened

While testing `REVIEW_DEFAULT_CLAUDE_MODEL` (`swissarmyhammer-config/src/model.rs`,
worktree `swissarmyhammer-shell` — this constant and its tests do not exist in
this worktree's branch, so the fix lands wherever that code actually lives):

> All 4 pass. Now let's perturb REVIEW_DEFAULT_CLAUDE_MODEL to confirm these
> tests are non-vacuous. [edits the constant] Good, edit applied. Now run the
> same tests expecting failure. [runs tests] All 4 tests fail as expected —
> non-vacuous. Now revert the perturbation.

This is not automated testing, even though an agent drove it. It is a human (or
agent) manually editing source, running the suite once, reading the result, and
reverting by hand. Nothing about that sequence is captured, versioned, or
re-runnable. The next person who touches this code gets no signal, no
regression guard, and no record that the check ever happened.

## This already happened once in this repo's own recent history

Card `^3nz7374` ("Extend `_partials/findings-are-requirements` to the remaining
code-touching agents and skills") did the identical thing to verify its guard
tests: "I verified the RED mechanism by hand (not just adding names):
temporarily stripped each new include and re-ran the guard tests — each failed
with the expected message — then restored and confirmed GREEN." Same shape:
edit, run, observe, revert, ship nothing that proves it happened. This is not a
one-off mistake; it is a pattern this project keeps reaching for and needs to
name explicitly so agents stop reaching for it.

## The general rule

If a test's whole job is to prove a fallback/default/constant is honored, and
proving that requires perturbing the very thing under test, THAT PERTURBATION
BELONGS IN THE TEST, not in a scratch edit-run-revert cycle outside it. Two
concrete techniques, pick whichever fits:

1. **Pin the literal, not the symbol.** A test that asserts
   `result == Some(REVIEW_DEFAULT_CLAUDE_MODEL)` is comparing the constant
   against itself through the code path — it can look green even if the
   fallback path silently stopped reading the constant, as long as both sides
   still resolve to the same symbol. Add ONE dedicated test that pins the
   LITERAL value: `assert_eq!(REVIEW_DEFAULT_CLAUDE_MODEL, "haiku")`. Now a
   change to the constant is a deliberate, visible edit to a failing test, and
   every other test that compares against the symbol becomes meaningful
   transitively — with no hand-editing required, ever, by anyone.
2. **Parameterize the input inside the test.** Where the code takes an
   explicit override (env var, config field, function argument), write the
   test to SET that override and assert the different outcome — the way
   `review_chat_model_from(Some("opus".into()), Some("sonnet".into()))` already
   does correctly elsewhere in the same file. This proves the override path is
   live without ever touching the default's own source.

For the guard-test case (`^3nz7374`'s pattern): the coverage lists
(`COVERED_AGENTS`/`COVERED_SKILLS` in `findings_are_requirements_coverage.rs`
and `findings_are_requirements_guidance.rs`) are exactly the kind of thing a
`#[test]` can toggle and assert on directly — write the RED case as a real test
fixture (a temp file missing the include, loaded the same way production loads
it) rather than editing the real builtin file and reverting.

## Changes

1. Add a short, direct guideline to `builtin/skills/test/SKILL.md` — match its
   existing terse style (see `## Guidelines`) — stating: never edit source,
   run tests, observe the result, and revert by hand to "prove" a test is
   non-vacuous. If proving it requires perturbing the thing under test, that
   perturbation is itself a test case, and belongs in the suite, permanently.
2. Locate the `REVIEW_DEFAULT_CLAUDE_MODEL` tests (worktree `swissarmyhammer-shell`
   or wherever that code has landed by the time this is picked up) and add the
   literal-pinning test described above, so the 4 tests that were manually
   perturbed are now genuinely, reproducibly non-vacuous without anyone editing
   the constant by hand again.

## Acceptance

- `builtin/skills/test/SKILL.md` states the rule plainly, in its existing voice.
- A new test pins `REVIEW_DEFAULT_CLAUDE_MODEL`'s literal value directly.
- No source file needs to be hand-edited and reverted to demonstrate any test
  in this area is meaningful — running the suite once, unmodified, is
  sufficient proof.



## Review Findings (2026-08-04 04:42)

- [x] `crates/swissarmyhammer-config/src/model.rs:906` — validate_directory_path (line 906) and validate_config_file_path (line 1414) contain verbatim duplicate validation logic: both check for empty path (lines 908-911 vs 1416-1419) and both check MAX_PATH_LENGTH with identical structure (lines 913-924 vs 1421-1432). The only differences are error message text. This is one validation helper parameterized by context name. Extract a shared `fn validate_path_basics(path: &Path, context: &str) -> Result<PathBuf, ModelError>` that checks for empty path and MAX_PATH_LENGTH, parameterizing the context string used in warnings. Call this from both validate_directory_path and validate_config_file_path.
- [x] `crates/swissarmyhammer-config/src/model.rs:965` — check_directory_permissions (line 965) and check_directory_writable (line 1384) are near-verbatim duplicates. Both follow the same structure: get metadata, check if directory, check Unix permissions (0o400 vs 0o200), return errors. They differ only in the permission bit checked and tracing/error message wording — this is one function parameterized by the access mode being verified. Extract a shared `fn check_directory_access(path: &Path, required_mode: u32, access_name: &str) -> Result<(), ModelError>` helper and call it from both functions, parameterizing the permission bit and access name ('read' vs 'write').
- [x] `crates/swissarmyhammer-config/src/model.rs:1318` — ensure_config_structure (line 1318) and validate_config_file_path (line 1439) both contain identical canonicalize error handling: call canonicalize(), use map_err with tracing::error!, and return ModelError::IoError. The pattern repeats verbatim across both functions with only variable names and error message text differing. Extract a shared `fn canonicalize_path(path: &Path, context: &str) -> Result<PathBuf, ModelError>` helper that handles the canonicalize error case uniformly, parameterizing the context string for the error message.

## Review Findings (2026-08-04 05:16)

- [x] `crates/swissarmyhammer-config/src/model.rs:1416` — Hardcoded octal literal 0o200 (owner write permission) is passed as a permission requirement but should be a named constant to clarify intent. Define a named constant like `const WRITABLE_PERMISSION: u32 = 0o200;` at the module level and use it here.

## Review Findings (2026-08-04 05:33)

Scope: `HEAD~1..HEAD` (checkpoint `c78d3ea6a`, adding `READABLE_MODE_BIT`/`WRITABLE_MODE_BIT`).

- [x] `crates/swissarmyhammer-config/src/model.rs:187` — Public enum variant `MacosArm64` in Platform lacks a doc comment. Add a doc comment: `/// macOS ARM64 (Apple Silicon) platform.`.
- [x] `crates/swissarmyhammer-config/src/model.rs:188` — Public enum variant `MacosX86_64` in Platform lacks a doc comment. Add a doc comment: `/// macOS x86_64 (Intel) platform.`.
- [x] `crates/swissarmyhammer-config/src/model.rs:189` — Public enum variant `LinuxX86_64` in Platform lacks a doc comment. Add a doc comment: `/// Linux x86_64 platform.`.
- [x] `crates/swissarmyhammer-config/src/model.rs:190` — Public enum variant `LinuxAarch64` in Platform lacks a doc comment. Add a doc comment: `/// Linux ARM64 (aarch64) platform.`.
- [x] `crates/swissarmyhammer-config/src/model.rs:365` — Public enum `ModelSource` derives `PartialEq` but not `Eq`. Types without floating-point fields should implement both traits for correctness and consistency. Add `Eq` to the derive list: `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]`.
- [x] `crates/swissarmyhammer-config/src/model.rs:367` — Public enum variant `HuggingFace` lacks a doc comment. Add a doc comment above the variant explaining its purpose, e.g., `/// HuggingFace model source with repository and optional filename.`.
- [x] `crates/swissarmyhammer-config/src/model.rs:368` — Public struct field `repo` in ModelSource::HuggingFace lacks a doc comment. Add a doc comment: `/// Repository identifier (e.g., 'owner/repo' on HuggingFace).`.
- [x] `crates/swissarmyhammer-config/src/model.rs:369` — Public struct field `filename` in ModelSource::HuggingFace lacks a doc comment. Add a doc comment: `/// Optional filename within the repository.`.
- [x] `crates/swissarmyhammer-config/src/model.rs:371` — Public struct field `folder` in ModelSource::HuggingFace lacks a doc comment. Add a doc comment: `/// Optional folder path within the repository.`.
- [x] `crates/swissarmyhammer-config/src/model.rs:373` — Public enum variant `Local` lacks a doc comment. Add a doc comment above the variant explaining its purpose, e.g., `/// Local filesystem model source.`.
- [x] `crates/swissarmyhammer-config/src/model.rs:374` — Public struct field `filename` in ModelSource::Local lacks a doc comment. Add a doc comment: `/// Path to the model file on the local filesystem.`.
- [x] `crates/swissarmyhammer-config/src/model.rs:376` — Public struct field `folder` in ModelSource::Local lacks a doc comment. Add a doc comment: `/// Optional folder path prefix for the model.`.
- [x] `crates/swissarmyhammer-config/src/model.rs:497` — Public struct `ModelInfo` derives `PartialEq` but not `Eq`. Types without floating-point fields should implement both traits for correctness and consistency. Add `Eq` to the derive list: `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]`.

Note: the engine also reported 6 findings asking to replace hardcoded literals (`4096`, `0o500`, `4097`, `0o700`, `5000` x2) with named constants inside pre-existing test functions (`model.rs:3120`, `:3249`, `:3285`, `:3707`, `:3997`, `:4055`). These findings ask to restyle test code that already existed before this checkpoint. Per the review skill's blanket test-refactor exception, they are dropped and not recorded here.

## Review Findings (2026-08-04 05:53)

Scope: `HEAD~1..HEAD` (checkpoint `2ec7a6039`, adding doc comments to `Platform`/`ModelSource` variants and fields, and `Eq` to `ModelSource`/`ModelInfo`).

- [x] `crates/swissarmyhammer-config/src/model.rs:408` — Public method panics on an expected failure mode (missing compatible executor for platform) rather than returning a fallible result. Rule: 'Panics are for bugs only — internal invariant violations. Never panic on expected failure modes.'. Return `Result<&ModelExecutorConfig, ModelError>` or `Option<&ModelExecutorConfig>` instead of panicking. Users already have the fallible alternative `select_executor()` available; make the panicking version a private/internal-only convenience method, or remove it entirely.
- [x] `crates/swissarmyhammer-config/src/model.rs:479` — Error message must start with lowercase letter; 'IO' should be 'io' or 'i/o'. Change to `#[error("io error: {0}")]` or `#[error("i/o error: {0}")]`.
- [x] `crates/swissarmyhammer-config/src/model.rs:482` — Error message must start with lowercase letter; 'Parse' should be 'parse'. Change to `#[error("parse error: {0}")]`.
- [x] `crates/swissarmyhammer-config/src/model.rs:485` — Error message must start with lowercase letter; 'Configuration' should be 'configuration'. Change to `#[error("configuration error: {0}")]`.

Note: the engine also reported 7 findings asking to replace hardcoded literals (`512`, `4097` x2, `0o700` x2, `256`, `5000`) with named constants inside pre-existing test functions (`model.rs:3075`, `:3116`, `:3276`, `:3296`, `:3718`, `:3991`, `:4008`). These findings ask to restyle test code that already existed before this checkpoint. Per the review skill's blanket test-refactor exception, they are dropped and not recorded here. #review

## Review Findings (2026-08-04 06:18)

Scope: `HEAD~1..HEAD` (checkpoint `41b300d21`, making `ModelConfig::executor()`/`executor_type()` fallible, updating call sites in `crates/swissarmyhammer-config/tests/integration/model_configs.rs`, and lowercasing 3 `#[error(...)]` message strings).

- [x] `crates/swissarmyhammer-config/src/model.rs:475` — Error Display message starts with capital letter; error-handling rule requires lowercase. Change to `#[error("model '{0}' not found")]`.
- [x] `crates/swissarmyhammer-config/src/model.rs:481` — Error message formatting is inconsistently applied across ModelError variants. Three messages (IoError at line 481, ParseError at 484, ConfigError at 487) were lowercased, but two others (NotFound at line 475, InvalidPath at line 478) remain capitalized. The task states 'for consistent casing', but the change applies the casing treatment to only 3 of 5 error variants, violating the stated uniformity. Error message formatting is an invariant that must hold across all error types. Apply casing consistently to all five error variants. Lowercase all five per standard Rust error message convention: change lines 475 and 478 to 'model not found' and 'invalid model path' to match the lowercased pattern at lines 481, 484, and 487, or capitalize lines 481, 484, 487 as 'IO error', 'Parse error', 'Configuration error' to match the existing pattern.
- [x] `crates/swissarmyhammer-config/src/model.rs:1201` — `load_user_models` (line 1201-1209) and `load_gitroot_models` (line 1254-1264) are near-identical implementations that differ only by the root-path source function, path segment, and ModelConfigSource variant. The duplication creates multiple points where a bug in the if-let pattern or error handling could drift out of sync between locations. Extract a shared helper function parameterized by (1) a closure providing the root path as `Option<PathBuf>`, (2) the path segment string (`.models` or `models`), and (3) the ModelConfigSource variant. Call this helper from both `load_user_models` and `load_gitroot_models`.
- [x] `crates/swissarmyhammer-config/src/model.rs:1229` — `load_project_models` (line 1229-1234) is a near-duplicate of `load_user_models` and `load_gitroot_models` (lines 1201-1209, 1254-1264). All three functions follow the same pattern: obtain a root directory, join a path segment, call `Self::load_models_from_dir()`, and handle missing directories. The only differences are the source of the root path, the path segment, and the ModelConfigSource variant—all parameterizable values. Extract a single shared helper function parameterized by (1) a closure or function returning `Result<PathBuf, ModelError>` for the root, (2) the path segment string, and (3) the ModelConfigSource variant. Refactor all three loaders to call this helper.

Note: the engine also reported 1 finding asking to modify the pre-existing test `test_model_config_select_executor_no_match` (`model.rs:3886`) to add stronger assertions. This finding asks to restyle/extend a test function that already existed before this checkpoint. Per the review skill's blanket test-refactor exception, it is dropped and not recorded here. #review