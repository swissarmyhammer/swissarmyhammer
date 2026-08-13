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