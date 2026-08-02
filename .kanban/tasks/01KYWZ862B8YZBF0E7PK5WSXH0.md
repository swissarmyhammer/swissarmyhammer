---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kz15hqxtx9ht5ksga88ggzz0
  text: |-
    ## Root cause — investigation result

    Five findings. One is the cause. One is the amplifier. One is a real defect found on the way. Two are ruled out or reclassified.

    ### RC1 (cause) — the review agent samples, and the default backend has no knob to pin

    - The review default agent is `claude-code-haiku`. `REVIEW_DEFAULT_AGENT` is in `crates/swissarmyhammer-config/src/model.rs`. Its executor is `claude-code`, which starts the `claude` CLI as a subprocess.
    - The only generation parameter the review engine sends is `max_tokens`. See the `_meta` map in `crates/swissarmyhammer-validators/src/validators/pool.rs` — it holds `MAX_TOKENS_META_KEY` and nothing else. `PoolConfig` has no sampling field.
    - `claude --help` shows no `--temperature`, no `--top-p`, no `--top-k`, no `--seed`. Verified on this machine. No knob exists.
    - The local backend is the opposite. `crates/llama-agent/src/generation/mod.rs` builds `LlamaSampler::chain_simple([dist(DEFAULT_GENERATION_SEED), greedy()])`. `greedy()` is last, so the local backend is argmax. The MTP path is argmax too.

    Result: run-to-run churn is a property of the hosted backend. You cannot pin temperature or a seed for the default review agent. The gate must therefore stay stable in spite of sampling, not because sampling stopped.

    ### RC2 (amplifier, and the cause of the match-arm false positive) — the rule gives no counting procedure

    `builtin/validators/complexity/rules/cognitive-complexity.md` says "Conditions nested more than 3 levels deep (4+ is a flag)". It never says what a level is. It never says whether a `match` counts as one level or one level per arm. It gives no worked example.

    With no procedure, each sample invents one. One sample counted each arm as its own level and reported "depth 4" for a two-arm `Option` match at depth 2. The sampling variance did not have to flip the finding — the ambiguity gave it a wide target.

    ### RC3 (real defect, found on the way) — validator enumeration is in HashMap order

    `ValidatorLoader` keeps `rulesets: HashMap<String, RuleSet>`. `list_rulesets`, `list_ruleset_names`, and `matching_rulesets` return `values()`/`keys()` with no sort. Rust seeds `HashMap` at random, so the order changes per process and per map.

    The review path does not show the defect today, because `match_validators_and_files` re-keys into a `BTreeMap` and `assemble_validator_work` sorts. But `list validators` output and `swissarmyhammer_validators::match_rules` are unstable, and the review path's determinism rests on a downstream accident instead of on the source.

    ### RC4 (ruled out) — batching and prompt rendering are deterministic

    - `batch_work_list` packs `distinct_files()`, which dedups through a `BTreeSet` over name-sorted validators and path-sorted files. `DEFAULT_BATCH_SIZE` 256KiB to 384KiB moves where a split lands, but the split is a pure function of the input.
    - `batch_work_list` now returns `(Vec<WorkList>, Vec<SkippedFile>)` instead of an error. This adds no order dependence.
    - `render_file_block` and `render_numbered_source` are pure over a pre-computed `FileWork`. No I/O. No shell-out to `git blame` — blame uses libgit2 earlier, in the scope stage.
    - The production review path holds no `HashMap`, no `HashSet`, no `join_all`, no `FuturesUnordered`, no rayon, no clock read, and no random id. Every fan-out drains in submission order. Dedup keys on a `BTreeSet`. The final sort is stable on `(file, line)`.

    So none of the three changes named in the card explain the churn.

    ### RC5 (a second, different drift source — reclassified) — blame shas are now prompt bytes

    `render_numbered_source` prints `{line} | {sha} {mark} | {text}`. For `Working`, `File`, and `Glob` scopes, blame binds to HEAD, and uncommitted lines render as `worktree`. A commit, an amend, or a `git add` changes the prompt without changing one source byte. Only `Scope::Sha` pins blame.

    This does not explain two back-to-back runs on an unchanged worktree. It does explain finding churn between finish-loop iterations, which is what corrupts the stuck-detection guardrail. Recorded as its own card.

    ### What the fix must therefore be

    Sampling cannot be pinned. So stabilize the two layers that can be:

    1. Remove the ambiguity the sampler exploits. Give the depth rule a mechanical counting procedure and answer the match-arm question in the rule text.
    2. Make the deterministic input path deterministic at the source, not by accident, and pin it with a repeated-run harness.

    The rule is not made quieter. The counting procedure keeps every check and every threshold. It only removes the freedom to invent a different count each run.
  timestamp: 2026-08-02T12:00:32.186591+00:00
- actor: claude-code
  id: 01kz1hwacw146p7mcfrxa11vrz
  text: |-
    ## Remedy changed mid-task — the model no longer counts

    The first remedy plan (pin sampling, tighten the prose) was superseded. The real cause is narrower and fixable: **the rule asked a language model to COUNT nesting depth and branches.** Models count badly. Sampling variance only decided which wrong count came back on a given run.

    So the count moved into code. The model is now handed the number and compares it against a gate.

    ### What was built

    **1. A cognitive-complexity scorer — `crates/swissarmyhammer-sem/src/parser/plugins/code/complexity.rs` (new)**

    A pure function over the tree-sitter parse. `cognitive_complexity(path, source) -> Option<FileComplexity>`. Per function it computes the published Sonar cognitive complexity, the max condition-nesting depth, and the supporting counts (`branch_count`, `max_boolean_operands`, `max_loop_nesting`, `max_else_if_chain`), plus `is_test`.

    Node kinds are a per-language `ComplexitySpec` data row, so the counted node set is reviewable in one place. The Rust row was built by parsing samples and reading the s-expression, not by guessing.

    Two gates, both named constants: `COGNITIVE_COMPLEXITY_THRESHOLD = 15` (the Sonar default) and `NESTING_DEPTH_THRESHOLD = 4` (the depth the rule already stated). The other numbers are evidence, not gates — the rule never set a limit for them, so none was invented.

    `is_test` is read from the attribute at the **definition**, never from the file name. `#[test]` and `#[tokio::test]` mark a test; `#[serial_test::serial]` and `#[test_case(..)]` do not. A `build_request` helper in `foo_test.rs` is still scored.

    **Language coverage: Rust only.** Every other language returns `None`, which the probe reports as "not computed". Never a zero — a silent zero would disable the validator on that language. Follow-up card ^xjyb2qf.

    **2. A `complexity` probe — `crates/swissarmyhammer-validators/src/review/probes.rs`**

    A fourth catalog entry, kind `fact`. It binds to each file under review and emits **one row per function over a gate**, each row carrying every measured number with its gate beside it:

        src/walk.rs:2 `walk` — cognitive complexity 10 (gate 15), max condition-nesting
        depth 4 (gate 4), 2 branches, at most 1 boolean operands in one condition,
        loops nested 2 deep, longest else-if chain 0

    `FileChange` gained `sources` (path to current content) so a file-bound probe can measure the whole review boundary, not only the entities the diff touched. `run_probe_cache` in `scope.rs` fills it and no longer short-circuits on an empty entity list, which a file-bound probe would have skipped.

    **3. A deterministic guard — `crates/swissarmyhammer-validators/src/review/verify.rs`**

    An **empty** complexity result is now a positive fact: "every function in this file is under both gates". A new `GUARD_RULES` row refutes a `complexity` finding on that fact, with no model in the loop. This is exactly the `tag_parser.rs` case.

    A "not computed" row keeps the result non-empty, so an unmapped language is never mistaken for a simple one.

    **4. The rule became a lookup** — `builtin/validators/complexity/rules/cognitive-complexity.md` and `VALIDATOR.md`

    "Report the functions the probe listed" replaces "analyze the file for functions with high cognitive complexity". The four unquantified words the old rule used — numerous, multiple, extended — are gone, replaced by the computed numbers. Both gates are stated. The card's second acceptance question is answered in code AND in prose: **a match/switch counts once for the whole construct, and its arms open no nesting level.**

    The rule is not quieter. The nesting gate the old rule stated is unchanged at 4, and a second gate was added on the score.

    ### The false positives, pinned

    `collect_line_tags` scores **5, depth 2**. `edit_line_markers` scores **12, depth 3**. Both are under both gates and cannot be flagged again. Both are pinned with the source exactly as it stood when the review flagged them (recovered from `4a7bbf7e0^`).

    The review reported "match arms contain code at depth 4" for a construct the scorer measures at depth 2.

    ### Why the N-run LLM harness is not in CI

    It no longer needs to be. The card asked for a repeated-run harness because the finding set drifted. The drift came from the count, and the count is now a pure function of a parsed tree — so the determinism test runs with **no model at all** and belongs in CI:

    - `repeated_scoring_of_one_source_never_drifts` — 25 runs over one source, identical results.
    - `the_scorer_is_insensitive_to_the_order_functions_are_declared_in`.
    - `complexity_rows_are_identical_across_repeated_runs` — 10 runs through the real `run_probes` entry point.

    A true N-run LLM harness would still be non-deterministic, because the default review agent is `claude-code-haiku` and the `claude` CLI exposes no `--temperature`, `--top-p`, or `--seed` (verified). That is why the fix removes the model's judgment from the number instead of trying to pin the model.

    ### One bug caught by self-review

    `break 5` returns a value from a loop. The first draft counted any `break`/`continue` with a named child as a labelled jump, so a valued break scored a spurious +1. Test `a_break_carrying_a_value_is_not_a_labelled_jump` went RED (2, expected 1), then GREEN after the spec gained an explicit `label_kinds` row.
  timestamp: 2026-08-02T15:36:01.692166+00:00
- actor: claude-code
  id: 01kz1hwzxe9g72nkb60bp1khea
  text: |-
    ### implement — changed

    - files (8):
      - `crates/swissarmyhammer-sem/src/parser/plugins/code/complexity.rs` (new) — the Sonar cognitive-complexity scorer, per-language `ComplexitySpec` table, both gate constants, 17 tests
      - `crates/swissarmyhammer-sem/src/parser/plugins/code/mod.rs` — re-export
      - `crates/swissarmyhammer-validators/src/review/probes.rs` — `complexity` catalog entry (fact), `run_complexity`, `FileChange::sources`, 4 tests
      - `crates/swissarmyhammer-validators/src/review/scope.rs` — `run_probe_cache` carries file sources, 2 production-path tests through the real `scope_review`
      - `crates/swissarmyhammer-validators/src/review/verify.rs` — guard rule refuting a complexity claim on an empty complexity fact, 3 tests
      - `crates/swissarmyhammer-validators/src/builtin/mod.rs` — `complexity` moved to the probe-bearing roster
      - `builtin/validators/complexity/VALIDATOR.md` — declares `probes: [complexity]`
      - `builtin/validators/complexity/rules/cognitive-complexity.md` — the rule is a lookup against computed numbers, not a count

    - root cause: the rule asked a language model to COUNT nesting depth and branches. Models count badly, so the same unchanged file produced a different finding set per run. Sampling variance only chose which wrong count came back. The default review agent is `claude-code-haiku` and the `claude` CLI exposes no temperature, top-p, or seed (verified), so pinning the sampler was never available — the count had to leave the model.

    - fix: compute the number with tree-sitter and hand it to the model. A `match`/`switch` now scores +1 for the whole construct and its arms open no nesting level, so the card's false positive is impossible BY CONSTRUCTION rather than by prose.

    - regression pinned: `collect_line_tags` scores 5 at depth 2; `edit_line_markers` scores 12 at depth 3. Both under the gates (15 / 4). Fixtures are the exact pre-flattening source from `4a7bbf7e0^`.

    - determinism in CI: `repeated_scoring_of_one_source_never_drifts` (25 runs), `complexity_rows_are_identical_across_repeated_runs` (10 runs through the real `run_probes`), `the_scorer_is_insensitive_to_the_order_functions_are_declared_in`. No model in any of them.

    - tests: `cargo nextest run -p swissarmyhammer-sem -p swissarmyhammer-validators` — 553 passed, 0 failed. `cargo nextest run -E '(rdeps(swissarmyhammer-validators) or package(swissarmyhammer-sem))'` — 5254 tests, 5251 passed, 3 timed out, 2 skipped.

    - the 3 timeouts are PRE-EXISTING, proved not assumed: `llama-agent agent_tools_mount_lists_intrinsic_tools_with_no_external_servers`, `llama-agent llama_dual_source_aggregation_has_shell_exactly_once`, `swissarmyhammer-agent review_runs_over_acp_against_a_real_local_model`. All three are local-model inference e2e. Re-ran two of them at HEAD with every change stashed: both still time out (300s and 480s). `kanban-app::ai_panel_e2e` hangs the same way and its source has zero references to review, complexity, or validator. `review_real_model_e2e` scopes its run to the `function-length` validator only, which declares no probe, so the complexity probe never executes on its path.

    - `cargo fmt --all --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.

    - follow-ups filed: ^xjyb2qf (scorer covers only Rust — map the remaining languages), ^8p6kjmw (blame shas drift the prompt between finish-loop iterations), ^mxv793g (ValidatorLoader enumerates in HashMap order), ^7wpd955 (batch budget measures raw bytes, plus two stale doc comments).

    - next: /review
  timestamp: 2026-08-02T15:36:23.726160+00:00
- actor: claude-code
  id: 01kz1j272cae1s8sg1kzgj2rgw
  text: |-
    ### finish iteration 1 — changed
    - implement: changed — root cause was the rule asking the model to COUNT. Sampling could not be pinned (the review agent is claude-code-haiku; `claude` exposes no --temperature/--top-p/--seed), so the count moved out of the model. New Sonar cognitive-complexity scorer as a pure tree-sitter function (swissarmyhammer-sem), a fourth `complexity` catalog probe, a verify guard that refutes a complexity claim against an empty result with no model involved, and the rule rewritten as a lookup. Gates: COGNITIVE_COMPLEXITY_THRESHOLD=15, NESTING_DEPTH_THRESHOLD=4. Rust only; every other language reports "not computed", never zero.
    - evidence: the two functions the review flagged as "depth 4" actually score 5 at depth 2 and 12 at depth 3, pinned against the pre-flattening source from 4a7bbf7e0^. Determinism now runs in CI with no model: 25 repeated scorings, 10 repeated run_probes, plus a declaration-order test.
    - test: green — fmt clean, clippy clean, `cargo nextest run -p swissarmyhammer-sem -p swissarmyhammer-validators` 553 passed. A wider run showed 3 timeouts, proven pre-existing by re-running them at stashed HEAD.
    - commit: 8d7d8f57d
    - next: BLOCKED on /review. The review engine currently fails fat batches with a bare `invalid_params` because the batch budget (393,216) is ~4x the agent prompt cap (100,000) — tracked as ^6jsxjbc. This card stays in `doing` until that lands, because a review run today could not distinguish "clean" from "the engine did not run".
    - follow-ups filed: ^xjyb2qf (non-Rust language coverage), ^8p6kjmw (blame shas drift the prompt between finish-loop iterations), ^mxv793g (ValidatorLoader enumerates in HashMap order), ^7wpd955 (stale doc comments; its budget half was folded into ^6jsxjbc)
  timestamp: 2026-08-02T15:39:14.892279+00:00
position_column: doing
position_ordinal: '8280'
title: complexity validator is nondeterministic on tag_parser.rs — same file, different findings per run
---
The `complexity` validator returns a different finding set on repeated runs over the same unchanged file. Observed on `crates/swissarmyhammer-kanban/src/tag_parser.rs` on 2026-07-31 while working ^tnr56gg.

## What was seen

Across repeated runs on one unchanged file:

- Some runs flagged `collect_line_tags` and `edit_line_markers` with "match arms contain code at depth 4".
- Other runs did not flag them at all.
- Later runs additionally raised stylistic items (a missing module `# Examples` section, a `b'` backtick literal in three functions) that earlier runs had not raised.

The match-arm findings were **false positives** against the validator's own documented rule. `builtin/validators/complexity/rules/cognitive-complexity.md` counts nested *conditions*; both functions were two-arm `Option` matches sitting at depth 2. They were flattened from `match` to `if let`/`else` anyway, purely to remove the ambiguity — not because the rule required it.

## Why it matters

The review gate is treated as binary: any open finding means a task is not done. That contract only holds if the gate is deterministic. Nondeterminism produces three concrete failures:

1. **A task can pass or fail on a coin flip.** The same commit reviewed twice can yield clean or not-clean.
2. **It manufactures busywork.** Work gets done to satisfy a finding that a re-run would not have raised, as happened here with the two match flattenings.
3. **It corrupts the finish loop's stuck-detection guardrail.** That guardrail declares a task stuck when the *same* finding survives 3 iterations. A validator whose finding set churns can hide a genuinely persistent problem behind a rotating cast of findings, or trip the guardrail on findings that were never really the same one.

## Investigate

- Whether the validator prompt or its file batching is order-dependent or size-dependent (`batch_size` inlines file bytes per review batch, so a file near a boundary may be split differently between runs).
- Whether the depth rule is being applied by agent judgment where the documented rule is stricter than the prompt conveys — `match` arms counting as a nesting level contradicts the cognitive-complexity doc.
- Whether sampling or temperature in the validator agent is the source, and whether it should be pinned for validators.

## Acceptance

- The same file reviewed N times with no change in between yields the same finding set. Demonstrate with a repeated-run harness, not a single run.
- If `match` arms are intended to count toward nesting depth, `cognitive-complexity.md` says so explicitly. If they are not, the validator stops raising them.

Do not "fix" this by weakening the rule to make findings disappear. The goal is a stable gate, not a quiet one.

Found while driving ^tnr56gg through the finish loop. Related: ^fpcbeth (frontmatter split defect found in the same run). #bug #review