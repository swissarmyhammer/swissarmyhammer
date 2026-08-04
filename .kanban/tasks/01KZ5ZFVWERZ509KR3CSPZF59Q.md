---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kz69yy2v779pvs3z9k310g7y
  text: |-
    ### implement — changed

    Findings fixed in `crates/swissarmyhammer-validators/src/review/probes.rs`:

    1. `render_probe_evidence` complexity fix: extracted the header render into `render_result_header(out, result, show_kind)`, the rows loop into `render_result_rows(out, result)`, and the per-row formatting into `render_probe_row(out, row)`. `render_probe_evidence` is now the early-return-on-empty plus a two-call-per-iteration loop.
       - Verified with the real scorer (`swissarmyhammer_sem::parser::plugins::code::cognitive_complexity`) via a temporary throwaway test that parsed the actual file and printed each function's score, then removed the test. Before: `render_probe_evidence` = 24 (over gate 15). After: `render_probe_evidence` = 2, `render_result_header` = 4, `render_result_rows` = 2, `render_probe_row` = 4. `run_probes` = 5, `resolve_entries` = 0. All well under the gate of 15.

    2. `run_probes` signature: `probe_names: &[String]` → `probe_names: &[&str]`. `resolve_entries` (its helper) updated the same way; its error path now uses `name.to_string()` instead of `name.clone()` since `name` is `&&str`, not `&String`.
       - Updated the one production call site: `crates/swissarmyhammer-validators/src/review/scope.rs::run_probe_cache` — builds `name_refs: Vec<&str> = names.iter().map(String::as_str).collect()` and passes `&name_refs`.
       - Updated all 7 test call sites in `probes.rs` to pass string-literal slices (e.g. `&["callers"]`) instead of `&["callers".to_string()]`.
       - Grepped the whole workspace for `run_probes(` — no other call sites exist outside this crate.

    Evidence:
    - `cargo build -p swissarmyhammer-validators --tests` — clean.
    - `cargo nextest run -p swissarmyhammer-validators` — 372 tests run: 372 passed, 0 skipped.
    - `cargo fmt --all` — no additional diff beyond the intended edits (`git diff --stat -- crates/` shows only `probes.rs` and `scope.rs`, 69 insertions / 47 deletions).
    - `cargo clippy --workspace --all-targets -- -D warnings` — clean, 0 warnings.
    - `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` — 3079 tests run: 3079 passed (2 slow), 0 skipped. No ripple effects.

    next: /review
  timestamp: 2026-08-04T11:53:50.939860+00:00
- actor: claude-code
  id: 01kz6a3te7rk9z6tnb4qxkdf3f
  text: |-
    ### finish iteration 1 — changed
    - implement: changed — split render_probe_evidence (complexity 24→2) into 4 functions; run_probes changed &[String] to &[&str], 1 call site + 7 tests updated
    - test: green — cargo nextest run -E 'rdeps(swissarmyhammer-validators)', 3079 passed, 0 failed, 0 skipped
    - commit: 0ad0390e9
    - review: pending
  timestamp: 2026-08-04T11:56:31.047473+00:00
- actor: claude-code
  id: 01kz6af4q8mg6dt96s1vcscs68
  text: |-
    ### review — findings
    - evidence: 1 finding — crates/swissarmyhammer-validators/src/review/probes.rs:568
    - scope: review sha HEAD~1..HEAD (checkpoint 0ad0390e9)
    - next: extract CALLGRAPH_MAX_DEPTH named constant at probes.rs:568; task stays in review
  timestamp: 2026-08-04T12:02:42.024506+00:00
- actor: claude-code
  id: 01kz6afjts95rg79tk3fy0kgjn
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — split render_probe_evidence (complexity 24→2) into 4 functions; run_probes changed &[String] to &[&str], 1 call site + 7 tests updated
    - test: green — cargo nextest run -E 'rdeps(swissarmyhammer-validators)', 3079 passed, 0 failed, 0 skipped
    - commit: 0ad0390e9
    - review: findings — crates/swissarmyhammer-validators/src/review/probes.rs:568 (hardcoded max_depth: 1 in run_callers needs a named constant)
  timestamp: 2026-08-04T12:02:56.473372+00:00
- actor: claude-code
  id: 01kz6aq4yd7nr6czpp5ga37gwh
  text: |-
    ### implement — changed
    - evidence: Added `const CALLGRAPH_MAX_DEPTH: u32 = 1;` with a doc comment above `run_callers` in crates/swissarmyhammer-validators/src/review/probes.rs, explaining the `callers` probe only needs direct (one-hop) inbound callers as a fact-check, not transitive callers-of-callers. Replaced the hardcoded `max_depth: 1` in the `CallGraphOptions` construction inside `run_callers` with `max_depth: CALLGRAPH_MAX_DEPTH`.
    - verification: `cargo fmt --all` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` — 3079 tests run: 3079 passed, 0 skipped, 0 failed.
    - Checked off the one open finding in `## Review Findings`.
    next: /review
  timestamp: 2026-08-04T12:07:04.397273+00:00
- actor: claude-code
  id: 01kz6avyqsfhnb1f23b9mvws6b
  text: |-
    ### finish iteration 2 — changed
    - implement: changed — added CALLGRAPH_MAX_DEPTH named constant with doc comment, replacing hardcoded max_depth: 1 in run_callers
    - test: green — cargo nextest run -E 'rdeps(swissarmyhammer-validators)', 3079 passed, 0 failed, 0 skipped
    - commit: 0521177dd
    - review: pending
  timestamp: 2026-08-04T12:09:41.881299+00:00
- actor: claude-code
  id: 01kz6b3qc6597qm9xvnsdaj7f2
  text: |-
    ### review — findings
    - evidence: 2 findings — crates/swissarmyhammer-validators/src/review/probes.rs:294, crates/swissarmyhammer-validators/src/review/probes.rs:302
    - scope: review sha HEAD~1..HEAD (checkpoint 0521177dd)
    - next: apply `impl IntoIterator` signature changes to `FileChange::new` and `FileChange::with_sources`, update call sites, re-run /review
  timestamp: 2026-08-04T12:13:56.486551+00:00
- actor: claude-code
  id: 01kz6b46g1a7njrqgxp26cz9h1
  text: |-
    ### finish iteration 2 — findings
    - implement: changed — added CALLGRAPH_MAX_DEPTH named constant with doc comment, replacing hardcoded max_depth: 1 in run_callers
    - test: green — cargo nextest run -E 'rdeps(swissarmyhammer-validators)', 3079 passed, 0 failed, 0 skipped
    - commit: 0521177dd
    - review: findings — crates/swissarmyhammer-validators/src/review/probes.rs:294 (FileChange::new takes Vec<ChangeEntry> instead of impl IntoIterator), :302 (FileChange::with_sources takes BTreeMap<String,String> instead of impl IntoIterator)
  timestamp: 2026-08-04T12:14:11.969333+00:00
- actor: claude-code
  id: 01kz6bbvhn69qyeztvpbycv9nm
  text: |-
    ### implement — changed
    - Changed `FileChange::new` (crates/swissarmyhammer-validators/src/review/probes.rs) from `pub fn new(entities: Vec<ChangeEntry>) -> Self` to `pub fn new(entities: impl IntoIterator<Item = ChangeEntry>) -> Self`, body now `entities.into_iter().collect()`.
    - Changed `FileChange::with_sources` from `pub fn with_sources(mut self, sources: BTreeMap<String, String>) -> Self` to `pub fn with_sources(mut self, sources: impl IntoIterator<Item = (String, String)>) -> Self`, body now `sources.into_iter().collect()`.
    - Grepped the full workspace for `FileChange::new`/`with_sources`/`ProbeChange::new` (the `ProbeChange` alias used in scope.rs) — only call sites are the tests in probes.rs (passing `vec![...]` literals) and `scope.rs:1681` (`ProbeChange::new(change_entities.to_vec()).with_sources(sources)` where `sources: BTreeMap<String, String>`). All keep compiling unchanged since `Vec` and `BTreeMap` both implement `IntoIterator`.
    - Evidence: `cargo check -p swissarmyhammer-validators --all-targets` exit 0; `cargo fmt --all` exit 0; `cargo clippy --workspace --all-targets -- -D warnings` exit 0 (no findings); `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` — 3079 tests run: 3079 passed, 0 skipped.
    next: /review
  timestamp: 2026-08-04T12:18:22.901728+00:00
- actor: claude-code
  id: 01kz6bgf32mrp4rgcafy3vhysy
  text: |-
    ### finish iteration 3 — changed
    - implement: changed — FileChange::new/with_sources changed from concrete Vec/BTreeMap to impl IntoIterator
    - test: green — cargo nextest run -E 'rdeps(swissarmyhammer-validators)', 3079 passed, 0 failed, 0 skipped
    - commit: 2326dd683
    - review: pending
  timestamp: 2026-08-04T12:20:53.986250+00:00
- actor: claude-code
  id: 01kz6bnjc5g637x7xfpf9jxx46
  text: |-
    ### review — clean
    - evidence: review sha HEAD~1..HEAD (commit 2326dd683) — 0 findings, 3 refuted, all prior checklist items already checked
    - next: task moved to done
  timestamp: 2026-08-04T12:23:41.189263+00:00
- actor: claude-code
  id: 01kz6bnzazdrks5nb48yr3d6g2
  text: |-
    ### finish iteration 3 — clean
    - implement: changed — FileChange::new/with_sources changed from concrete Vec/BTreeMap to impl IntoIterator
    - test: green — cargo nextest run -E 'rdeps(swissarmyhammer-validators)', 3079 passed, 0 failed, 0 skipped
    - commit: 2326dd683
    - review: clean — task moved to done
  timestamp: 2026-08-04T12:23:54.463293+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffa280
title: 'probes.rs: render_probe_evidence exceeds complexity gate; run_probes takes &[String] instead of &[&str]'
---
Found by `mcp__sah__review` while working task ^401xdvp (unrelated to that task's diff — flagged on pre-existing code in a file the task happened to touch elsewhere).

# Findings

1. `crates/swissarmyhammer-validators/src/review/probes.rs` — `render_probe_evidence` (near line 171). Cognitive complexity 24 exceeds the threshold of 15. Nested control flow (multiple loops, conditionals, branches) is hard to follow and maintain. Extract the inner loop that renders rows into a separate helper function. Suggested structure: early return for empty results, then iterate results and delegate row rendering to `render_result_rows(out, result)`.
2. `crates/swissarmyhammer-validators/src/review/probes.rs` — `run_probes` (near line 377). Parameter `probe_names: &[String]` should be `&[&str]` so callers can pass `&["foo", "bar"]` instead of requiring `&["foo".to_string(), "bar".to_string()]`. Update the function body to use `.as_str()` or forward the borrowed references. Check all call sites when changing this signature.

# Acceptance

- `render_probe_evidence` cognitive complexity below `COGNITIVE_COMPLEXITY_THRESHOLD` (15).
- `run_probes` takes `&[&str]`, all call sites updated.
- `cargo nextest run -p swissarmyhammer-validators` passes.
- `cargo fmt --all`; `cargo clippy --workspace --all-targets -- -D warnings` clean. #bug #review

## Review Findings (2026-08-04 06:57)

- [x] `crates/swissarmyhammer-validators/src/review/probes.rs:568` — Hardcoded limit of 1 for callgraph depth should be a named constant to explain the design choice of limiting inbound callers to one level of call graph depth. Define a module-level named constant like `const CALLGRAPH_MAX_DEPTH: usize = 1;` and use it at line 568, documenting why one level is the appropriate depth.

## Review Findings (2026-08-04 07:10)

- [x] `crates/swissarmyhammer-validators/src/review/probes.rs:294` — Function accepts concrete `Vec<ChangeEntry>` instead of generic `impl IntoIterator`; callers with slices or iterators must materialize to Vec first, unnecessarily limiting API flexibility. Change to `pub fn new(entities: impl IntoIterator<Item = ChangeEntry>) -> Self` with `Self { entities: entities.into_iter().collect(), sources: BTreeMap::new() }` — callers can then pass `vec![...]`, `&[...]`, or any iterator without conversion.
- [x] `crates/swissarmyhammer-validators/src/review/probes.rs:302` — Function accepts concrete `BTreeMap<String, String>` instead of generic `impl IntoIterator<Item = (String, String)>`; callers without a map must construct one explicitly. Change to `pub fn with_sources(mut self, sources: impl IntoIterator<Item = (String, String)>) -> Self` with `self.sources = sources.into_iter().collect();` — callers can pass a BTreeMap, array of pairs, iterator, etc.
