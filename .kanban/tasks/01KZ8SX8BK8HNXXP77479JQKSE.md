---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kz8t67vyvjhrzcd21782k77k
  text: |-
    Research done. Discoveries:
    - The op-dispatch pattern lives in `crates/swissarmyhammer-tools/src/mcp/tools/review/mod.rs`. Each op is a unit struct that implements `Operation` (verb + noun + params), a `Lazy` static, an entry in `REVIEW_OPERATIONS`, and one match arm in `execute`.
    - The loader-read op logic lives in `validators.rs`. It already has `engine_matched_names` (the engine matcher: `MatchContext::new().with_file(...)` + `matching_rulesets`). The new op will reuse the same matcher, so the result cannot differ from a review run.
    - Forgiving list-shape parsing has a precedent: `ref_list` in `crates/swissarmyhammer-kanban/src/dispatch.rs` (array of strings, stringified array, single string). The new `paths` parser follows that shape.
    - Unique file names: `swissarmyhammer_common::generate_monotonic_ulid_string` is available to the tools crate.
    - The test `review_description_names_every_required_param` forces `description.md` to name the new required `paths` param in backticks.
    - The registration test and the command-tree test in `tests.rs` list the ops; the command-tree test derives from `REVIEW_OPERATIONS` mechanically.
    - `.skills/` is not tracked in git; only `builtin/skills/implement/SKILL.md` needs the edit.
    - The guard test asserts against the rendered skill body, and also asserts `get validator` does not appear.
    Plan: TDD — update the guard test first (RED), then SKILL.md (GREEN); add the five real-path tests to `tests.rs` plus the registration-list entry (RED), then implement the op in `validators.rs` + `mod.rs` (GREEN); then `description.md`, fmt, clippy, nextest, self-review.
  timestamp: 2026-08-05T11:15:56.414149+00:00
- actor: claude-code
  id: 01kz8ttkga722vzrvr87zd3cr8
  text: |-
    Implementation landed (TDD, RED then GREEN at each step):
    - `crates/swissarmyhammer-tools/src/mcp/tools/review/validators.rs`: new `dump_validators` op logic — forgiving `paths` parse (`path_list`, same shape tolerance as kanban's `ref_list`), engine matcher per path, dedup across paths, `render_rules_markdown`, file in `std::env::temp_dir()` named `sah-rules-<ulid>.md`, `DumpValidatorsResponse { path, validators, rule_count, matched, extensions }`.
    - `mod.rs`: `DumpValidators` operation (verb `dump`, noun `validators`, required `paths` Array param), roster entry, dispatch arm, loader-read op doc mentions updated.
    - `description.md`: documents `dump validators` as the read-the-rules-before-you-edit op with the per-extension model; `list validators` pointed back to summary/introspection.
    - `builtin/skills/implement/SKILL.md`: "Know the rules" step now prescribes one `dump validators` call with one example file per extension, read the file whole one time, re-fetch only for a new extension.
    - Guard test updated to the new prescription; watched it fail RED before the skill edit.
    - Five real-path tests added to `review/tests.rs`; watched all five fail RED ("unknown operation") before implementation.
    Discoveries worth keeping:
    - The compiled-in BUILTIN validator layer matches `.rs`/`.ts` fixtures too, so tests must assert fixture-relative invariants, never exact set equality.
    - A `rule_count` cross-check against `###` headings in the file fails: rule bodies carry their own `###` headings. The dedup proof is instead: one path vs two same-extension paths yield identical `validators` and `rule_count`.
    Verification: `cargo nextest run -p swissarmyhammer-tools` 1514/1514 pass; `-p swissarmyhammer-skills` 128/128 pass; `cargo fmt --all` clean; `cargo clippy -p swissarmyhammer-tools -p swissarmyhammer-skills --all-targets` zero warnings. Self-review next.
  timestamp: 2026-08-05T11:27:03.690658+00:00
- actor: claude-code
  id: 01kz8zkv5kjwk42t2db4wwna5w
  text: |-
    Self-review and double-check complete.
    - Self-review round 1: 7 findings. Fixed 5 in my change set: extracted `required_string_arg` in `mod.rs` (dedups the required-param extraction across `review file`/`review sha`/`get validator`); added typed `ValidatorOpError` enum (thiserror) for all four validator ops, which also removes the duplicated `load_rules` error mapping (the message lives once in the `Load` variant). 2 findings targeted pre-existing tests outside my diff (literals 400 and 9999) — released by the review skill's written rule that drops findings asking to refactor tests that already existed.
    - Self-review round 2: 3 findings, all fixed: `validator_op_error` helper maps caller mistakes to `invalid_params` and server failures (`Load`, `WriteRulesFile`) to `internal_error`, applied to every validator-op arm; `Clone` derived on every response struct in `validators.rs`.
    - Self-review round 3: 1 finding on a pre-existing test (line 1318, literal 400) outside my diff — released by the same written rule. Clean otherwise.
    - /double-check verdict REVISE with 3 items, all implemented: (1) blank/whitespace `paths` entries are now dropped and a blank-only input is rejected as empty (RED test first, then fix); (2) the forgiving list parse moved to `op_tool_helpers::forgiving_string_list` (the declared home for shared primitives) with its own unit test — no private copy remains in `validators.rs`; (3) the advertised stringified-JSON-array shape is now covered by `dump_validators_accepts_a_single_string_path`.
    - Note: `builtin/agents/implementer/AGENT.md` shows a working-tree change from a parallel session (removes the findings-are-requirements partial include). Not mine; left untouched.
    Final gate: `cargo nextest run -p swissarmyhammer-tools -p swissarmyhammer-skills` — 1643/1643 pass, 0 skipped; clippy zero warnings; `cargo fmt --all` applied.

    ### implement — changed
    - evidence: 7 files — builtin/skills/implement/SKILL.md, crates/swissarmyhammer-skills/tests/implement_rules_and_self_review_guidance.rs, crates/swissarmyhammer-tools/src/mcp/op_tool_helpers.rs, crates/swissarmyhammer-tools/src/mcp/tools/review/description.md, crates/swissarmyhammer-tools/src/mcp/tools/review/mod.rs, crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs, crates/swissarmyhammer-tools/src/mcp/tools/review/validators.rs
    - next: formal /review
  timestamp: 2026-08-05T12:50:45.043992+00:00
position_column: doing
position_ordinal: '8380'
title: Add `dump validators` op to the review tool
---
# Goal

One tool call gives an implementer agent all the review rules for the code it will edit. The op writes one markdown file and returns the path. The agent reads that file one time. This replaces the one-call-per-file `list validators` loop in the implement skill. Keep `list validators` unchanged.

The unit of work is the **file extension**, not the file. Rules match by file pattern, so one example file per extension produces the same rule set as every file with that extension.

# New op: `dump validators`

Add the op to the `review` MCP tool (`crates/swissarmyhammer-tools/src/mcp/tools/review/`).

Input:
- `paths` — one file path or an array of file paths. Accept forgiving shapes (single string, JSON array, stringified array), the same as other sah ops. The expected use is one example file per distinct extension.

Behavior:
1. Load the RuleSet stack one time with `swissarmyhammer_validators::load_rules`.
2. Match each path with the engine matcher (`MatchContext::new().with_file(...)` + `ValidatorLoader::matching_rulesets`) — the same matcher `scope_review` and `list validators` use. The result cannot differ from what a review run enforces.
3. Deduplicate the validator set across all input paths.
4. Write one markdown file. For each validator: a heading with its name, description, and source layer; then each rule name and its body verbatim.
5. Write the file to the system temp directory (`std::env::temp_dir()`) with a unique name, for example `sah-rules-<ulid>.md`. Do not write to the CWD — bundled GUI apps start with CWD=/ which is read-only.
6. Return JSON: `path` (the markdown file), `validators` (names), `rule_count`, a `matched` map from each input path to its validator names, and `extensions` — the distinct extensions of the input paths.

Edge cases:
- No validator matches any path: still write the file (state that no rules apply) and return it with an empty `validators` list.
- An empty `paths` input is an error.

# Documentation updates

- `crates/swissarmyhammer-tools/src/mcp/tools/review/description.md`: document `dump validators` as the read-the-rules-before-you-edit op. State the per-extension model: pass one example file per extension. Point `list validators` back to summary/introspection use.
- `builtin/skills/implement/SKILL.md`, step "Know the rules": replace the one-call-per-file `list validators` instruction. New instruction:
  1. Collect the distinct extensions of the files you plan to edit. Pick one example file for each extension.
  2. Call `dump validators` one time with those example paths.
  3. Read the returned file whole, one time.
  4. Do not call again for more files with the same extensions. Call again only when a later edit targets a file with a new extension.
  Keep the "before you edit", "verbatim rules", and "obey as you write" guidance.

# Test updates

- `crates/swissarmyhammer-skills/tests/implement_rules_and_self_review_guidance.rs`: the `RULES_CALL` marker and the "One call per file" marker must change to assert the new prescription: one `dump validators` call with one example file per extension, read the returned file, before the Implement step, re-fetch only for a new extension. Keep the order assertions and the obey-item markers.
- `crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs`: real-path tests for the new op:
  - Call the op with two example files of different extensions that match different validators. Read the produced markdown. Assert it contains each applicable rule body one time (deduplicated).
  - Assert the returned `matched` map pairs each file with the correct validator names, and `extensions` lists the distinct extensions.
  - A path that matches no validator returns an empty list and a valid file.
  - Forgiving input: a single string path works the same as a one-element array.
  - Empty `paths` returns an error.