---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzd9hv2cmmzdw098xxb4mx61
  text: |-
    Research done. Picked up the card, moved to `doing`.

    Discoveries that set the design:

    1. **The grammar roster to reuse is `swissarmyhammer-sem::parser::plugins::code`.** The validators crate already reuses it twice: `is_code_file` (review scope) and `cognitive_complexity` (the `complexity` probe). Its `languages.rs` holds the one extension → grammar table (`get_language_config`, `dotted_lowercase_extension`) and a thread-local parser cache. `swissarmyhammer-validators` does NOT depend on `swissarmyhammer-treesitter`, so pulling that crate in would add the second roster the card forbids. The card's acceptance criterion "same as the complexity probe does today" also points at this roster, because the complexity probe's "could not compute" row comes from exactly this lookup returning `None`.

    2. **`languages` is a private module.** Only `get_all_code_extensions` and `is_code_file` are re-exported. So the reuse needs one new public function in the sem code plugin: parse a file with the shared roster and hand back the tree. `CodeParserPlugin::extract_entities` and `cognitive_complexity` both build their own `tree_sitter::Parser` today; both will be re-expressed on the new function so there stays exactly one parse path.

    3. **`ProbeCatalogEntry` name/kind would be a second source of truth** if a trait probe restated them beside its implementation. The `CATALOG` static becomes a `catalog()` function built once, with the trait-probe rows read off `TreeSitterProbe::name()`/`kind()`. Nothing outside `review/mod.rs` names `CATALOG`.

    4. **`FileChange` carries only after-content.** Diff-aware probes need both sides, so `FileChange` gains `before_sources`. `scope_review` already computes `before_by_path` for the blame/line-mark pass, so the before side is free — no new git read.

    5. **Test harness for "rows reach the validator prompt"** already exists: `TestRepo` + `loader_with(name, glob, probes)` + `index_conn()` + `MockEmbedder` + the real `scope_review`, then `fleet::render_file_payload`. That is the same shape `working_scope_groups_duplicate_under_validator_with_full_source` and `a_findings_line_number_survives_from_the_prime_to_the_report` use.
  timestamp: 2026-08-07T05:01:22.892996+00:00
- actor: claude-code
  id: 01kzdaqfqbkn1erkftmpkccp5k
  text: |-
    Implementation landed. The trait shape, which the four blocked cards build on:

    ```rust
    pub trait TreeSitterProbe: std::fmt::Debug + Send + Sync {
        fn name(&self) -> &'static str;
        fn kind(&self) -> ProbeKind;
        fn run(&self, context: &TreeSitterProbeContext<'_>) -> Vec<ProbeRow>;
    }

    impl<'a> TreeSitterProbeContext<'a> {
        pub fn path(&self) -> &'a str;
        pub fn after(&self) -> ParsedRevision<'a>;
        pub fn before(&self) -> Option<ParsedRevision<'a>>;
    }

    impl<'a> ParsedRevision<'a> {
        pub fn source(&self) -> &'a str;
        pub fn parsed(&self) -> &'a ParsedCode;   // tree + entities, no second parse
    }
    ```

    Registration is one line: add the implementation to `TREE_SITTER_PROBES`. The catalog row is BUILT from the implementation, so a probe's name and kind live in exactly one place.

    What did NOT work, so the next agent does not repeat it:

    - **The first parse-sharing test was worthless.** It asserted `logs_contain("parses=2")` against a per-cache summary log. A deliberate mutation (build one cache per probe) still logged `parses=2` three times, so the test passed while the bug was live. The measurement had to become a per-parse ledger event (`PARSE_EVENT`, one line per real call into the grammar roster) counted with `logs_assert`. Re-run of the same mutation then reported 6 parses and the test failed as it should. Any future change to that instrumentation must be re-checked the same way: break the sharing on purpose and confirm RED.
    - **`swissarmyhammer-treesitter` was the wrong crate to reach for**, though the card names it. `swissarmyhammer-validators` does not depend on it, and its `LanguageRegistry` is a SECOND grammar roster beside the one the validators crate already uses. Reusing `swissarmyhammer-sem`'s code plugin keeps the count at one.

    Discoveries worth carrying forward:

    - `CodeParserPlugin::extract_entities` and `cognitive_complexity` each built their own `tree_sitter::Parser`. Both now go through the new `parse_code`, so there is one parse path, one grammar table, and one thread-local parser cache. `ParsedCode::entities()` runs the same extractor against a parse the caller already holds, which is how a probe lists a file's definitions without re-parsing.
    - `ProbeOp` gained `TreeSitter(&'static dyn TreeSitterProbe)`, so its derived `PartialEq`/`Eq` had to become manual impls. They compare tree-sitter ops by probe NAME, not by trait-object pointer — fat-pointer equality depends on vtable layout.
    - Two file-bound probe families now share `per_file_results` and `not_computed_row` in `probes.rs`. `run_complexity` was re-expressed on them, so the "one result per file, one not-computed row when unmeasurable" contract has one owner.
    - `scope_review` already had `before_by_path` for the blame pass; `run_probe_cache` now forwards it, so the before revision costs no extra git read.

    Verification: 782 tests in swissarmyhammer-validators + swissarmyhammer-sem, 2 doc-tests, 107 review tests in swissarmyhammer-tools, workspace clippy clean, rustdoc warning count unchanged at 49 (measured against a stashed baseline).
  timestamp: 2026-08-07T05:21:56.459158+00:00
- actor: claude-code
  id: 01kzdaqvey22y2egagk1ev3dkj
  text: |-
    ### implement — changed
    - evidence: 7 files — crates/swissarmyhammer-validators/src/review/tree_sitter_probes.rs (new), review/probes.rs, review/scope.rs, review/mod.rs, crates/swissarmyhammer-sem/src/parser/plugins/code/mod.rs, code/complexity.rs, code/languages.rs. Tests: 782 pass in swissarmyhammer-validators + swissarmyhammer-sem, 0 failed, 0 skipped; 2 doc-tests pass; workspace clippy clean.
    - next: /review
  timestamp: 2026-08-07T05:22:08.478506+00:00
position_column: doing
position_ordinal: '8480'
title: 'TreeSitterProbe trait: file + parse + logic'
---
Add a second probe family beside the `ProbeOp` catalog in `swissarmyhammer-validators/src/review/probes.rs`.

A tree-sitter probe is a trait implementation. Input: one changed file and its tree-sitter parse. Output: `ProbeRow`s.

Work:
- Define the trait: name, `ProbeKind`, and `run(file, parse, diff_context) -> Vec<ProbeRow>`.
- Diff-aware probes need the before AND after parse of a changed file. Put both in the probe context.
- Parse each file one time per review. Share the tree across every probe that runs on that file.
- Register trait probes in the existing probe catalog. Validators declare them by name in `probes:` exactly as today. `probe_exists` and `check validators` see them.
- Reuse the tree-sitter grammars and language routing that the code-context chunker already owns. Do not add a second grammar roster.
- Probe results flow to rules on the existing `ProbeResult` path. No new plumbing per probe.

Acceptance:
- A trivial trait probe (for example: function count per file) registers, runs on a review, and its rows reach the validator prompt.
- One parse per file per review, proven by test.
- A file whose language has no grammar produces one "could not compute" row, same as the complexity probe does today.

#tool-validators