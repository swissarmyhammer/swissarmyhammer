---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kwc97phkpp5xj05gt23yhv8y
  text: |-
    Implemented. Changes:
    - RuleSet gains `manifest_body: String` + `manifest_body()` accessor (types.rs).
    - parse_ruleset_directory (parser.rs) captures the VALIDATOR.md body via the existing extract_frontmatter, trimmed.
    - render_validator_suffix (fleet.rs) emits a new render_validator_guidance("## Guidance") block AFTER ## Mandate and BEFORE ## Rules. Empty body emits nothing, so the render stays byte-stable for body-less validators and the fork-prefix reuse contract holds. render_fleet_prompt covers the monolithic fallback via the same helper. render_run_prime untouched.
    - Updated the 4 RuleSet-literal test fixtures.

    Embedded-builtin path: production load_builtins → loader.load_rulesets_directory → parse_ruleset_directory, so the body flows through the same single parse path the on-disk loader uses. The mirdan installer embed (crates/mirdan/src/builtin_validators.rs) stores raw file content verbatim, so it already carries the body and needed no change. Added builtin loader test asserting duplication/data-driven carry the "does not apply to test code" body.

    Tests added: parser captures body + empty-body; builtin rulesets carry body; render_validator_suffix ordering (mandate<body<rules); empty-body omits guidance; monolithic prompt carries body. RED confirmed by reverting the emit (2 renderer tests fail).

    Verify: cargo nextest run -p swissarmyhammer-validators = 305 passed; clippy clean on -validators and -tools.
  timestamp: 2026-06-30T12:49:30.675839+00:00
- actor: claude-code
  id: 01kwc9h3e8mqy88zjw78fy49sr
  text: 'Orchestrator: iter-1 implement landed green (305 passed, clippy clean, double-check PASS) — RuleSet.manifest_body + parser capture + render_validator_suffix `## Guidance` block (after Mandate, before Rules; empty-body byte-identical), embedded path verified. ONE coupling found: the new builtin test asserts duplication/data-driven bodies contain "does not apply to test code" — that''s the user''s editable VALIDATOR.md line, making the test brittle. Decoupling it (assert stable body content instead) so the engine change commits independently of the user''s parallel validator edits. Then path-scoped checkpoint commit (engine files only, NOT the user''s builtin/validators/*.md edits).'
  timestamp: 2026-06-30T12:54:38.792422+00:00
- actor: claude-code
  id: 01kwch8bnfkenz5b5mafdggw0r
  text: |-
    Re-review after rebuild+restart (future-proofing rule removed from ~/.validators).

    VERDICT: the change itself is correct and PROVEN working — the VALIDATOR.md body now reaches the finder. But the review GATE cannot go green here for two reasons that are NOT this task's code, now tracked separately:

    1. fleet.rs (135,248 B) > 131,072 B batch cap → `review sha HEAD~1..HEAD` fails outright; per-file fallback SKIPS fleet.rs entirely. The core change (the `## Guidance` block) goes unreviewed. → ^kf1w8ka (extract test module; 80 KB of 135 KB is tests).

    2. data-driven/duplication still flag TEST code despite the loaded "does not apply to test code" line + the body now reaching the model. The finder said so verbatim: "must use named constants even in tests" (types.rs:507,682,767,787). Prose carve-out reaches qwen and is ignored → structural skip needed → ^sa2v84v.

    Confirmation the future-proofing fix worked: types.rs went 49→21 findings, ALL ~40 "public field → getter" findings gone (root cause was a stale ~/.validators/rust/rules/future-proofing.md orphan — deleted from builtin source but preserved by init; removed manually).

    Marking BLOCKED on ^kf1w8ka + ^sa2v84v. Once both land, re-run the review on this commit; the remaining 21 file-scoped findings (derive PartialEq/Eq, apply_defaults/matches duplication, with_changed_files Vec arg, wrapper delegation) are then a clean fix set with no test-code noise and fleet.rs reviewable.
  timestamp: 2026-06-30T15:09:40.911862+00:00
depends_on:
- 01KWCH6K6ZTPCXV9PBSKF1W8KA
- 01KWCH6ZCQ52ZXRAFGBSA2V84V
position_column: doing
position_ordinal: '8280'
project: local-review
title: Include each validator's VALIDATOR.md body in the per-validator review prompt
---
## Problem

The review fan-out discards the VALIDATOR.md **body** (the markdown after the YAML frontmatter). `render_validator_suffix` (crates/swissarmyhammer-validators/src/review/fleet.rs) builds each validator's prompt from only two things: `## Mandate` = the frontmatter `description` (`RuleSet::description()` → `manifest.description`), and `## Rules` = each `rules/*.md` body (`rule.body`). `RuleSet` (types.rs:602) holds only `manifest` + `rules`; the VALIDATOR.md prose body is never stored or sent.

But the VALIDATOR.md body is **authored guidance** — it explains the validator's intent, scope, and probe evidence, and is the natural home for validator-WIDE direction that applies across all of a validator's rules (e.g. a blanket "this rule does not apply to test code" exclusion). Today that guidance is dead weight: edits to it (e.g. the exclusion lines just added to duplication/VALIDATOR.md and data-driven/VALIDATOR.md) have ZERO effect on review behavior because the model never sees them.

## Change

Fold each validator's VALIDATOR.md body into its per-validator review prompt.

1. **Capture the body.** Add a field to `RuleSet` (e.g. `manifest_body: String`, or onto `RuleSetManifest`) holding the VALIDATOR.md markdown body — everything after the closing `---` of the frontmatter, trimmed. Populate it in `parse_ruleset_directory` (parser.rs).
2. **Embedded-builtin path too.** Production loads builtin validators EMBEDDED in the binary (build-time include of builtin/validators), not from disk. The body capture MUST flow through that embed/codegen path as well, not only the on-disk `ValidatorLoader`. Verify both paths carry the body (there are loader tests in builtin/mod.rs that exercise the embedded set).
3. **Emit it in the prompt.** In `render_validator_suffix` (fleet.rs), emit the captured body as a validator-level guidance block immediately after `## Mandate` (the description) and before `## Rules`, trimmed, so it is shared by every rule in that validator's fan-out. The monolithic fallback `render_fleet_prompt` calls `render_validator_suffix`, so it's covered automatically. The shared `render_run_prime` carries NO validator text — leave it unchanged. Keep the render a pure, byte-stable function of its inputs (the fork-prefix reuse contract depends on it).

## Tests

- Parser: a VALIDATOR.md with a distinctive body line is parsed with that line captured on the RuleSet (and empty body → empty, no panic).
- Embedded builtin: the embedded `duplication`/`data-driven` rulesets carry their VALIDATOR.md body (assert the body field is non-empty / contains a known phrase).
- `render_validator_suffix`: given a RuleSet whose body contains `"does not apply to test code"`, the rendered suffix CONTAINS that line, positioned after the Mandate and before the Rules. Assert ordering.
- Degraded/monolithic `render_fleet_prompt` also contains the body (same render path).

## Acceptance criteria

- A validator's VALIDATOR.md body text appears verbatim in its review prompt (warm-fork suffix AND monolithic fallback), once per validator, after Mandate, before Rules.
- Works on both the on-disk loader and the embedded-builtin path.
- `render_run_prime` unchanged (no validator text in the shared prime).
- `cargo nextest run -p swissarmyhammer-validators` green; `cargo clippy -p swissarmyhammer-validators --all-targets` zero warnings. (And `-p swissarmyhammer-tools` if the embedded path touches it.)

## Note / motivation

Discovered while trying to make a blanket "does not apply to test code" exclusion on the `duplication` and `data-driven` validators actually take effect — the exclusion was placed in the VALIDATOR.md body, which is currently inert. This change makes VALIDATOR.md-body guidance load-bearing. (Prose exclusions are still only as reliable as the finder model honors them — a separate, structural test-code drop may still be wanted — but first the body must at least reach the model.)