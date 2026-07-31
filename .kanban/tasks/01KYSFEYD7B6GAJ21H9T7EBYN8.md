---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kyvr0nwbpcxnv6yemvs52aq0
  text: |-
    Picked up by /finish #bug (card 2). No prior attempts.

    Live confirmation the defect is real, from this session: `add task` with `tags: ["bug","kanban"]` still returned `tags: []` at 20:42 — because the fix from ^1t92gnj is committed but the running MCP server is the older binary. Same silent-drop family, different mechanism. Not this card's problem, but it shows the deploy/runtime split matters when verifying.

    Scope reminder for whoever works this: the fix is NOT to add a `hooks` field to the two structs. That fixes one symptom of twelve. Claude Code supports 12 frontmatter fields SAH currently drops — `when_to_use`, `argument-hint`, `arguments`, `disable-model-invocation`, `user-invocable`, `disallowed-tools`, `model`, `effort`, `background`, `hooks`, `paths`, `shell`. Make the round-trip lossless with a flattened catch-all, and keep `profiles` (SAH-internal) out of the deployed file via one shared named constant.

    Agents already round-trip correctly — they deploy verbatim as symlinks into `~/.agents/<name>/`. Only the skill path re-serializes through a closed struct. That asymmetry is the bug.
  timestamp: 2026-07-31T09:27:49.387475+00:00
- actor: claude-code
  id: 01kyvsa2vgzjmdt9v22ebv5njn
  text: |-
    Implemented. Round-trip is now lossless.

    ## Working tree on pickup

    A prior run was killed mid-flight by an API error and left the tree half-applied: both tests existed, `skill.rs`/`skill_loader.rs` were done, but `deploy.rs` declared `SkillFrontmatter.extra` without ever constructing it. The tree did not compile. The design matched the card, so I kept it and finished it instead of starting again.

    ## RED proved first

    To watch the tests fail I reverted the production half (kept the tests), then ran them.

    - Unit `test_format_skill_md_round_trips_unmodeled_frontmatter_keys` FAILED. Output showed the deployed frontmatter reduced to `name` + `description` + `license` — `hooks`, `model`, `paths`, `disable-model-invocation` all gone.
    - Real-path `init_profile_preserves_unmodeled_skill_frontmatter` FAILED.

    Then I restored the production half and completed it.

    ## Change

    - `skill_loader.rs` — `#[serde(flatten)] extra: BTreeMap<String, serde_yaml_ng::Value>` catch-all, then `retain` to drop internal keys, so "extra never holds an internal key" is an invariant of every parsed Skill.
    - `skill.rs` — `pub extra` on `Skill`, plus `SAH_INTERNAL_FRONTMATTER_KEYS` as the one named constant both sides read.
    - `deploy.rs` — `SkillFrontmatter` gains a flattened `extra`, filled from `Skill::extra` and filtered again through the same constant.

    ## Is any key besides `profiles` SAH-internal? No — checked, not assumed

    The loader names 9 keys; deploy writes 8. The set difference is exactly `{profiles}`. `context` and `agent` look SAH-specific but deploy already writes them today and `test_format_skill_md_round_trips_context_and_agent` pins that behavior, so they are not internal. `validation.rs` gates only `name`/`description` and treats no key as internal. So `SAH_INTERNAL_FRONTMATTER_KEYS = ["profiles"]` is correct.

    ## Second live casualty found

    The real-path test failed first not on `finish`, but on `double-check` — `builtin/skills/double-check/SKILL.md` carries `background: false`, which was also being dropped. So the bug had two live victims, not one. Both survive now.

    ## Real-path evidence, not just unit

    Built `sah` and ran a genuine `sah init project` into a temp dir with HOME redirected. The deployed `.skills/finish/SKILL.md` now carries the full block:

    ```
    hooks:
      Stop:
      - hooks:
        - type: command
          command: sah tool ralph ralph check --
    ```

    `profiles:` does not leak (0 hits). `double-check`'s `background: false` is present. Note the nested sequence re-indents (2 spaces vs 4 in source) — serde_yaml_ng's emitter style. Semantically identical YAML; the tests compare parsed values, not text.

    ## Flatten hazard checked empirically

    The card flagged flatten vs the renamed `allowed-tools`. Covered by `test_format_skill_md_writes_each_modeled_key_once`, which re-parses the output and counts each key exactly once.

    I also probed a hazard the card did not name: `#[serde(flatten)]` routes sibling fields through `deserialize_any`, which can break `metadata: HashMap<String, String>` when a value is an unquoted YAML scalar. Ran a live probe with `version: 1.0`, `count: 3`, `flag: true` — all still coerce to strings. No regression.

    ## Test status

    `cargo nextest run -E 'rdeps(swissarmyhammer-skills)'` — 10080 run, 10079 pass. `cargo fmt` and `cargo clippy --workspace --all-targets -- -D warnings` clean.

    The single failure `work_the_card_skills_render_record_progress_guidance` is NOT mine. It asserts on builtin skill BODY text; `builtin/skills/finish/SKILL.md` has no `_partials/record-progress` include at HEAD nor in the working tree. I proved it by reverting my whole change and re-running that test alone — it fails identically. `builtin/` is off-limits on this card (the user edits it in parallel).

    ## What did not work

    `files edit` with `replace_all: true` reported `replacements_made: 1` and patched only the first of five identical `Skill { .. }` literals. Do not trust that flag here — verify the count, or use a `perl -0pi` pass as I did for the remaining four.
  timestamp: 2026-07-31T09:50:26.160646+00:00
- actor: claude-code
  id: 01kyvte348ay1ktwch5eztprn1
  text: |-
    Adversarial review round. Verdict was REVISE with 8 findings. Fixed 6, deferred 1 to a new card, left 1 deliberately.

    ## The finding that mattered — my internal-key filter was untested

    The reviewer proved you could delete `SAH_INTERNAL_FRONTMATTER_KEYS` and BOTH filter sites and every test still passed. Reason: `profiles` is a *named* field on both frontmatter structs, so serde routes it there and `Skill::extra` can never hold it. What actually kept `profiles` out of the deployed file was the absence of the field on the deploy struct — not my new code. My "profiles absent after format" test was passing for the wrong reason.

    Added `test_format_skill_md_drops_internal_keys_planted_in_extra`, which hand-builds a `Skill` with the internal key planted directly in `extra` — the only route that reaches the filter. Verified it is load-bearing: removed the filter and it went RED with `profiles: - kanban` leaking into the output, then restored.

    ## Second real finding — my duplicate-key comment was factually wrong

    I wrote "serde_yaml_ng rejects duplicate mapping keys". It does not. `DuplicateKeyError` lives only in the `Deserialize` impl for `Mapping`; it never runs on a derived struct. What actually caught a duplicated *modeled* key was `serde_derive`'s `duplicate_field` error, which it does emit on the flatten path.

    Worse, the `frontmatter_map` test helper deserialized into `BTreeMap<String, Value>`, which keeps the last value silently — so all three key-comparison tests were blind to duplicates, the exact `#[serde(flatten)]` hazard the card told me to prove I avoided.

    Changed both copies of `frontmatter_map` (skills + mirdan) to return `serde_yaml_ng::Mapping`, which makes duplicate rejection real, and corrected the comment to attribute each check to the right layer.

    ## Confirmed: the renamed `allowed-tools` cannot double-write

    `deserialize_field_identifier` matches the serde name and only falls through to the catch-all when nothing matches. The rename is safe, and the occurrence-count assertions are genuine, not vacuous.

    ## Other fixes

    - Both tests now read `SAH_INTERNAL_FRONTMATTER_KEYS` instead of hardcoding `"profiles"`. Before this, adding a second internal key would have made the mirdan test FAIL — it would have asserted the new internal key survived deploy, which by design it must not. "Single source of truth" was a claim the tests contradicted.
    - Re-exported the const from the crate root, consistent with the other `skill::` items.
    - Documented on `Skill::extra` that unmodeled keys are copied verbatim and are NOT Liquid-rendered. Only the body and `metadata` values are. A `{{version}}` inside `hooks:` deploys literally. Newly reachable, since before this change unmodeled keys never reached the deployed file at all.
    - Filled the `format_skill_md` doc gap — it still described only the 8 modeled fields.

    ## Deferred to a new card

    ^ksys4z5 — the AGENT.md path has the identical defect. `Agent::to_agent_md` rebuilds frontmatter from a closed struct with no catch-all, and `install_profile_agents` calls it on every init. I verified this directly. **This card's own description is wrong** where it says "Agents are not affected. They deploy verbatim as symlinks" — `to_agent_md` is a real re-serialize path. Not biting yet (the 8 builtin AGENT.md files use only modeled keys). That function also concatenates YAML by hand with no escaping, which the new card covers too.

    ## Left alone deliberately

    The reviewer wanted the duplicated `frontmatter_map` helper hoisted into one shared place. The two copies live in different crates and `split_frontmatter` is private, so sharing would mean making a test-only helper public across a crate boundary — worse than two small copies. Both now carry the same `Mapping` semantics.

    Also kept the loader-side `retain`, which is defensive rather than currently load-bearing (serde already routes `profiles` to its named field). The card requires one constant that both sides read, and the retain keeps the invariant true if that named field is ever reworked. Its comment already says exactly this.

    ## Verification after the revisions

    `cargo nextest run -E 'rdeps(swissarmyhammer-skills)'` — 10081 run, 10080 pass. `cargo fmt` and `cargo clippy --workspace --all-targets -- -D warnings` clean. Rebuilt `sah`, re-ran a real `sah init project` in a temp sandbox: the deployed `finish/SKILL.md` carries the full `hooks:` Stop block, zero files under `.skills/` contain `profiles:`, and `double-check` keeps `background: false`. Deploy is idempotent — two consecutive inits produce byte-identical output.

    Still the one pre-existing failure, `work_the_card_skills_render_record_progress_guidance`, proven unrelated by reverting the whole change.
  timestamp: 2026-07-31T10:10:06.088834+00:00
position_column: doing
position_ordinal: '8280'
title: Skill install strips unknown frontmatter (ralph Stop hook is lost)
---
Skill install is a lossy parse-then-rebuild. It keeps an allowlist of 9 frontmatter fields and discards every other key without a warning.

## Symptom

`builtin/skills/finish/SKILL.md:12-16` declares a skill-scoped Stop hook:

```yaml
hooks:
  Stop:
    - hooks:
        - type: command
          command: "sah tool ralph ralph check --"
```

The deployed file has no `hooks` key:

```
$ awk '/^---$/{n++; if(n==2) exit} {print}' ~/.skills/finish/SKILL.md
---
name: finish
description: ...
license: MIT OR Apache-2.0
compatibility: Requires the `kanban` and `ralph` MCP tools plus a Stop-hook-capable harness.
metadata:
  author: swissarmyhammer
  version: 0.17.0
```

`~/.claude/skills/finish` is a symlink to `~/.skills/finish`, so Claude Code reads the stripped file. Result: `/finish` calls `set ralph`, but no Stop hook exists to block the stop. The loop dies between iterations.

`hooks` is an official Claude Code skill frontmatter field. See https://code.claude.com/docs/en/skills#frontmatter-reference and https://code.claude.com/docs/en/hooks#hooks-in-skills-and-agents. Skill-scoped hooks are active only while the skill runs, which is the correct behavior for `/finish`.

## Cause

Both sides of the round-trip use a closed struct:

- `crates/swissarmyhammer-skills/src/skill_loader.rs:11-28` — `SkillFrontmatter` names 9 fields. Serde discards unknown keys. There is no `deny_unknown_fields`, so no error is raised.
- `crates/swissarmyhammer-skills/src/deploy.rs:27-44` — `format_skill_md` writes only 8 fields. A dropped key cannot come back.

## Size of the gap

SAH carries: `name` `description` `license` `compatibility` `context` `agent` `metadata` `allowed-tools` `profiles`.

Claude Code also supports these 12, all dropped today: `when_to_use` `argument-hint` `arguments` `disable-model-invocation` `user-invocable` `disallowed-tools` `model` `effort` `background` `hooks` `paths` `shell`.

Only `finish` uses `hooks` now, so only the ralph hook is broken. The other 11 fail as soon as anyone writes them.

Agents are not affected. They deploy verbatim as symlinks into `~/.agents/<name>/`, so keys such as `skills:` and `disallowed-tools:` survive. That asymmetry is the defect.

## Required change

Make the round-trip lossless. Do not add a `hooks` field alone — that fixes one symptom and leaves 11.

1. In `skill_loader.rs`, capture unknown keys with `#[serde(flatten)] extra: BTreeMap<String, serde_yaml_ng::Value>`.
2. Carry `extra` on `Skill` (`crates/swissarmyhammer-skills/src/skill.rs`).
3. In `deploy.rs`, flatten `extra` back out on serialize.
4. Subtract the SAH-internal keys. `profiles` must stay out of the deployed file. Use one shared named constant that both the parse and serialize sides read, so the internal set has a single source of truth.

## Acceptance

- Round-trip test: parse a SKILL.md that carries `hooks`, `model`, `paths`, and `disable-model-invocation`; format it; assert every key survives. This test must fail before the change.
- Test that `profiles` is present after parse and absent after format.
- After `sah init`, `~/.skills/finish/SKILL.md` contains the `hooks` block.

Note: the hook command itself is also broken, in a separate card — `ralph` is not registered in the CLI tool registry, so `sah tool ralph ralph check --` fails. Both cards are needed before the Stop hook works. #bug #init #ralph