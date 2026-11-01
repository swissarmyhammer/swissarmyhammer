---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m0nfm8e28fg6e0n6hk55n7cj
  text: |-
    ## Superseded by ^m7ynz9c — do not work this card as written

    ^m7ynz9c ("swift partial: prefer `swift package format --lint` when the Airbnb plugin is a dependency") rewrites the SAME Format/Lint bullet in `builtin/_partials/project-types/swift.md`. Both halves of this card landed there:

    - **Checkbox 1 (the Format bullet).** The partial now names each tool beside the config file it alone reads (`swift format` → `.swift-format`, `swiftformat` → `.swiftformat`, `swiftlint` → `.swiftlint.yml`), states plainly that the first two are DIFFERENT programs, says to use the tool whose config file the repo already holds and to obey it, forbids writing a config file as a side effect of formatting, forbids style flags that disagree with a config already there, and states the tool defaults apply when no config exists.
    - **Checkbox 2 (the content regression test).** Landed as `swift_partial_separates_the_two_formatters_and_documents_the_airbnb_plugin` in `crates/swissarmyhammer-templating/src/resolver.rs`. Verified RED against the old partial, GREEN against the new one.

    Two departures from this card's text, both driven by measurement:

    1. **The test reads the SERVED content, not the file on disk.** This card proposed a test in `crates/swissarmyhammer-project-detection/src/types.rs` reading the `.md` through `CARGO_MANIFEST_DIR`. That crate holds only the partial's PATH string; it never sees the content. `swissarmyhammer-templating` embeds the partial at build time via `get_builtin_partials()` and `PromptResolver::load_builtin_partials` registers it as `_partials/project-types/swift` — the same path `partial!("swift")` builds. Asserting there measures the text an agent actually receives. `spec_partial_matches_key` is untouched and still passes.

    2. **"`swiftformat .` honors `.swiftformat` likewise" is not sufficient advice.** Measured on swiftformat 0.62.1, the four options our shipped `idioms-swift` validator pins all default the other way:

       | option | default | what `idioms-swift` pins |
       |---|---|---|
       | `--pattern-let` | `hoist` | `inline` |
       | `--short-optionals` | `preserve-struct-inits` | `always` |
       | `--single-line-for-each` | `ignore` | `convert` |
       | `--guard-like-if-statements` | `preserve` | `convert` |

       A bare `swiftformat .` rewrote `if case .some(let inner) = value` into `if case let .some(inner) = value`, and `idioms-swift` then reported that exact line. So the partial states the four options, and states that a repo `.swiftformat` already carrying them makes a bare run correct.

    Config discovery was measured, not assumed: a root `.swift-format` reached a file two directories down (7-space indentation applied, no `--configuration` flag), and a root `.swiftformat` did the same.

    This card can be archived.
  timestamp: 2026-08-22T19:37:10.850616+00:00
- actor: claude-code
  id: 01m6bzrtnyyv83bpgg728qqvwn
  text: |-
    ### implement — changed
    - evidence: builtin/_partials/project-types/swift.md (one sentence: tool defaults apply when no .swift-format exists). project-detection holds no swiftformat branch or test (nothing to remove); honoring .swift-format and no-config-written already in partial. No markdown-text test added per scope guard. cargo nextest -p swissarmyhammer-project-detection: 35 run, 35 passed, 0 skipped.
    - next: review
    step: implement
    outcome: changed
    task: ^7fgdenq
  timestamp: 2026-11-01T16:41:04.446687+00:00
- actor: claude-code
  id: 01m6bzwas7rm3mm878dc8nyvr2
  text: |-
    ### test — green
    - evidence: cargo build --workspace --all-targets — exit 0, 0 warnings; cargo nextest run -p swissarmyhammer-project-detection --no-fail-fast — 35 run, 35 passed, 0 failed, 0 skipped
    - next: review
  timestamp: 2026-11-01T16:42:59.239005+00:00
- actor: claude-code
  id: 01m6bzwyrb1q0vm81gdsz7ghah
  text: |-
    ### commit — changed
    - evidence: c259d8361 docs(swift): say tool defaults apply when no .swift-format exists; file builtin/_partials/project-types/swift.md
    - next: review
  timestamp: 2026-11-01T16:43:19.691742+00:00
- actor: claude-code
  id: 01m6bzzhtqjxxk4ryhyar81e28
  text: |-
    ### review — findings
    - evidence: review sha c259d8361~1..c259d8361 matched no validator for builtin/_partials/project-types/swift.md (engine could not look; its 0 findings prove nothing). Hand measurement: 2 findings. (1) swift.md:68 does not forbid ad-hoc style flags, `--configuration` overrides, or editing/regenerating a held .swift-format. (2) crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/swift_guidelines_partial.rs:107 does not assert the new sentence "When the project holds no `.swift-format`, the tool defaults apply." No `.swiftformat` text in swift.md or project-detection (rg exit 1).
    - next: implementer fixes both findings, then re-review.
  timestamp: 2026-11-01T16:44:44.759540+00:00
- actor: claude-code
  id: 01m6c01e2t0tm83wk251vmadpk
  text: |-
    Finding 2 (swift_guidelines_partial.rs test for the defaults sentence) is dropped. Reason: written rule, kanban card ^ga169e "Delete tests that assert on skill text or source text". Another session deletes that test file. Checked off as released; the test file is untouched.

    ### implement — changed
    - evidence: builtin/_partials/project-types/swift.md Formatting paragraph now forbids ad-hoc style flags, `--configuration` overrides, and editing or regenerating a held `.swift-format`. The partial's own commands do not use `--configuration`. `rg -i "swiftformat|airbnb"` on the file: no match (exit 1). Markdown-only change, no cargo tests run. Findings 1 and 2 both checked off in the card.
    - next: /review
  timestamp: 2026-11-01T16:45:46.458842+00:00
- actor: claude-code
  id: 01m6c027vn9xe0arjfxw70vewh
  text: |-
    ### commit — changed
    - evidence: 670807af8 docs(swift): forbid overriding or editing a held .swift-format (builtin/_partials/project-types/swift.md)
    - next: review
  timestamp: 2026-11-01T16:46:12.853693+00:00
- actor: claude-code
  id: 01m6c03xvwh1wk73485qdbd7m7
  text: |-
    ### review — clean
    - evidence: review sha 670807af8~1..670807af8 returned 0 findings, but the engine matched no validator to builtin/_partials/project-types/swift.md (0 files reviewed, 1 not reviewed). The engine cannot look at *.md files, so its zero proves nothing. Hand measurement against the card (with the .swiftformat half superseded by the user's decision):
      - Board `get task` shows both prior Review Findings items as [x]. Item 2 carries the release note "released: user ruling, tests asserting on skill text are removed (^ga169e)". Card ^dga169e ("Delete tests that assert on skill text or source text") is real and is in doing.
      - rg -i "swiftformat|airbnb" matches nothing in builtin/_partials/project-types/swift.md and nothing in crates/swissarmyhammer-project-detection (exit 1).
      - swift.md:68 now says: obey a held .swift-format; do not write one as a side effect; do not pass ad-hoc style flags or --configuration overrides that fight it; do not edit or regenerate a held .swift-format as a side effect; when none exists the tool defaults apply. This closes prior finding 1.
      - The `dump-configuration` sentence is an explicit act, so it does not contradict the no-side-effect rule. The commands at swift.md:60 and :63 match the prose.
      - Frontmatter (title, description, partial: true) is unchanged. The commit touches only swift.md (1 line).
    - next: none. Task moved to done.
  timestamp: 2026-11-01T16:47:08.156640+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffffdd80
title: 'guidelines: swift format instruction — honor an existing .swift-format/.swiftformat config, defaults otherwise'
---
## What

The Swift project guidelines partial — `builtin/_partials/project-types/swift.md`, served through `detect projects` via the `partial!("swift")` entry in `crates/swissarmyhammer-project-detection/src/types.rs` (~line 261) — currently gives one bare formatting line (line 40):

> `- Format: \`swift format -i -r Sources Tests\` (or \`swiftformat .\`) — run before committing`

That says nothing about configuration, so an agent working in someone else's repo can steamroll the project's own style. Update it to make config-honoring explicit:

- [ ] **Replace the line-40 Format bullet** in `builtin/_partials/project-types/swift.md` with guidance to this effect (wording may be polished, substance fixed):
  - If the repo has a `.swift-format` config (Apple's `swift-format`), run `swift format -i -r Sources Tests` — the tool discovers and honors the config automatically (it searches each file's directory and its parents). Never pass ad-hoc style flags or `--configuration` overrides that fight it, and never edit or regenerate the config as a side effect of formatting.
  - If the repo instead has a `.swiftformat` config (Nick Lockwood's SwiftFormat), use `swiftformat .`, which honors it likewise — pick the tool that matches the config file present.
  - If NO formatter config exists, format with the tool defaults (`swift format -i -r Sources Tests`) and do NOT create a config file as a side effect.
- [ ] **Add a content regression test** next to `spec_partial_matches_key` (~line 586) in `crates/swissarmyhammer-project-detection/src/types.rs` `#[cfg(test)]`: read `../../builtin/_partials/project-types/swift.md` via `CARGO_MANIFEST_DIR` (the same relative-root convention `swissarmyhammer-validators/src/builtin/mod.rs` uses for `../../builtin/validators`) and assert the Format guidance (a) mentions honoring an existing `.swift-format` and `.swiftformat`, (b) states defaults apply when no config exists, and (c) forbids creating/overriding a config as a formatting side effect — so the instruction can't silently regress to the bare one-liner.

**Deploy note (not part of this card's code change):** builtin content changes need the usual rebuild + redeploy (`just sah` + `sah init`) before deployed copies serve the new text.

## Acceptance Criteria

- [ ] `builtin/_partials/project-types/swift.md` no longer contains the bare `- Format: \`swift format -i -r Sources Tests\` (or \`swiftformat .\`) — run before committing` line; the replacement covers all three cases (`.swift-format` present → honor it; `.swiftformat` present → `swiftformat .` honors it; neither → tool defaults) and forbids creating or overriding a config as a side effect.
- [ ] The file's frontmatter (`title` / `description` / `partial: true`) and all other sections (ULID guidance, Testing, other Common commands, File locations) are unchanged.
- [ ] The new content regression test fails against the pre-change file and passes after.
- [ ] `spec_partial_matches_key` still passes (partial path untouched).

## Tests

- [ ] New test in `crates/swissarmyhammer-project-detection/src/types.rs` `#[cfg(test)]` asserting the swift partial's Format guidance covers config-honoring, defaults fallback, and the no-config-creation rule.
- [ ] `cargo nextest run -p swissarmyhammer-project-detection` — green.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

## Review Findings (2026-11-01 10:55)

> Scope: `review sha c259d8361~1..c259d8361`. The review engine matched no validator to `builtin/_partials/project-types/swift.md` (0 files reviewed, 1 not reviewed, 0 findings). The engine could not look at this file, so its zero proves nothing. The findings below come from a hand measurement against the card, read as updated by the user's decision (the `.swiftformat` half is superseded; `swift format` from the Swift 6.2+ toolchain is the one formatter).

- [x] `builtin/_partials/project-types/swift.md:68` `hand/acceptance-criteria` — The Formatting paragraph says "Obey a `.swift-format` the project already holds. Do not write one as a side effect of formatting." It does not forbid overriding or editing the config. The card says: "Never pass ad-hoc style flags or `--configuration` overrides that fight it, and never edit or regenerate the config as a side effect of formatting", and the acceptance criterion says the text "forbids creating or overriding a config as a side effect". Add a sentence that forbids ad-hoc style flags, `--configuration` overrides, and editing or regenerating a held `.swift-format` as a side effect of formatting.
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/swift_guidelines_partial.rs:107` `hand/acceptance-criteria` — The commit adds the sentence "When the project holds no `.swift-format`, the tool defaults apply." Test `swift_partial_states_the_one_toolchain_formatter` asserts six phrases and none of them is this sentence. The card requires a test that asserts (b) "states defaults apply when no config exists". The sentence can be deleted and the test stays green. Add that phrase to the asserted list, and add the phrase for the override rule from the first finding. -- released: user ruling, tests asserting on skill text are removed (^ga169e)

Hand measurement that passed: no `.swiftformat` text anywhere in `builtin/_partials/project-types/swift.md` or in `crates/swissarmyhammer-project-detection` (rg exit 1, no match); the bare Format bullet is gone; frontmatter `title`/`description`/`partial: true` equal the original; ULID, Common commands and File locations text equal the original; `.swift-format` is named and read from the file directory or a parent (line 68); `-s` check command present (line 56 section); the no-creation rule is present (line 68).
