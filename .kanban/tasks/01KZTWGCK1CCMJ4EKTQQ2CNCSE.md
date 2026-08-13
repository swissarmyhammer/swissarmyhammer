---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzw8ps4cfs24v7ncyp457hkg
  text: |-
    Research — the real path a finding travels

    - `Finding` already carried `validator` and `rule` (`review/types.rs`). Nothing was lost in the engine; the render dropped it. `synthesize::render_item` wrote only `` - [ ] `file:line` — claim ``.
    - The GFM checklist is rendered in RUST, one place only: `synthesize::render_item`. The review skill and the reviewer agent copy `report.markdown` verbatim, so no markdown layer builds items of its own.
    - `fleet::tag_findings` overwrote `validator` authoritatively but left `rule` exactly as the agent typed it — so the rule name was whatever a model invented. The e2e fixture proved it: it emitted `"rule": "r"`.
    - Tool rules already set both names authoritatively (`tool_rules.rs` sets `finding.rule = Some(run.rule)`), so only the prompt fan-out path needed attribution.
    - The `review` MCP tool streams whole `Finding` JSON under logger "review" (`review_op/progress.rs`). It serializes the same struct, so it inherits the resolved names with no change; its test already asserts `validator` and `rule` on the streamed finding.

    The fan-out shard is per VALIDATOR, not per rule — one prompt carries every prompt rule of a set (code-hygiene ships 31). So the engine cannot know which rule fired by construction; it can only pin what the agent cited to a real roster name.
  timestamp: 2026-08-13T00:35:12.652003+00:00
- actor: claude-code
  id: 01kzw8q5z1m79ccd35sr0j9bft
  text: |-
    What landed

    - `Finding::attribution()` (`review/types.rs`) is the ONE place the `set/rule` spelling lives, with `UNATTRIBUTED_RULE` as its documented fallback.
    - `fleet::resolve_rule` pins the rule an agent cited to a name the validator's loaded roster really carries. Rungs: exact name, then normalized spelling ("Magic Numbers" -> `magic-numbers`), then exactly one roster name whose words wrap or are wrapped by the cited name ("no-magic-numbers" -> `magic-numbers`), then the shard's sole rule when it showed only one. Word runs, never raw substrings — "r" sits inside "numbers" as text but cites no rule.
    - The roster is the validator's WHOLE loaded rule list, wider than the shard: a prompt rule a healthy tool rule superseded left the shard but is still a real rule document. That also keeps attribution independent of local tool health, so the test is not machine-dependent.
    - `tag_findings` now resolves the rule beside the validator and warns when it cannot, so neither half of the attribution is the agent's to decide.
    - `synthesize::render_item` renders `` - [ ] `file:line` `set/rule` — claim. suggestion. ``
    - The fan-out output contract now tells the agent to copy one of the `### Rule:` names it was shown, spelled exactly.
    - Docs: `builtin/skills/review/SKILL.md` states the item shape.

    Approaches ruled out, and why

    - Dropping a finding whose rule will not resolve: the codebase's own history forbids it. A missing `validator` field once failed the parse and degraded a whole batch to zero findings — a real review that silently found nothing. A model that omits `rule` is common, so dropping would gut reviews.
    - Fan-out per rule (the only way to make attribution total by construction): one forked session and sweep per rule instead of per validator. code-hygiene alone would go from 1 task to 31. Not sanctioned by the card and a 10-30x agent-turn change.
    - Passing an invented rule name straight through: it names a document that does not exist, which is the failure the card is about.
  timestamp: 2026-08-13T00:35:25.793306+00:00
- actor: claude-code
  id: 01kzw8qhee8xj4xzvzxfj7y4sj
  text: |-
    The test that holds the shape, and the one residual

    Real-path test: `review_e2e_every_task_finding_names_a_roster_set_and_rule` (`crates/swissarmyhammer-tools/tests/integration/review_e2e.rs`). It drives the registered production `review` tool over the planted temp repo, writes the report onto a REAL file-backed kanban board the way the review skill does, reads the description back, and for EVERY `- [ ]` item reads the `set/rule` span out of the item text and checks both names against the roster `load_rules(Some(repo_path))` returns — the same call the tool makes. It asserts `counts.skipped == 0` first, so every item really comes from a loaded validator.

    RED verified by mutation: changing one fixture rule name to `bogus-rule` fails the test with
      `unattributed` must be a rule of the `code-security` set: - [ ] `src/payments.rs:5` `code-security/unattributed` — ...
    Restored after.

    The fixture now emits real roster rule names (`duplication`, `reuse`, `no-secrets`, `data-driven`, `dead-code`, `type-safety`, `error-handling`) instead of the placeholder `"r"`, which is what a real agent does — the prompt lists the names.

    Residual, named rather than hidden: a finding whose cited rule matches nothing in the roster, on a shard that showed more than one rule, renders as `set/unattributed`. It still names the set, it is logged with the cited name, and the strengthened output contract plus the resolution ladder make it rare. Closing it completely needs per-rule fan-out — see the ruled-out list above. Worth a separate card if it shows up in real runs.
  timestamp: 2026-08-13T00:35:37.550810+00:00
- actor: claude-code
  id: 01kzw8qqz02aqnf0h4b5gp1gfg
  text: |-
    ### implement — changed
    - evidence: 7 files — crates/swissarmyhammer-validators/src/review/types.rs, .../review/fleet.rs, .../review/fleet/tests.rs, .../review/synthesize.rs, crates/swissarmyhammer-tools/tests/integration/review_fixture.rs, .../review_e2e.rs, builtin/skills/review/SKILL.md. `cargo nextest run -p swissarmyhammer-validators -p swissarmyhammer-tools`: 2228 passed, 0 failed. `cargo nextest run -p swissarmyhammer-agent -p swissarmyhammer-cli`: 779 passed. `cargo fmt --all -- --check` clean, `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - next: /review
  timestamp: 2026-08-13T00:35:44.224068+00:00
- actor: claude-code
  id: 01kzwadyxk5nyzma1n6236k0t3
  text: |-
    ### review — findings
    - evidence: `review sha 18fd3f260~1..18fd3f260`. counts: 2 findings, 2 confirmed, 36 refuted, 24 attempted, 0 failed, 1 skipped file. 1 finding recorded — crates/swissarmyhammer-validators/src/review/fleet/tests.rs:1. 1 finding dropped under the review skill's test-refactor exception (see below). 0 findings dropped for a false premise.
    - next: split `review/fleet/tests.rs` so the duplication validator can read it, then re-review.

    Acceptance criterion of this card, measured on this run: the findings this run produced DO NOT carry the `set/rule` attribution beside the `file:line`. The cause is not the committed code — it is that the live engine predates the commit.

    - Every running `sah serve` executes `/Users/wballard/.cargo/bin/sah`, mtime 2026-08-12 10:36. Commit 18fd3f260 landed 2026-08-12 20:41:39, ten hours later. Verified per pid with `lsof -d txt`. So the renderer that produced this run's markdown is the OLD `render_item`.
    - The committed code does produce the shape. `cargo test -p swissarmyhammer-validators --lib attribution` passes 3 tests: `review::types::tests::attribution_names_the_set_and_the_rule`, `attribution_falls_back_to_the_unattributed_rule_name`, `review::fleet::tests::resolve_rule_reports_no_attribution_for_a_multi_rule_shard_with_no_name`. The synthesize renderer test asserts the exact item shape ``- [ ] `path/to/file.rs:42` `dead-code/no-unused` — ...``. Full `cargo test -p swissarmyhammer-validators --lib review::`: 493 passed, 0 failed.
    - The attribution therefore stays unproven ON A REAL RUN until a `sah` built from 18fd3f260 or later is installed and the MCP servers restart. That is a deployment step, not a code change.

    Dropped finding, stated in full so the drop is auditable:
    `crates/swissarmyhammer-tools/tests/integration/review_e2e.rs:194` — "Hardcoded worker thread count `2` should be a named constant — it configures test parallelism. Extract to a named constant `const TEST_WORKER_THREADS: usize = 2;` and reuse."
    Dropped because its subject is test code that already existed. The premise is TRUE — line 194 is `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]`. But the commit's only hunk is `@@ -359,3 +359,103 @@`, and line 194 is byte-identical in `18fd3f260~1` and `18fd3f260`. The review skill's blanket exception covers it: never record a finding that asks to refactor test code that already existed.
  timestamp: 2026-08-13T01:05:20.819888+00:00
- actor: claude-code
  id: 01kzwaf9y082y1w9dhhdp6bzgx
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 7 files. The trace found the gap was sharper than this card assumed. `Finding` ALREADY carried `validator` and `rule`; the checklist renderer dropped both, and `fleet::tag_findings` overwrote `validator` authoritatively while passing `rule` through exactly as the agent typed it — so the rule name was whatever a model invented. The e2e fixture proved it by emitting `"rule": "r"`. `Finding::attribution()` is now the single place the set/rule spelling lives, and `fleet::resolve_rule` pins a cited rule to a name the validator's loaded roster really carries, matching whole words so `"r"` inside `"numbers"` cites nothing. The roster is the validator's whole loaded rule list rather than the shard, so a rule a superseding tool rule displaced still resolves.
    - test: green — cargo nextest run --workspace, 14163 passed, 0 failed, 0 skipped. fmt and clippy clean.
    - commit: 18fd3f260
    - review: findings — crates/swissarmyhammer-validators/src/review/fleet/tests.rs:1. 2 findings raised, 1 dropped under the skill's written test-refactor exception (a pre-existing `worker_threads = 2` on a line byte-identical either side of the commit).

    **The acceptance criterion is met in code and under test, and is UNPROVEN on a real run.** The findings this review produced do not carry the new `set/rule` shape, and the cause is not the code: every running `sah serve` executes `/Users/wballard/.cargo/bin/sah`, mtime 2026-08-12 10:36, while this commit landed 20:41:39 — ten hours later. The renderer that produced this run's markdown is the old one. Confirmed per pid with `lsof -d txt`. The shape is held instead by the renderer's own tests, which assert the exact item form ``- [ ] `path/to/file.rs:42` `dead-code/no-unused` — ...``. Proving it on a live run needs a `sah` built from 18fd3f260 or later and an MCP restart. That is a deployment step, not a code change.
  timestamp: 2026-08-13T01:06:04.864965+00:00
- actor: claude-code
  id: 01kzwb57wa3kpp9tcbg8kye0fk
  text: |-
    Splitting `review/fleet/tests.rs` — how the seams were chosen

    The file already carried its seams as `// ---- subject ----` markers. Each marker became a module, named for its subject, in the idiom `review/tool_rules/tests.rs` already uses: the parent keeps the module doc, the `mod` list, the imports and the shared fixtures; each child opens with a `//!` doc and `use super::*;`.

    - `reask` — the one re-ask a forked task gets when its reply cannot be read.
    - `budget` — the config constants, the over-cap verdict, the rendered measure the packer costs a file by, and the framing's share. These four markers stand in ONE module because they share three fixtures (`changed_set_duplicates`, `bare_file_work`, `short_line_source`); splitting them would have forced those fixtures up into the parent, which is a relocation, not a seam.
    - `renderer` — the prime, the validator suffix, the monolithic fallback, the output contract. No agent runs.
    - `fanout` — the orchestrator against a scripted agent, and the follow-up sweep.
    - `forking` — the primed prefix and the forks on it, including the degraded fork modes.
    - `progress` — the progress stream, and the tally a run reports when tasks fail. The failure tests read the same stream through `drain_progress`, so they stand here rather than in a module of their own.
    - `reuse` — warm/cold classification of a fork.
    - `attribution` — `resolve_rule` and `tag_findings`, this card's own subject.

    Purity, proved rather than asserted: every line of the nine files, concatenated in the original order with the module headers stripped, is byte-identical to the original file minus the 22 section-marker and blank lines the docs replaced (`diff` reports no difference). No test logic, assertion, or name changed.

    Test count: 68 before, 68 after — `attribution` 8, `budget` 17, `fanout` 8, `forking` 7, `progress` 6, `reask` 2, `renderer` 14, `reuse` 4, and the two harness self-tests that stay with the harness in the parent.
  timestamp: 2026-08-13T01:18:03.658143+00:00
- actor: claude-code
  id: 01kzwb5qne83ykqp51ejtd6wx0
  text: |-
    Measured sizes against the 262144-byte cap, and the model that produces them

    A file's rendered block is `## File:` header + the numbered source + the semantic diff + the probe evidence. `render_numbered_lines` prints `{line:>6} | {sha:8} {mark} | {text}`, so the source render costs `raw + 22 bytes per line`, plus about 1.7 KB of fixed block headers and the legend. Calibrating the remaining term (diff + duplication probe evidence) on the one measured point — the original file, 126606 raw bytes, 3381 lines, 319188 rendered — gives `rendered ≈ 1.92 × raw + 22 × lines + 1700`. That model reproduces the measured 319188 to within 22 bytes.

    Every resulting file, estimated by that model:

    | file | raw | lines | est. rendered | share of cap |
    | --- | --- | --- | --- | --- |
    | `fleet/tests.rs` | 13638 | 346 | 35496 | 13% |
    | `fleet/tests/budget.rs` | 29113 | 767 | 74470 | 28% |
    | `fleet/tests/renderer.rs` | 21950 | 562 | 56208 | 21% |
    | `fleet/tests/fanout.rs` | 20282 | 549 | 52719 | 20% |
    | `fleet/tests/progress.rs` | 17653 | 487 | 46307 | 17% |
    | `fleet/tests/forking.rs` | 16318 | 438 | 42666 | 16% |
    | `fleet/tests/attribution.rs` | 4666 | 128 | 13474 | 5% |
    | `fleet/tests/reask.rs` | 4009 | 117 | 11971 | 4% |
    | `fleet/tests/reuse.rs` | 1850 | 59 | 6550 | 2% |

    None passes the cap. The largest sits at 28% of it, so the headroom is real, not a byte.

    The rest of the review tree: no other file passes the cap. The closest is `review/drive.rs` — 94844 raw, 2286 lines, est. 232354, which is 89% of the cap. Its floor (the source render alone, before diff and probes) is 145098, so it is under for every validator today, but it is the next file the cap will catch. `synthesize.rs` (est. 185832) and `test_support.rs` (est. 179405) follow.

    The cap is hit in more than one place, outside this tree. Six files in the repository pass the cap on the SOURCE RENDER ALONE — over the cap for every validator, not only for duplication: `kanban/src/scope_commands.rs` (floor 313824), `sem/src/parser/plugins/code/complexity.rs` (302861), `kanban/src/dispatch.rs` (284258), `tools/tests/integration/file_tools_integrations.rs` (276387), `config/src/model.rs` (267136), `entity/src/context.rs` (262659). Raised as ^0fn6dbf.
  timestamp: 2026-08-13T01:18:19.822151+00:00
- actor: claude-code
  id: 01kzwb66eatk92n66yyfnz5z3b
  text: |-
    ### implement — changed
    - evidence: 9 files — crates/swissarmyhammer-validators/src/review/fleet/tests.rs (rewritten as the shared parent), and the new crates/swissarmyhammer-validators/src/review/fleet/tests/{attribution,budget,fanout,forking,progress,reask,renderer,reuse}.rs. Pure move, proved byte-identical by diff. Tests: 68 fleet tests before, 68 after; `cargo nextest run -p swissarmyhammer-validators`: 708 passed, 0 failed, 0 skipped. `cargo fmt --all -- --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean. Largest resulting file 29113 raw bytes, est. 74470 rendered — 28% of the 262144-byte cap.
    - next: /review. New card ^0fn6dbf raised for six files elsewhere in the repository that pass the cap on their source render alone.
  timestamp: 2026-08-13T01:18:34.954119+00:00
position_column: doing
position_ordinal: '8280'
title: A recorded finding must name the validator set and the rule that produced it
---
A review finding recorded on a kanban task names a file and a line. It never names the validator set or the rule that produced it.

## Why this costs

An implementer who picks up a card with open findings cannot tell which rule to read. To act on a finding, or to judge whether the rule measures the right thing, the implementer must infer the rule from the wording of the finding.

Measured on 2026-08-12, unsticking ^wwb6hk7: three findings stood on the card. The rule behind the duplication finding was confirmed only by reading `builtin/validators/duplication/rules/duplication.md` and matching its carve-out text to the finding. The rule behind the other two took four searches over `builtin/validators/` to reach `completeness/rules/invariant-propagation.md`, and the attribution was never proved — only inferred from the wording.

The finish skill states that a rule which measures the wrong thing produces findings that are NOT requirements, and that a person must correct the rule. That decision needs the rule's name. Today the name is guesswork.

## What to do

- Carry the validator set name and the rule name on each finding, from the engine through to the GFM checklist a review writes on a task.
- State them in the checklist item, beside the `file:line`.
- Hold the shape with a test: a finding written to a task names a set and a rule that both exist in the loaded roster.

## Done when

- Every item of a `## Review Findings` section names its set and its rule.
- A reader can open the rule from the finding with no search.

#tool-validators

## Review Findings (2026-08-12 20:42)

> ⚠️ 1 file(s) not reviewed — the rendered prompt would exceed the agent's prompt cap:
> - `crates/swissarmyhammer-validators/src/review/fleet/tests.rs` — 319188 rendered bytes, over the 262144-byte per-file cap; not reviewed by: duplication (split the file)

- [x] `crates/swissarmyhammer-validators/src/review/fleet/tests.rs:1` — This file exceeds the review prompt cap — 319188 rendered bytes against the 262144-byte per-file cap — so these validators could not review it: duplication. Split the file into smaller modules that fit the review prompt cap.