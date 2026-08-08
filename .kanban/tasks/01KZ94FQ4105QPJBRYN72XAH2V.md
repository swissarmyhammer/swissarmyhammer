---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzdezd23zka8n1p358ba4hgb
  text: |-
    Picked up. Research done.

    Findings:
    - `TreeSitterProbe` lives in `crates/swissarmyhammer-validators/src/review/tree_sitter_probes.rs`. A new probe implements `sealed::Sealed` beside the trait and adds one entry to `TREE_SITTER_PROBES`. The catalog builds its row from `name()`/`kind()`, so no second registration.
    - `TreeSitterProbeContext` gives `path()`, `after()` and `before() -> Option<ParsedRevision>`. `ParsedRevision::parsed().entities(path, source)` returns `SemanticEntity` rows carrying `id`, `name`, `entity_type`, `content_hash`, `start_line`. `id` is the stable identity (`file::type::name` or `file::parent::name`), so before/after comparison keys on `id` and compares `content_hash`.
    - `is_function_entity_type` (probes.rs) is the shared predicate for function/method entity types.
    - `scope_review(Scope::Working, ...)` fills `FileChange::before_sources` from git HEAD, so a `TestRepo` commit + working-tree edit is a real diff for the probe. `FileWork::probe_results()` exposes the rows, so a real-pipeline test can assert on rows and not only on rendered text.
    - Pairing must NOT be scoped by `parent_id`: serde puts `serialize` in `impl Serialize for T` and `deserialize` in `impl Deserialize for T`, which are different parents. Module scope = the file.

    Design:
    - Pair table is one data list of word pairs (serialize/deserialize, encode/decode, to/from, read/write, open/close, save/load, lock/unlock, push/pop).
    - Names split into lower-case words (snake_case, kebab, camelCase). Two functions pair when their word vectors have the same length and differ at exactly one position that holds the two sides of one table row. That covers `serialize`/`deserialize`, `write_all`/`read_all` and `to_json`/`from_json` with one comparison and no name reconstruction.
    - `before() == None` (added file, or a scope with no base revision) emits one not-computed row, the same contract `TREE_SITTER_NOT_PARSED` holds, so an undiffable file never reads as "no broken pairs".
    - Kind is `Candidate`: naming convention is a heuristic the agent judges, not a guard-able fact.
  timestamp: 2026-08-07T06:36:10.179082+00:00
- actor: claude-code
  id: 01kzdfgj0t6689cgk04dx4evam
  text: |-
    Implementation landed.

    The probe: `inverse-pairs`, a `TreeSitterProbe` in `crates/swissarmyhammer-validators/src/review/tree_sitter_probes.rs`, kind `Candidate`, one entry in `TREE_SITTER_PROBES`.

    - `INVERSE_PAIRS` is one data list of eight `InversePair` rows (serialize/deserialize, encode/decode, to/from, write/read, open/close, save/load, lock/unlock, push/pop). Adding a convention is adding a row.
    - `name_words` splits an identifier into lower-case words (snake, kebab, camel, acronym runs). `pairing_convention` pairs two names when their word lists are the same length and differ at exactly one position holding both sides of one row. That one comparison covers `serialize`/`deserialize`, `write_all`/`read_all` and `to_json`/`from_json`, with no name reconstruction and no casing rules.
    - `pair_candidates` marks a definition edited when the base revision holds no entity under the same `SemanticEntity::id` or holds one with a different `content_hash`. Keying on the id keeps same-named methods on different types apart; hashing content keeps a definition that only moved down the file reading as untouched.
    - `before() == None` emits the new `INVERSE_PAIRS_NOT_DIFFED` row rather than an empty result, so an added file or a base-less scope never reads as "no pair was broken here". Re-exported from `review::mod` beside `TREE_SITTER_NOT_PARSED`.

    Tests, all RED first:
    - `a_change_to_one_side_of_an_inverse_pair_names_the_untouched_partner` and `a_change_to_both_sides_of_an_inverse_pair_reports_no_rows` are the two acceptance criteria, driven through a real `TestRepo` commit + working-tree edit and the real `scope_review`, reading rows off `FileWork::probe_results()`. Not a hand-built change set.
    - `a_conversion_pair_is_found_on_one_word_of_a_multi_word_name` proves the table pairs words, not whole names.
    - `a_file_with_no_base_revision_reports_one_not_computed_row`.
    - `test_completeness_declares_the_inverse_pairs_probe_its_rule_reads` (builtin/mod.rs) covers the wiring: the declaration, the catalog, and the rule that reads the rows. RED verified by removing the `probes:` block and re-running.

    Two shared test helpers, `reviewed_work` and `changed_file_work`, were extracted so the new scoped-review tests do not copy the existing prompt test's repo-and-scope block.

    Rule change: `inverse-operation-coverage` gained a "What the probe gives you" section — the row shape, the row as candidate not verdict, an explicit "an empty row list is not a clean bill" (the probe sees only same-file function definitions), and the not-computed row. Added an exception for a row whose two names match the spelling but not the meaning. The `completeness` VALIDATOR.md gained `probes: [inverse-pairs]` and its body no longer claims every rule needs no engine probe.

    Verification: `cargo test -p swissarmyhammer-validators` 478 passed / 0 failed; `cargo clippy --workspace --all-targets` zero warnings; `check validators` ok, 0 errors.
  timestamp: 2026-08-07T06:45:32.314156+00:00
- actor: claude-code
  id: 01kzdfgqmwqbsp6m9xbc3pg229
  text: |-
    ### implement — changed
    - evidence: 5 files — crates/swissarmyhammer-validators/src/review/tree_sitter_probes.rs, crates/swissarmyhammer-validators/src/review/mod.rs, crates/swissarmyhammer-validators/src/builtin/mod.rs, builtin/validators/completeness/VALIDATOR.md, builtin/validators/completeness/rules/inverse-operation-coverage.md. `cargo test -p swissarmyhammer-validators`: 478 passed, 0 failed. `cargo clippy --workspace --all-targets`: 0 warnings.
    - next: /review
  timestamp: 2026-08-07T06:45:38.076288+00:00
- actor: claude-code
  id: 01kzdgdmfqnda35j9vg2kkqn8c
  text: |
    ### review — clean
    - evidence: review sha HEAD~1..HEAD (ede9b46ca) — 0 findings, 0 confirmed, 0 refuted; 9 pairs attempted, 0 failed, 0 skipped
    - next: task moved to done
  timestamp: 2026-08-07T07:01:25.111956+00:00
- actor: claude-code
  id: 01kzdge7f28v3bsxrmnj1cxg5p
  text: |-
    ### finish iteration 1 — clean
    - implement: changed — 5 files; the inverse-pairs probe with an 8-row data pair table, wired into the completeness set and its inverse-operation-coverage rule
    - test: green — cargo nextest run --workspace 13692 passed, doc tests 0 failed, fmt clean, clippy clean
    - commit: ede9b46ca
    - review: clean — 0 findings, 9 pairs attempted; task moved to done
  timestamp: 2026-08-07T07:01:44.546496+00:00
depends_on:
- 01KZ94F228KKTWT5T9Y59VJJVY
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffba80
title: Inverse-pair census probe for completeness
---
A TreeSitterProbe that reports when a diff touches one side of a paired operation and not the other.

- Extract symbol names per module from the parse.
- Pair by naming convention: serialize/deserialize, encode/decode, to_x/from_x, read/write, open/close, save/load, lock/unlock, push/pop. Keep the pair table as data, one list, easy to extend.
- One ProbeRow per broken pair: the touched symbol, its untouched partner, and the convention that paired them.

Wire-up:
- Add the probe to the `completeness` set's `probes:` list.
- Update the inverse-operation-coverage rule to consume the rows: the pairs are found for you; judge whether the partner needed the change.

Acceptance:
- A diff that edits `serialize` and not `deserialize` in the same module yields one row.
- A diff that edits both yields no rows.

#tool-validators