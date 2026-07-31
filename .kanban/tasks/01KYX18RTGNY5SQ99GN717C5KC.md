---
assignees:
- claude-code
position_column: todo
position_ordinal: d580
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
