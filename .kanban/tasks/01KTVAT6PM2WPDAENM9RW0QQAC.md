---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff9e80
project: local-review
title: 'fix(review): `review working` scope must include untracked files'
---
## What

A full `/finish` run in ../calcutron (2026-06-11) produced 8 × `review working` calls that ALL resolved to an empty scope — `review scope resolved validators=[] validator_count=0 files=0`, `attempted: 0` — even though the loop had just written an entire app (`Cargo.toml`, `src/*.rs` ×8, `tests/`), all untracked on a June 4 HEAD. Each review returned an empty findings report in seconds, zero model generation, and reviewer agents read it as "clean". Untracked files MUST be reviewed — they are precisely the code that WILL be added.

**Precise root cause (verified in source):** `resolve_working` in `crates/swissarmyhammer-validators/src/review/scope.rs` DOES include `status.untracked` — but `GitOperations::get_status` in `crates/swissarmyhammer-git/src/operations.rs` sets `opts.include_untracked(true)` WITHOUT `opts.recurse_untracked_dirs(true)`. libgit2 therefore reports an untracked directory as ONE entry (`src/`) instead of its files. So calcutron's scope was `["CLAUDE.md", "Cargo.toml", "src/", "tests/", ...]` — `read_working("src/")` fails (directory), and no validator glob matches `src/` → validators=[] → files=0. The existing unit test only covers a top-level untracked FILE (`untracked.txt`), which is why it passes.

Fix:

- [x] Add `opts.recurse_untracked_dirs(true)` to `GitOperations::get_status` (crates/swissarmyhammer-git/src/operations.rs) so untracked directories expand to their contained files (`.gitignore` still respected via `include_ignored(false)`).
- [x] Audit other `get_status` callers for behavior change (workspace-wide grep) — the summary lists more untracked paths now. (Callers: `is_working_directory_clean` — semantics unchanged; `get_uncommitted_changes` in swissarmyhammer-tools git changes — now lists file paths instead of bare dir entries, the intended improvement; all suites green.)
- [x] Filter UNTRACKED scope entries to code files using the EXISTING canonical extension list `get_all_code_extensions()` in `crates/swissarmyhammer-sem/src/parser/plugins/code/languages.rs` (exported via `pub use` from the code plugin module; reused in scope.rs `is_code_file` — no second extension list). Tracked modifications keep current behavior.
- [x] An empty resolved scope is unmistakable in the report markdown: `synthesize` renders "Nothing in scope to review." under the dated header when zero tasks were attempted and no findings kept.

## Acceptance Criteria

- [x] In a repo with committed history plus a brand-new untracked directory of source files, `review working` resolves files > 0, the rust validator matches the new `.rs` files, and the fleet runs (attempted > 0). (`working_scope_includes_untracked_nested_source_files`)
- [x] `.gitignore`d files/dirs are NOT swept into scope. (`test_get_status_recurses_untracked_directories_into_files`)
- [x] Untracked non-code files (logs, jsonl, toml-only junk) do not get content-read into scope. (`working_scope_excludes_untracked_non_code_files`)
- [x] A genuinely clean tree returns a report whose markdown explicitly states nothing was in scope. (`an_empty_scope_renders_the_nothing_in_scope_marker`)

## Tests

- [x] crates/swissarmyhammer-git: extended get_status test — untracked nested dir `src/new.rs` appears as `src/new.rs` (file path, not `src/`); gitignored file excluded.
- [x] crates/swissarmyhammer-validators scope tests: untracked `src/new.rs` → in scope and matches the rust validator; untracked `logs/run.log` → excluded by the code-extension filter; tracked non-code modification stays in scope; empty scope → "Nothing in scope to review." marker (plus a no-marker test for an attempted clean run).
- [x] `cargo test -p swissarmyhammer-git -p swissarmyhammer-validators -p swissarmyhammer-tools` green (also -p swissarmyhammer-sem), all unit tests <10s, no model. `cargo clippy --all-targets -- -D warnings` clean for all four crates.

## Workflow
- Used `/tdd` — each change red-green: git recursion test failed on `["src/"]` first; scope exclusion test failed on `["logs/run.log"]` first; marker test failed on the bare header first.

## Evidence

../calcutron/.sah/mcp.38775.log (2026-06-11 12:22–12:23 UTC): 8 × `review working` → each `review scope resolved validators=[] validator_count=0 files=0`, `review verify complete candidates=0`, result `{"markdown": "## Review Findings (2026-06-11 07:22)\n", "counts": {... "attempted": 0, "failed": 0}}` in 1–16s, zero llama generation. calcutron `git status`: `?? src/`, `?? tests/`, `?? Cargo.toml` — directories unexpanded, HEAD = June 4.

## Review Findings (2026-06-11 08:55)

Scope: `review file` on the four changed files (working tree carries unrelated changes). Acceptance criteria spot-checked clean: scope.rs reuses `get_all_code_extensions` from swissarmyhammer-sem (no second list), `.gitignore` stays excluded via `include_ignored(false)`, and the "Nothing in scope to review." marker is gated on `attempted == 0 && kept.is_empty()` only.

### Blockers
- [x] `crates/swissarmyhammer-validators/src/review/scope.rs:730` — The entire test-fixture suite in the scope.rs tests module — the TestRepo git fixture (new/write/commit), the code_context index fixture (index_conn, seed_file, seed_chunk, seed_symbol, seed_call_edge), and the validator-loader fixture (loader_with, ruleset, body) — is a verbatim copy of fixtures already present in two sibling files of the same review module. That makes three byte-identical copies inside one crate (scope.rs, drive.rs, probes.rs), past the rule of three. A schema change to indexed_files/ts_chunks, a TestRepo behavior fix, or a RuleSetManifest field addition now has to be applied three times in this crate alone, and a copy that is missed becomes a silently divergent fixture. Extract one crate-internal `#[cfg(test)]` test-support module for the review pipeline (e.g. `src/review/test_support.rs`, gated with `#[cfg(test)] pub(crate) mod test_support;`) holding TestRepo, index_conn, seed_file, seed_chunk, seed_symbol, seed_call_edge, loader_with, ruleset, and body. Have the tests in scope.rs, drive.rs, and probes.rs import from it and delete the three local copies. The cross-crate copies (swissarmyhammer-tools review tests, the agent e2e TinyRepo) are a follow-up, but the three in-crate copies share a module boundary and have no excuse. **Resolved:** extracted `crates/swissarmyhammer-validators/src/review/test_support.rs` (`#[cfg(test)] pub(crate)`) holding TestRepo, index_conn, seed_file, seed_chunk, seed_symbol, seed_call_edge, loader_with, ruleset (severity is now a parameter — the only axis the three copies varied on), body, DIM, and dup_emb; all three local copies deleted. Cross-crate follow-up filed as task 01KTVH4DV1BNZBTQX9TPT0G8SY.

### Warnings
- [x] `crates/swissarmyhammer-git/src/operations.rs:297` — The status-classification loop in `get_status` is an 8-arm `if` chain over the known set of `git2::Status` flags where every arm differs only in two constants: the flag tested and the `StatusSummary` field pushed to. This is a table expressed as control flow — adding or changing a status category means editing parallel arms a human must keep in lockstep with `StatusSummary`, and the change under review just modified this function, so it is live code, not frozen history. Replace the chain with a single loop over a flag→field table. **Resolved:** `STATUS_BUCKETS` const table (`(git2::Status, StatusBucket)` rows, `type StatusBucket = fn(&mut StatusSummary) -> &mut Vec<String>`) interpreted by one loop; a new status category is a one-row addition.
- [x] `crates/swissarmyhammer-sem/src/parser/plugins/code/mod.rs:52` — `let _ = p.set_language(&language)` swallows the `LanguageError`, and the language-less parser is still inserted into PARSER_CACHE by `or_insert_with`. Because the cache entry is permanent for the thread, one version-mismatch failure means every subsequent file of that language silently parses to `None` → empty entities, forever, with no signal anywhere that extraction is broken. **Resolved:** rewrote the cache insert as an explicit `Entry` match — on `set_language` failure it emits `tracing::warn!(language, error, ...)` and returns without caching, so the next call retries; only a successfully configured parser is ever cached (tracing added to the sem crate's deps). No dedicated unit test: a `LanguageError` requires an ABI-incompatible grammar that cannot be constructed against the workspace's pinned tree-sitter versions; the fix is contained and the full 184-test sem suite stays green.
- [x] `crates/swissarmyhammer-validators/src/review/scope.rs:399` — The new `is_code_file` reimplements swissarmyhammer-sem's extension-matching convention (extract extension → lowercase → prepend dot → membership in `get_all_code_extensions()`) rather than that crate exporting the predicate itself. **Resolved:** moved the predicate into swissarmyhammer-sem languages.rs as `pub fn is_code_file(path: &str) -> bool` built on a shared `dotted_lowercase_extension` helper that `CodeParserPlugin::extract_entities` now also uses; re-exported from the code plugin module; scope.rs deleted its copy and imports `is_code_file`. The dotted-lowercase convention has exactly one owner, with unit tests (known extensions, case normalization, non-code/extensionless rejection).
- [x] `crates/swissarmyhammer-validators/src/review/scope.rs:1205` — The test `two_validators_share_one_probe_run_for_the_same_file` claims to prove the probe was not re-run per validator, but its assertion cannot detect a re-run (deterministic mock → byte-identical results either way). **Resolved:** the test now asserts on execution count via `MockEmbedder::call_count()` — a single-validator baseline run fixes the expected embed count, and the two-validator run must match it exactly (a per-validator re-run multiplies it). Red-verified: temporarily re-running probes per validator made the test fail (left: 2, right: 1), then reverted. The fan-out equality assertion is kept as the secondary check.

### Nits
- [x] `crates/swissarmyhammer-sem/src/parser/plugins/code/mod.rs:14` — Public struct `CodeParserPlugin` has no doc comment. **Resolved:** doc comment added describing the tree-sitter multi-language entity extraction and its `SemanticParserPlugin` relationship.
- [x] `crates/swissarmyhammer-sem/src/parser/plugins/code/mod.rs:14` — `pub struct CodeParserPlugin;` (and the `pub use languages::get_all_code_extensions` re-export at line 4) have no doc comments. **Resolved:** doc comments added on the struct, both re-exports (`get_all_code_extensions`, the new `is_code_file`), and the underlying functions in languages.rs.
- [x] `crates/swissarmyhammer-validators/src/review/scope.rs:818` — The `seed_symbol` helper hardcodes the LSP symbol kind as the bare literal `12`. **Resolved:** `const LSP_SYMBOL_KIND_FUNCTION: i64 = 12;` in the shared test_support module, bound as a SQL parameter.
- [x] `crates/swissarmyhammer-validators/src/review/scope.rs:954` — The mock embedding literal `vec![1.0, 0.0, 0.0, 0.0]` is repeated and its length silently duplicates `DIM`. **Resolved:** `dup_emb()` helper in test_support derives the vector length from `DIM`; all copies in scope.rs, drive.rs, and probes.rs replaced.
- [x] `crates/swissarmyhammer-validators/src/review/synthesize.rs:413` — The test timestamp literal "2026-04-11 13:08" is repeated 16 times. **Resolved:** `const NOW` at the top of the test module, passed as the `now` argument everywhere; the literal stays inline only in the byte-for-byte snapshot/expected strings, per the finding's guidance.

## Review Resolution (2026-06-11)

All findings worked, none skipped. Verification: `cargo test -p swissarmyhammer-git -p swissarmyhammer-sem -p swissarmyhammer-validators -p swissarmyhammer-tools` fully green (96 + 184 + 237 + 1075 lib tests plus all integration suites, validators lib 3.0s / git 1.8s — no model, all unit tests well under 10s); `cargo clippy --all-targets -- -D warnings` clean on all four crates. One unrelated flake observed once under full parallel load (`mcp::file_watcher::tests::test_start_builds_without_shared_lock`); it passes standalone and in two subsequent full runs and touches no changed code.