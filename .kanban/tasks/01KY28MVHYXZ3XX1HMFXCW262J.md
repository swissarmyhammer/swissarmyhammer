---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01ky2yanrgrztwex7t0bw7tqhy
  text: 'TDD: added test_swift_casing_accepts_both_acronym_spellings in crates/swissarmyhammer-validators/src/builtin/mod.rs, watched it fail RED against the old casing.md content (assertion on "BOTH accepted" missing). Then rewrote the line-10 uniform-acronym bullet in builtin/validators/swift/rules/casing.md and removed the line-12 ID bullet + line-13 prevalence-tiebreaker bullet, replacing all three with the flexible both-spellings-accepted rule plus its two sub-bullets (position rule, single-declaration no-mixing rule). LoRA/OAuth/GraphQL canonical bullet and radarDetector ordinary-word bullet left byte-identical. `cargo nextest run -p swissarmyhammer-validators builtin` — all 28 tests pass (GREEN). cargo fmt + cargo clippy -p swissarmyhammer-validators --all-targets -- -D warnings clean.'
  timestamp: 2026-07-21T18:17:07.600889+00:00
position_column: doing
position_ordinal: '8280'
title: 'validators: swift casing — accept both acronym spellings (URL/Url, ID/Id), never flag conversions between them'
---
## What

Relax the Swift acronym-casing rule in `builtin/validators/swift/rules/casing.md`: both the uniform spelling (`URL`, `ID`, `JSON`) and the capitalized-word spelling (`Url`, `Id`, `Json`) are accepted in every position, and converting between them is never a finding. Rationale (user): the uniform-only rule is too picky to apply to other people's open source — spelling-conversion renames touch every call site, widen fork-merge surface, and change no behavior.

Edits to `builtin/validators/swift/rules/casing.md` (current line numbers):

- [ ] **Rewrite the line-10 uniform-acronym bullet** (currently: "Acronyms and initialisms are cased uniformly … (`Url`, `Json`, `Http`, `deviceId` are all wrong)") and **delete the line-12 ID bullet and line-13 prevalence-tiebreaker bullet**, replacing all three with one flexible rule to this effect (wording may be polished, substance verbatim):

  ```
  - **Acronym spelling is flexible — the uniform form (`URL`, `ID`, `JSON`, `HTTP`)
    and the capitalized-word form (`Url`, `Id`, `Json`, `Http`) are BOTH accepted —
    never flag one toward the other.** `entryID` and `entryId`, `baseURL` and
    `baseUrl`, `schemaJSON` and `schemaJson` are all valid; so are the leading
    lower forms `id`/`idToken`/`urlString`. Do not raise a finding whose only
    substance is converting between the two spellings of the same acronym, in
    either direction, on any declaration — new or pre-existing, public or
    private. Such a rename is always churn: it touches every call site, widens
    fork-merge surface, and changes no behavior. A finding that proposes one is
    a validator error.
    - Position rules still hold: an acronym leading a `lowerCamelCase` name is
      down-cased as a unit (`urlSession`, `idToken` — never `URLSession` as a
      property name).
    - Within a SINGLE declaration's own name, don't mix spellings of the same
      term (`tokenIdToEntryIDMap` is flaggable — pick one spelling inside one
      name). Consistency across different declarations, files, or with
      surrounding code is NOT required and NOT flaggable.
  ```

- [ ] **Keep the canonical mixed-case bullet (line 14) and ordinary-word bullet (line 15) unchanged** — `LoRA`/`OAuth`/`GraphQL`/`gRPC`/`IPv6`/`macOS` keep their canonical spellings and are never flattened; `radarDetector` stays an ordinary word. Also unchanged: the type/member case bullets (lines 8–9) and the no-SCREAMING_SNAKE / no-Hungarian bullets (lines 16–17).
- [ ] **Add a content regression test** alongside the existing builtin-loading tests in `crates/swissarmyhammer-validators/src/builtin/mod.rs` (pattern: `test_test_integrity_homes_no_hard_code`, ~line 231): load builtins via `load_builtins` and assert the swift `casing` ruleset's loaded rule text (a) contains `BOTH accepted`, and (b) no longer contains the retired directives — neither `are all wrong` applied to `Url`/`Json`, nor `DON'T: \`entryId\``, nor the "flag toward the uniform form" tiebreaker phrase — so the relaxation can't be silently reverted or half-applied.

**Deploy note (not part of this card's code change):** `sah init` never refreshes already-deployed validator stores in consuming projects (known fossil behavior) — after this merges, deployed copies (e.g. in the mlx repo) need a redeploy/prune for the relaxed rule to take effect there.

## Acceptance Criteria

- [ ] `builtin/validators/swift/rules/casing.md` presents both acronym spellings as accepted and states that a spelling-conversion rename is a validator error; the old uniform-only directives (`are all wrong` list with `Url`/`Json`, the `DON'T: \`entryId\`` bullet, the flag-toward-uniform tiebreaker) are gone.
- [ ] The single-name no-mixing sub-rule (`tokenIdToEntryIDMap` flaggable) and the leading-position down-casing sub-rule are present in the new text.
- [ ] The canonical mixed-case (`LoRA`, `OAuth`, …) and ordinary-word (`radarDetector`) bullets are byte-identical to before.
- [ ] The swift `casing` ruleset still loads cleanly through `load_builtins` (frontmatter intact — `name: casing`); the new regression test fails on the pre-change content and passes after.

## Tests

- [ ] New test in `crates/swissarmyhammer-validators/src/builtin/mod.rs` `#[cfg(test)]` asserting the loaded swift casing rule text contains `BOTH accepted` and contains none of: an `are all wrong` list naming `Url`/`Json`, `DON'T: \`entryId\``, or the flag-toward-uniform tiebreaker phrase.
- [ ] Existing builtin loader tests (`test_focused_validators_have_clean_manifest_frontmatter` etc.) still pass — frontmatter untouched.
- [ ] `cargo nextest run -p swissarmyhammer-validators builtin` — green.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.