---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kyxazg0zx34q836njbw8j32z
  text: |-
    Implemented all 3 typed pairs in `crates/swissarmyhammer-validators/src/review/scope.rs`.

    1. `FileChangeBuilder::push(path: FilePath, versions: FileVersions)` — the two sides are now one named-field `FileVersions { before: BeforeContent, after: AfterContent }`. The card asked for a struct; the dispatch also required the two sides to be genuinely distinct types, so the struct carries two DIFFERENT newtypes. A transposed struct literal is `error[E0308]: mismatched types`, not just a mis-named field.
    2. `ValidatorWork::new(validator_name, rules: RuleNames, probes: ProbeNames, files)` — two `#[serde(transparent)]` newtypes, so the serialized work-list payload does not change. `MatchedValidator` carries the same two types, so the internal path is typed end to end. The `rules()`/`probes()` accessors still return `&[String]`, so no consumer changed.
    3. `select_probe_results(cache, file, changed_symbols: &[String], probes: &ProbeNames)` — reuses `ProbeNames` instead of a parallel newtype.

    Also added `FilePath::into_string()` so `push` unwraps the path once.

    `AfterContent::absent()` was written and then deleted: no production site needs it (only `BeforeContent::absent()` is used, by the glob scope), so it was dead code and `-D warnings` rejected it. The deletion side of the test uses `AfterContent::new(None)`.

    Mechanical sweep tool at /tmp/sweep.py: parses every `fn` signature, splits parameters at top-level commas, compares adjacent types. Validated against `git show HEAD:...scope.rs` — it reports exactly the 3 pairs the card names. On the new file: 151 functions, 44 adjacent pairs, 0 same-typed.
  timestamp: 2026-08-01T00:18:27.999245+00:00
- actor: claude-code
  id: 01kyxbd6ev4vpg4cdy3ngd21j6
  text: |-
    Verified. Evidence:

    RED proof — deliberately transposing each pair and running `cargo check -p swissarmyhammer-validators --all-targets` gives 4 errors across the 3 pairs:
    - `error[E0308]: mismatched types` ... `before: AfterContent::new(...)` — "expected `BeforeContent`, found `AfterContent`"; `after: BeforeContent::new(...)` — "expected `AfterContent`, found `BeforeContent`"
    - `error[E0308]: arguments to this function are incorrect` on `ValidatorWork::new` — "expected `scope::RuleNames`, found `scope::ProbeNames`" and the mirror
    - `error[E0308]: mismatched types` on `select_probe_results` — "expected `&ProbeNames`, found `&Vec<String>`"
    The transpositions were reverted; `cargo check` is clean.

    GREEN — `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'`: 5020 tests run, 5020 passed, 2 skipped. `cargo fmt --all` applied. `cargo clippy --workspace --all-targets -- -D warnings` clean.

    Sweep — scope.rs after the change: 151 functions parsed, 44 adjacent parameter pairs compared, 0 same-typed. The same tool on HEAD reports exactly the 3 pairs the card names, so it is not vacuously passing.

    Outside scope.rs (REPORT ONLY, not touched — needs a separate scope decision): 8 same-typed adjacent pairs in non-test crate code —
    - review/types.rs `extract_json_value(open: char, close: char)` and `matching_delimiter(open: char, close: char)`
    - review/fleet.rs `handle_fork_failed(parent_session_id: String, message: String)` and `parse_task_response(content: &str, validator: &str)`
    - review/drive.rs `confine_under_repo(repo_root: &Path, requested: &Path)`
    - review/synthesize.rs `FleetTally::new(attempted: usize, failed: usize)`
    - validators/loader.rs `add_builtin(name: &str, content: &str)` and `add_builtin_include(name: &str, content: &str)`
    (26 more sit in test helpers/fixtures.)

    An independent double-check agent re-derived the four `builder.push` call sites against HEAD (no side swapped), confirmed `push` produces identical `SemFileChange` values, confirmed the tracing output and the serialized payload are unchanged, and ran its own signature parser: PASS.
  timestamp: 2026-08-01T00:25:56.955410+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffff8980
title: 'scope.rs: type the 3 remaining transposable adjacent parameter pairs'
---
# Why

Card ^t6tw0kg fixed a review finding on `read_at_ref`, whose adjacent `refspec: &str` / `path: &str` parameters could be transposed silently. That fix was scoped by its instructions to `&str`/`String` pairs, and the remaining instances were to be REPORTED for an explicit scope decision rather than refactored silently. This is that report, as a card.

A mechanical sweep of all 54 production function signatures in `crates/swissarmyhammer-validators/src/review/scope.rs` (parse each signature, compare each adjacent parameter pair's type) found exactly 3 remaining adjacent same-typed pairs. None is a `&str`/`String` pair, which is why card ^t6tw0kg left them.

# The 3 instances

1. **`FileChangeBuilder::push(&mut self, path: &str, before: Option<String>, after: Option<String>)`** — private to the file. **This is the most serious one, arguably worse than the pair that was fixed.** It is called on the line immediately after each `read_at_ref` pair in `resolve_working`, `resolve_sha` and `resolve_file` (`builder.push(path, before, after)`). Transposing the two arguments compiles silently and inverts `FileStatus::Added` <-> `FileStatus::Deleted` plus `before_content`/`after_content` for the whole review. The refspec/path case that was fixed produced a nonsense address that resolves to nothing; this one produces a plausible-looking INVERTED diff, which is harder to notice.
2. **`ValidatorWork::new(validator_name: String, rules: Vec<String>, probes: Vec<String>, files: Vec<FileWork>)`** — `rules` and `probes` are adjacent `Vec<String>` with different meanings (rule names vs probe names). `pub`, with 3 call sites, all inside this crate (`review/scope.rs`, `review/synthesize.rs`, `review/fleet/tests.rs`).
3. **`select_probe_results(probe_cache: &[ProbeResult], file: &str, changed_symbols: &[String], probes: &[String])`** — `changed_symbols` and `probes` are adjacent `&[String]` with different meanings (symbol names vs probe names). Private to the file.

# Changes

1. `FileChangeBuilder::push` — replace the two `Option<String>` parameters with ONE argument that names the sides, so they cannot be swapped. Prefer a single struct (e.g. `struct FileVersions { before: Option<String>, after: Option<String> }`) over two newtypes: the struct makes the call sites read as named fields and removes the positional risk entirely.
2. `ValidatorWork::new` — give `rules` and `probes` distinct types, or take them via a small builder. Update all 3 call sites.
3. `select_probe_results` — give `changed_symbols` and `probes` distinct types, or reorder so the two `&[String]` parameters are not adjacent.

Follow the pattern card ^t6tw0kg established: newtypes wrap an owned value with a private field and purposeful constructors, following `swissarmyhammer_git::BranchName`. Do NOT use `swissarmyhammer_common::define_id!` — it emits a `pub` field and a `new()` that mints a fresh ULID, which is an ID generator, not a domain-value wrapper.

# Acceptance

- No production function in `scope.rs` has two adjacent same-typed parameters carrying different semantics. Verify mechanically by parsing every signature, not by eye.
- A test per change proving the intended argument order, in the style of `read_at_ref_addresses_the_path_within_the_refspec_never_the_transposition`.
- `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` passes.
- `cargo fmt --all`; `cargo clippy --workspace --all-targets -- -D warnings` clean. #review
