---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kz7fkq2ccsf1azm0gthpdtjw
  text: |-
    Picked up the card with partial work already in the working tree from an earlier pass. Kept all of it.

    **Both "# Changes" items are in `builtin/skills/implement/SKILL.md`.**

    - `### Know the rules` sits between `### Research before writing` and `### Implement`. It prescribes the one call the card names, on the `review` tool, and lists all six obey-items.
    - `### Self-review` sits between `### Implement` and the handoff step. It prescribes `{"op": "review working"}`, the four finding sentences, the repeat-until-clean loop, then `/double-check`, then handoff.

    **Why the Rust files are in this change.** The card tells the skill to call `{"op": "list validators", "match": "<file>", "rules": true}`. Before the fix, an agent that sent the JSON string `"true"` — which agents routinely do where a schema declares a boolean — got a success response carrying `rule_count` and NO rule bodies and NO error. The card's own instruction was silently useless. `bool_arg` now returns `Result<bool, String>`, coerces `"true"`/`"false"`, and errors on a value it cannot read; the `list validators` call site maps that to `invalid_params`. `bool_arg` has exactly ONE production call site, so the signature change has no other blast radius.

    **Discovery — the same cause is still at five sibling sites.** `diagnostics/mod.rs` (`dependents`), `code_context/mod.rs` (`include_source` twice, `include_declaration`), and `shell/grep_history/mod.rs` all read a caller's boolean with a bare `as_bool()` and drop a string flag. `{"op": "check working", "dependents": "false"}` folds dependents in anyway and answers as if it obeyed. Recorded as a new card ^16bjcm9 rather than widened here.

    **Guard test added.** `crates/swissarmyhammer-skills/tests/implement_rules_and_self_review_guidance.rs`, following the existing `*_guidance.rs` pattern. It asserts against the RENDERED skill body (partials expanded), pins the exact call strings, the six obey-items, the four finding sentences, and the step ordering: rules < Implement, Implement < review working < /double-check < handoff. RED verified twice — once against the pre-change SKILL.md (both tests failed for the right reason), and once by moving the `/double-check` sentence above the `review working` block (the ordering assertion caught it).

    **Double-check feedback, all implemented.**

    1. `compatibility:` frontmatter did not declare the `review` MCP tool the skill now requires — added, naming both uses.
    2. The skill said "Send `rules` as a JSON boolean, not as the string `"true"`" while the same change made the string work — the two halves of one change disagreed. Sentence deleted.
    3. `### Know the rules` had been inserted ABOVE the research step's closing "Record what you discovered" line, so that line read as an instruction to record the validator rules. Moved the record line back under `### Research before writing`.
    4. The guard test pinned `review < handoff` but not `review < /double-check`, so the card's actual ordering requirement could regress silently; it also pinned none of the six obey-items. Both fixed.
    5. Five sibling `as_bool()` sites — new card ^16bjcm9.
    6. Acceptance command had no recorded run — run and recorded below.

    **Self-review.** First run returned 6 findings, every one against test code in `review/tests.rs` that already existed at prior commits (`git blame` confirms lines 100, 1128, 1413, 1417, 1418, 1668 belong to c6e9dda6b, b34eb9ced, e6a38400 and 945a7583f). The review skill's blanket exception — "Never ask to refactor existing tests ... Drop the finding" — is an explicit written rule and drops all six. Second run after the double-check fixes: 0 findings, 9 candidates attempted and refuted.
  timestamp: 2026-08-04T22:51:49.196396+00:00
- actor: claude-code
  id: 01kz7fm0mfsks6m9atk41qyxfr
  text: |-
    ### implement — changed
    - evidence: 5 files — builtin/skills/implement/SKILL.md, crates/swissarmyhammer-skills/tests/implement_rules_and_self_review_guidance.rs (new), crates/swissarmyhammer-tools/src/mcp/op_tool_helpers.rs, crates/swissarmyhammer-tools/src/mcp/tools/review/mod.rs, crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs
    - acceptance: `cargo nextest run -E 'rdeps(swissarmyhammer-skills)'` — 8309 tests run: 8309 passed, 0 failed, 0 skipped
    - fmt: `rustfmt --edition 2021 --check` clean on all four Rust files touched (workspace `cargo fmt --all --check` has pre-existing drift in swissarmyhammer-common/src/test_utils.rs, swissarmyhammer-config/src/model.rs, swissarmyhammer-tools/src/health_registry.rs — committed by another session, not this change)
    - clippy: `cargo clippy -p swissarmyhammer-skills -p swissarmyhammer-tools --all-targets -- -D warnings` clean
    - self-review: `review working` — 0 findings (9 attempted, 9 refuted)
    - follow-up card: ^16bjcm9 (five sibling `as_bool()` sites still drop a string flag)
    - next: ready for `/review`; card stays in `doing`
  timestamp: 2026-08-04T22:51:58.991079+00:00
depends_on:
- 01KYVYC5DE70XGP2P9GT6TW0KG
position_column: doing
position_ordinal: '8380'
title: 'Implement skill: read validator rules before coding, self-review before handoff'
---
# Problem

The implementer never reads the validator rules. The review engine then finds the rule violations after the code is written. Each fix pass adds new code that also does not obey the rules. Session 4203e383 used 8 review iterations for one task (^1t92gnj). More than 7 hours went to convergence after the first implementation.

The fix is to follow the rules from the start. Do not weaken the review gate.

Depends on ^t6tw0kg: the skill uses one `rules: true` call per file. A loop of `get validator` calls is an opportunity to fail — do not use it.

Related: ^s948zpf adds the `findings-are-requirements` partial that carries the reporting-language stance (findings verbatim, no severity words) into every agent and skill.

# Changes

## 1. `builtin/skills/implement/SKILL.md` — add a "Know the rules" step

Add to the "Research before writing" section:

- Before you edit a file, get the rules that review will enforce on it — one call: `{"op": "list validators", "match": "<file path>", "rules": true}` on the `review` tool. The response carries every applicable rule body verbatim.
- Obey each rule when you write the code, not after: document each public item, name each numeric constant, do not copy blocks, keep functions small and flat, follow the project naming, delete dead code.

## 2. `builtin/skills/implement/SKILL.md` — add a "Self-review" step

Add a new step before the `/double-check` step:

- Run `{"op": "review working"}` on your changes.
- Fix every finding. A finding is a requirement. Do not rank findings. Do not defer findings. Do not label findings.
- Run the review again. Repeat until the review is clean.
- Only then hand off for the formal `/review`.

Rationale: one author-side review run costs ~15 minutes. One full implement→test→review iteration costs ~50 minutes. The self-review replaces iterations.

# Acceptance

- `cargo nextest run -E 'rdeps(swissarmyhammer-skills)'` passes.
- The implement skill instructs the agent to fetch the applicable rules with one `rules: true` call per file before editing, and to run `review working` until clean before handoff.