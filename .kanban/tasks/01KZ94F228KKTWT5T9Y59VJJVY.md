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
- actor: claude-code
  id: 01kzdc68sz749a77y51e86srqg
  text: 'Check-in from the watching session (card author, 2026-08-07): all three acceptance criteria verified — catalog rows built from the trait impl (single source of truth), one-parse-per-file proven by the mutation-checked PARSE_EVENT ledger (rebuilding that test after catching the worthless first version was exactly right), and the shared not_computed_row owner. One card correction, in your favor: the card said to reuse the code-context chunker''s grammars, but from swissarmyhammer-validators that crate IS a second roster — your choice of the sem code plugin''s table (already used by is_code_file and cognitive_complexity) is the correct reading of the "no second roster" requirement. Reviewers: do not flag the crate substitution. The trait shape you published is what the four blocked probe cards (^58n25xs, ^w0efc04, ^72xah2v, ^cysg4xv) should build on verbatim.'
  timestamp: 2026-08-07T05:47:29.471231+00:00
- actor: claude-code
  id: 01kzdcs59yssfyp591ybd840gj
  text: |
    ### review — findings
    - scope: review sha HEAD~1..HEAD (commit 9a0598fc5)
    - evidence: 15 findings, 15 confirmed, 1 refuted, 27 attempted, 0 failed — languages.rs:5, :5, :6, :7, :8, :9, :10, :11, :98, :364, :364; code/mod.rs:158; probes.rs:1386; tree_sitter_probes.rs:124, :432
    - next: fix all 15 items in the 2026-08-07 00:33 Review Findings section, then re-run /review
  timestamp: 2026-08-07T05:57:48.478994+00:00
- actor: claude-code
  id: 01kzdctbq2jqss73dsbd4qvdy2
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 7 files; new crates/swissarmyhammer-validators/src/review/tree_sitter_probes.rs with the TreeSitterProbe trait, one shared parse_code entry point in swissarmyhammer-sem
    - test: green — cargo nextest run --workspace 13685 passed, doc tests 0 failed, fmt clean, clippy clean
    - commit: 9a0598fc5
    - review: findings — 15 open. crates/swissarmyhammer-sem/src/parser/plugins/code/languages.rs (11: missing docs on LanguageConfig and its fields, TSX_CONFIG duplicates TYPESCRIPT_CONFIG), .../code/mod.rs:158, crates/swissarmyhammer-validators/src/review/probes.rs:1386, .../review/tree_sitter_probes.rs:124, :432
  timestamp: 2026-08-07T05:58:27.810323+00:00
- actor: claude-code
  id: 01kzddhaq83908a1sjwrpytv4v
  text: |-
    All 15 findings from the 2026-08-07 00:33 section are fixed. Notes the next agent needs:

    **Sealing does NOT block the four dependent cards — I checked before I sealed.** The finding asked for a private `Sealed` bound. The four cards (^58n25xs, ^w0efc04, ^72xah2v, ^cysg4xv) each add a probe that must appear in `TREE_SITTER_PROBES`, and that roster lives in `tree_sitter_probes.rs`. A private `mod sealed` in that file is visible to the file AND to every submodule of it, so a probe written in `tree_sitter_probes.rs` or in a future `tree_sitter_probes/<probe>.rs` can implement `Sealed`. Nothing outside the module implements `TreeSitterProbe` today (grep of the whole workspace shows only the trait's own module). The one rule for a new probe: write it in this module or a submodule of it, and add `impl sealed::Sealed for YourProbe {}` beside the `impl TreeSitterProbe`. The trait doc says exactly that. No conflict, no blocker.

    I proved the seal bites rather than assuming it: deleting `impl sealed::Sealed for FunctionDeltaProbe {}` fails to compile with `error[E0277]: the trait bound FunctionDeltaProbe: sealed::Sealed is not satisfied`.

    **Both case-insensitivity tests were verified RED by mutation, not just written.**
    - `parse_code_routes_an_uppercase_extension_to_the_same_grammar` — dropping `.to_lowercase()` from `dotted_lowercase_extension` makes it panic on `"rust is in the roster"`.
    - `similar_binds_to_a_function_entity_type_spelled_in_upper_case` — replacing `to_ascii_lowercase()` with `to_string()` in `is_function_entity_type` makes it panic on `"a similar result bound to the added body"`, because a `METHOD` entry never binds and no result exists at all.

    **The `similar` tests now share one setup helper.** Adding a second near-copy of `similar_returns_an_existing_util_and_excludes_self` would have been a duplication finding, so the setup moved into `similar_for_entity_type(entity_type)` and both tests call it. That removed dead setup the old test carried: it seeded two chunks with `dup_emb()`, then immediately ran `DELETE FROM ts_chunks` and re-seeded the same two chunks with the query vector. The helper seeds the query vector once.

    **One correction to the finding text on `languages.rs`.** The finding for line 9 describes `container_node_types` and the one for line 10 describes `call_entity_identifiers`, but the field ORDER in the struct is `entity_node_types`, `container_node_types`, `call_entity_identifiers`. I documented each field by its own meaning, not by the line number the finding cited. `call_entity_identifiers` is the Elixir-style "a definition is spelled as a call" list; `container_node_types` is the class-body/declaration-list list.

    **TSX duplication.** `typescript_family_config` is a `const fn`, so both statics keep their `static LanguageConfig` type and no lazy init appears. All 48 TypeScript/TSX tests still pass, so the two configs are byte-identical to what they were.

    Verification: `cargo nextest run -p swissarmyhammer-sem -p swissarmyhammer-validators` — 784 passed, 0 failed, 0 skipped (was 782; the two new tests are the difference). `cargo fmt --all --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean. `cargo doc --no-deps` on both crates: 8 + 39 warnings, none in a file this change touched.
  timestamp: 2026-08-07T06:11:00.456723+00:00
- actor: claude-code
  id: 01kzddhh0r03hkcp2qpvqaq1dj
  text: |-
    ### implement — changed
    - evidence: 4 files — crates/swissarmyhammer-sem/src/parser/plugins/code/languages.rs, .../code/mod.rs, crates/swissarmyhammer-validators/src/review/probes.rs, .../review/tree_sitter_probes.rs. All 15 findings in the 2026-08-07 00:33 section checked off. Tests: cargo nextest run -p swissarmyhammer-sem -p swissarmyhammer-validators — 784 passed, 0 failed, 0 skipped. fmt clean, workspace clippy -D warnings clean.
    - next: /review
  timestamp: 2026-08-07T06:11:06.904533+00:00
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

## Review Findings (2026-08-07 00:33)

- [x] `crates/swissarmyhammer-sem/src/parser/plugins/code/languages.rs:5` — Public struct `LanguageConfig` lacks documentation. This is a core public data structure that defines language configuration and should have a doc comment explaining its purpose. Add a doc comment above the struct explaining what LanguageConfig represents and how it is used.
- [x] `crates/swissarmyhammer-sem/src/parser/plugins/code/languages.rs:5` — Public struct `LanguageConfig` and all its public fields lack doc comments. All public items must have documentation explaining their purpose and usage. Add doc comments to the struct and each field. For example: `/// Configuration for a programming language's tree-sitter grammar and parsing behavior.` before the struct, and `/// The language identifier, mirroring the extension-based lookup key.` before each field.
- [x] `crates/swissarmyhammer-sem/src/parser/plugins/code/languages.rs:6` — Public struct field `id` lacks a doc comment. All public items, including struct fields, must be documented. Add a doc comment such as: `/// The language identifier, used as the lookup key.`.
- [x] `crates/swissarmyhammer-sem/src/parser/plugins/code/languages.rs:7` — Public struct field `extensions` lacks a doc comment. All public items, including struct fields, must be documented. Add a doc comment such as: `/// File extensions this language handles (e.g., `".rs"`).`.
- [x] `crates/swissarmyhammer-sem/src/parser/plugins/code/languages.rs:8` — Public struct field `entity_node_types` lacks a doc comment. All public items, including struct fields, must be documented. Add a doc comment such as: `/// Tree-sitter node kinds representing top-level entities (functions, classes, etc.).`.
- [x] `crates/swissarmyhammer-sem/src/parser/plugins/code/languages.rs:9` — Public struct field `container_node_types` lacks a doc comment. All public items, including struct fields, must be documented. Add a doc comment such as: `/// Tree-sitter node kinds that contain or group entities (class_body, etc.).`.
- [x] `crates/swissarmyhammer-sem/src/parser/plugins/code/languages.rs:10` — Public struct field `call_entity_identifiers` lacks a doc comment. All public items, including struct fields, must be documented. Add a doc comment such as: `/// Call target identifiers for special forms (e.g., Elixir's defmodule, def).`.
- [x] `crates/swissarmyhammer-sem/src/parser/plugins/code/languages.rs:11` — Public struct field `get_language` lacks a doc comment. All public items, including struct fields, must be documented. Add a doc comment such as: `/// Function pointer to retrieve the tree-sitter Language for this grammar.`.
- [x] `crates/swissarmyhammer-sem/src/parser/plugins/code/languages.rs:98` — TSX_CONFIG is a near-verbatim copy of TYPESCRIPT_CONFIG, differing only by three parameter values (id, extensions, and get_language). Two blocks differing only by values should be extracted into a shared parameterized function to prevent drift. Extract a helper function (e.g., `const fn typescript_family_config(id: &'static str, extensions: &'static [&'static str], get_language: fn() -> Option<Language>) -> LanguageConfig`) that encapsulates the shared entity_node_types, container_node_types, and call_entity_identifiers. Initialize both TYPESCRIPT_CONFIG and TSX_CONFIG by calling this function with their differing parameters.
- [x] `crates/swissarmyhammer-sem/src/parser/plugins/code/languages.rs:364` — Public function `get_language_config` lacks documentation. This is a core API function for looking up language configuration by file extension and needs a doc comment. Add a doc comment explaining that this function retrieves the LanguageConfig for a given file extension, returns None if not found, and describes the parameters and return type.
- [x] `crates/swissarmyhammer-sem/src/parser/plugins/code/languages.rs:364` — Public function `get_language_config` lacks a doc comment. All public functions must be documented. Add a doc comment such as: `/// Look up the language configuration for a file extension. Returns None if the extension is not recognized.`.
- [x] `crates/swissarmyhammer-sem/src/parser/plugins/code/mod.rs:158` — The new `parse_code` function (lines 97–151) normalizes file extensions to lowercase via `dotted_lowercase_extension(path)?` at line 122 before looking them up. However, all three new tests for `parse_code` (lines 158–188) use only lowercase extensions ('.rs', '.txt'), so case-insensitivity of extension matching is not verified. Add one test case with uppercase extension like `parse_code("src/lib.RS", ...)` or `parse_code("test.PY", ...)` to verify that case-insensitive extension matching works correctly.
- [x] `crates/swissarmyhammer-validators/src/review/probes.rs:1386` — The new `is_function_entity_type` function (lines 398–401) performs case-insensitive matching via `to_ascii_lowercase()` before checking if the lowercased entity type contains 'function' or 'method'. The test for `run_similar` — the only code path exercising this function — only provides lowercase entity types (`"function"` at line 1418), so case-insensitivity is not verified. Add one test case for `run_similar` with mixed-case entity type like `"Function"` or `"METHOD"` to confirm case-insensitive matching works as intended.
- [x] `crates/swissarmyhammer-validators/src/review/tree_sitter_probes.rs:124` — Public trait `TreeSitterProbe` is not meant to be implemented downstream — all implementations are in this module's internal registry, and validators reference probes by name from the catalog, not by implementing the trait. Without sealing, downstream implementations would be useless yet possible, creating semver hazards if methods are added later. Seal the trait using a private `Sealed` trait: add a private module with `pub trait Sealed {}`, require it as a bound on `TreeSitterProbe`, and implement it only for types in this module. This makes non-implementation intent explicit and prevents downstream breakage if methods are added.
- [x] `crates/swissarmyhammer-validators/src/review/tree_sitter_probes.rs:432` — Hardcoded numeric literal `2` should be a named constant. It represents the expected number of revisions (before and after) that should be parsed for a single file under review. Define a constant such as `const EXPECTED_REVISIONS_PER_FILE: usize = 2;` at the top of the test module and replace the hardcoded `2` with this named constant for clarity.
