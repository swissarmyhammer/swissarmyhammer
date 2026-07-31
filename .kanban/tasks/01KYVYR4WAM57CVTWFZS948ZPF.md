---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kyx3wdz0cb2m1h8meq0zsr6x
  text: |-
    Research findings on the partial include mechanism (verified, not assumed):

    - Include syntax is Liquid: `{% include "_partials/<name>" %}`. 17 partials survive; `validator-tools.md` and `architecture-awareness.md` are the models.
    - Partial discovery is AUTOMATIC. `crates/swissarmyhammer-templating/build.rs` runs `BuiltinGenerator` over `../../builtin/_partials` with extensions stripped, and `resolver.rs::load_builtin_partials()` adds each as `_partials/<name>`. A new partial file needs ZERO Rust changes and no name list update.
    - Neither `SkillResolver::resolve_builtins()` nor `AgentResolver::resolve_builtins()` renders. Both return RAW bodies with the tag still present. Expansion happens in the consumers: `mirdan/src/install.rs` (`render_profile_skill`, `install_profile_agents`), `swissarmyhammer-tools` skill `use_op.rs`, `claude-agent/src/agent.rs`, and `llama-agent/src/acp/config.rs`.
    - So agents DO expand partials in production. Confirmed both in code and empirically: `_partials/architecture-awareness` renders as prose in a live implementer system prompt.
    - Gap discovered: NOTHING tested that a builtin AGENT.md renders at all. `crates/swissarmyhammer-agents` had no `tests/` directory. `mirdan/src/install.rs` tests only assert `AGENT.md` exists. `swissarmyhammer-templating/tests/all_skills_render_test.rs` covers skills only, with no agent equivalent. The installer only logs a warning and falls back to the RAW body when a render fails, so a broken agent include would silently ship the literal Liquid tag to the model.

    Approach chosen because of that gap: the agent guard lives in `crates/mirdan/tests/` and drives the real `AgentResolver::resolve_builtins()` + `TemplateLibrary::render_text()` pair. mirdan is the only crate that depends on swissarmyhammer-agents, swissarmyhammer-skills, and swissarmyhammer-templating at once, and it is in the rdeps set of both crates named in the acceptance command.

    Rejected approach: a hand-rolled `replace(tag, partial_body)` expander in a new `crates/swissarmyhammer-agents/tests/` file. It was written and then deleted. It proves nothing about whether the `_partials/` prefix resolves, which is exactly the wiring that was untested.

    The skill guard stays in `crates/swissarmyhammer-skills/tests/` next to its four sibling `*_guidance.rs` tests and reuses their `tests/common/mod.rs` helpers, so the split is: skills guard = 3 skills + single source of truth, agents guard = 4 agents through the production renderer. No overlap.
  timestamp: 2026-07-31T22:14:27.552110+00:00
- actor: claude-code
  id: 01kyx3wsg6sv1dtzt91qtfwv7b
  text: |-
    Implementation landed. TDD cycle and verification evidence:

    RED first. Both guard tests were written before the partial existed. 3 tests failed for the right reasons: "builtin skill 'implement' must render the findings-are-requirements stance", and the single-source check reporting `left: []` because no builtin file carried the stance yet.

    GREEN after adding the partial and the 7 includes: 3 passed.

    Removal proof, one file at a time. For each of the 7 files, the include line was stripped, the guard was run, and the file was restored from a backup copy. All 7 went RED:
    - implementer / reviewer / tester / committer AGENT.md -> "builtin agent '<name>' must include the findings-are-requirements partial"
    - implement / finish / review SKILL.md -> "builtin skill '<name>' must render the findings-are-requirements stance"
    Then GREEN again on restore. The guard cannot be silently dropped.

    Full verification:
    - `cargo nextest run -E 'rdeps(swissarmyhammer-skills) or rdeps(swissarmyhammer-agents)'` -> 10204 tests run, 10204 passed, 2 skipped, 251s.
    - `cargo fmt --all` -> clean, no diff.
    - `cargo clippy --workspace --all-targets -- -D warnings` -> clean.

    Canonical guard sentence is "Do not decide you know better than the rule." It exists in exactly one file under `builtin/`, which `assert_guidance_single_source` enforces.

    finish/SKILL.md Scope now keeps three loop-mechanics bullets (findings are in scope by definition, the review gate is the only way to `done`, re-review is not noise) and delegates the stance to the include. Step 7's "(per Scope)" cross-reference still resolves, because the partial defines a true conflict and routes it to a stuck task.
  timestamp: 2026-07-31T22:14:39.366509+00:00
- actor: claude-code
  id: 01kyx520pg282r3d662rayhj07
  text: |-
    Self-review found 11 items. All 11 are answered. The important ones were real defects, not polish:

    1. The stance shipped TWICE. `builtin/skills/review/SKILL.md` kept its own longhand copy with a DIFFERENT exception list, so one prompt held two absolute and disagreeing rules. Deleted the two duplicated bullets ("Findings are obeyed, never declined", "Fix at the root, not the cited line"). The review skill keeps only driver text.
    2. The partial said "Only one condition is an exception", which contradicted the review skill's "Never ask to refactor existing tests — blanket exception, overrides every other rule". Both were absolute, so a model had to break one. The partial now names two release conditions: an explicit written rule that drops the finding, and a true conflict. Raising a finding and obeying a finding are now separate concerns.
    3. Content loss in finish. The deleted prose had a third conflict category — a rule that fights a documented contract, with the `snake_case` backend-payload and `T | null` examples — and said "compile / type-check". The partial had neither, so an implementer with a finding that breaks a wire contract had no way to report it, and non-compiled languages fell outside the wording. Both restored in the partial.
    4. `finish` step 7 pointed at "(per Scope)", but the conflict text moved out of Scope into the included section. Now points at "(per **Findings Are Requirements** below)".
    5. Content loss: "If findings feel like churn or pedantry, that means the right fix hasn't been found yet." The partial forbade the words but did not answer the feeling. Added "A finding that seems unnecessary shows you did not find the correct fix yet." Also restored "as a blocker" and "A person corrects the rule and starts the work again."
    6. The single-source guard pinned one sentence and so missed the duplicate in item 1. It now pins two: the anti-editorializing sentence and "There is no severity tier. Every finding is mandatory."
    7. The skills guard used the hand-rolled test expander, not the production renderer, so it would stay green if the partial stopped being embedded. The mirdan guard now covers all 7 files (4 agents + 3 skills) through the real `TemplateLibrary::render_text`.
    8. Fixed a stale cross-reference to the deleted `crates/swissarmyhammer-agents/tests/` path.
    9. The frontmatter description claimed "every coding agent and skill", which is not true. Reworded to "Rules and findings are requirements. Obey them." Wider coverage is a follow-up, see below.
    10. Style: reworded three sentences for ASD-STE100. "weigh the cost" -> "compare the cost of a rule with its benefit"; "demands" -> "requires"; unpacked the nested relative clauses.
    11. In `committer/AGENT.md` the h3 landed after `## Safety` and read as a child of Safety. Moved the include above `## Guidelines`.

    Re-proved the guard after the rewrite. Three kinds of mutation, each caught:
    - Strip the include from each of the 7 files in turn -> RED every time (agents fail the agent test, skills fail both the skills-crate test and the mirdan skills test). GREEN on restore.
    - Drop "nit" from the partial's forbidden-label list -> RED, "must forbid labelling a finding 'nit'".
    - Append the stance to a second builtin file -> RED, and it names the offender: `found in: ["_partials/findings-are-requirements.md", "skills/kanban/SKILL.md"]`.

    Final: `cargo nextest run -E 'rdeps(swissarmyhammer-skills) or rdeps(swissarmyhammer-agents)'` -> 10205 passed, 2 skipped. `cargo fmt --all` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.
  timestamp: 2026-07-31T22:34:59.152716+00:00
- actor: claude-code
  id: 01kyx68fn44c0305hk4ck8an60
  text: |-
    Review finding closed: the mirdan coverage guard now pins BOTH stance sentences.

    `crates/mirdan/tests/findings_are_requirements_coverage.rs` — `CANONICAL_STANCE` is now a two-element `&[&str]` ("Do not decide you know better than the rule." and "There is no severity tier. Every finding is mandatory."), the same shape as `CANONICAL_STANCE` in `crates/swissarmyhammer-skills/tests/findings_are_requirements_guidance.rs`. `assert_renders_stance` loops the list, so all four agents and all three skills must render both sentences. The doc comment says the two lists must stay identical and why.

    RED -> GREEN proof. Deleted only the second sentence from `builtin/_partials/findings-are-requirements.md`:
    - Before the fix: `cargo nextest run -p mirdan --test findings_are_requirements_coverage` -> 2 tests, 2 passed. That is the gap the finding names.
    - After the fix, same mutation: 2 failed. "builtin agent 'implementer' must render the stance sentence: There is no severity tier. Every finding is mandatory." and the same message for skill 'implement'.
    - Partial restored (diff-clean against HEAD): 4 tests pass across both guard files.

    Whole-file sweep of the guard, each pin compared with the partial:
    - BANNED_LABELS — the 6 labels are exactly the quoted labels in the partial. Complete.
    - "mark the task stuck" — matches the sibling guard word for word. No drift.
    - COVERED_AGENTS + COVERED_SKILLS = 7 files, and exactly 7 builtin files include the partial (4 agents, 3 skills). Complete.
    - PARTIAL_TAG — present in the raw body, absent after render. Complete.
    - `profile_template_context()` matches the installer helper in `crates/mirdan/src/install.rs` (version only). No covered file or partial uses `{{ agent }}`, so the extra var `render_profile_skill` sets changes nothing here.

    Rejected a shared cross-crate constant (a `test-support` feature on swissarmyhammer-skills): it would put test assertion data in a production API to remove one duplicated literal. The parallel two-element arrays plus the cross-reference comment keep the two guards recognisably paired.

    Verification: `cargo nextest run -E 'rdeps(swissarmyhammer-skills) or rdeps(swissarmyhammer-agents) or rdeps(mirdan)'` -> 10205 passed, 2 skipped, 0 failed. `cargo fmt --all` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.
  timestamp: 2026-07-31T22:55:59.652515+00:00
position_column: doing
position_ordinal: '8280'
title: '_partials/findings-are-requirements: one rule-obedience stance, included by every coding agent and skill'
---

# Problem

Across sessions and models, agents editorialize about rules: findings get labeled "nits", rules get weighed against their cost, and agents propose to weaken the review loop instead of obeying it. The finish skill carries strong anti-dismissal language, but only finish has it. The implementer, reviewer, tester, and the other skills do not — so the stance does not reach the agents that write the code.

The counter-stance must ship in the product, once, and render into every prompt — the same pattern as `_partials/validator-tools.md` ("You are a judge, not an editor").

# Changes

## 1. New partial: `builtin/_partials/findings-are-requirements.md`

Short, strong, declarative. Content:

- A validator rule is a requirement. A review finding is a requirement.
- There is no severity tier. Never write "nit", "minor", "cosmetic", "polish", "pedantry", or "churn" about a finding — not in reports, comments, commits, or summaries. State findings verbatim.
- Do not weigh a rule's cost. Do not skip a rule to save time or effort. Do not decide you know better than the rule.
- Satisfy a finding by removing its cause across the whole file, not only at the cited line.
- Only a true conflict — two rules that cannot both hold, or a rule that demands code that cannot compile — is reportable. Record it on the task, mark the task stuck, and stop. Never resolve it yourself and never edit a validator.

## 2. Include it everywhere code gets written or judged

- `builtin/agents/implementer/AGENT.md`
- `builtin/agents/reviewer/AGENT.md`
- `builtin/agents/tester/AGENT.md`
- `builtin/agents/committer/AGENT.md`
- `builtin/skills/implement/SKILL.md`
- `builtin/skills/finish/SKILL.md` (replace the overlapping prose in Scope with the include where the render allows; keep the loop-mechanics text)
- `builtin/skills/review/SKILL.md`

## 3. Coverage guard

A test that asserts each of the agents and skills above renders the partial's text — so a future agent or skill cannot silently drop the stance.

# Acceptance

- `cargo nextest run -E 'rdeps(swissarmyhammer-skills) or rdeps(swissarmyhammer-agents)'` passes.
- Each listed agent and skill includes `_partials/findings-are-requirements`.
- The coverage-guard test fails when an included file removes the partial. #review

## Review Findings (2026-07-31 17:38)

- [x] `crates/mirdan/tests/findings_are_requirements_coverage.rs:30` — The canonical stance consists of two sentences that must both be verified, but this file only verifies one. The change purpose explicitly states both sentences ('Do not decide you know better than the rule.' and 'There is no severity tier. Every finding is mandatory.') were duplicated separately and must both be pinned. File 1's CANONICAL_STANCE contains only the first sentence, while crates/swissarmyhammer-skills/tests/findings_are_requirements_guidance.rs:26-29 verifies both. This creates incomplete coverage for agents: they pass File 1's test even if the second sentence is missing from their rendered output. Update CANONICAL_STANCE at line 30 to include both sentences (either as a two-element array matching the skill test file, or as a single concatenated string), and update the assertion logic at line 56 to verify both sentences are rendered. This ensures agents receive the same complete verification as skills.
