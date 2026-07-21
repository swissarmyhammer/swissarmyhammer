---
assignees:
- claude-code
position_column: todo
position_ordinal: b280
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