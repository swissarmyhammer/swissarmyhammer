---
assignees:
- claude-code
depends_on:
- 01KTBNNTCCVS81QZV4CFQZV4X1
- 01KTBNQZFX33J2QA6E99HE0M5S
- 01KTBNGHCH7B3J3DVF9CXPADJ1
position_column: todo
position_ordinal: '9180'
project: local-review
title: 'End-to-end: real diff → review tool → confirmed duplication/dead-code/hardcode findings'
---
## What
The acceptance test for the whole feature, following the repo's reference real-pipeline pattern (`tests/integration/semantic_search_e2e.rs`: real indexer → real query → real result). It must prove the new system catches exactly the failures the user called out, filters false positives, and records results correctly.

Build a temp git repo fixture with a diff that plants, on purpose:
1. a **copy-pasted block** duplicating an existing function → caught by **`duplication`** (blocker, `duplicates` probe).
2. a **new helper that reimplements an existing shared util** instead of calling it → caught by **`reuse`** (warning, `similar` probe).
3. a **hardcoded if-chain over a known set** that should be a table → **`data-driven`** (warning).
4. a **new function with zero inbound callers** (not entry/export/test) → **`dead-code`** (blocker, `callers` probe).
5. a **planted secret** → **`no-secrets`** (blocker).
6. an **agent red herring** — looks buggy, is correct → must be **refuted by the adversarial verifier** and NOT reported.
7. a **guard red herring** — a function the scripted agent *claims* is dead but which HAS a caller → must be **auto-refuted by the deterministic probe guard** (the `callers` fact contradicts the claim) WITHOUT a verifier agent. Proves the two-layer verify.
8. (optional, cheap) a **language-specific issue** (e.g. a Rust idiom) → caught by the `rust` language validator, proving language validators fire end-to-end.

Drive the full path: index the fixture (code_context), then run the tool with a deterministic/playback agent (CI needs no live model — but assert pipeline behavior, not canned strings):
- `review working` (uncommitted), `review sha <range>` (committed), and `review file <glob>` — assert each returns a `ReviewReport` that:
  - contains confirmed findings for items 1–5 tagged to the correct validator + severity,
  - does NOT contain item 6 (agent-refuted) or item 7 (guard-refuted),
  - reports a non-zero `refuted` count distinguishing guard vs agent,
  - renders in the existing dated GFM checklist format.
- Skill path: assert the report lands on a temp kanban task (task-mode append / range-mode tracking task) in the documented format.

## Acceptance Criteria
- [ ] E2E exercises real scope → fan-out → guard → verify → synthesize through the registered tool against a real temp git repo + code_context index, across `review working`/`sha`/`file`.
- [ ] Items 1–5 reported with correct validator + severity; item 6 refuted by the agent; item 7 refuted by the guard (no agent spent on it); the language item (if included) flagged by the `rust` validator.
- [ ] The skill-side write path lands the report on a temp kanban task in the dated GFM format.
- [ ] Runs in CI without a live model (deterministic agent), asserting pipeline behavior, not hardcoded output strings (no fixture-only shortcut).

## Tests
- [ ] `crates/swissarmyhammer-tools/tests/integration/review_e2e.rs` (or the established integration location) green.
- [ ] `cargo test -p swissarmyhammer-tools review_e2e` and `cargo test --workspace` green.

## Workflow
- Use `/tdd` — stand up the planted-defect fixture and assertions first. Mirror `semantic_search_e2e.rs` for the real-indexer→real-tool structure. Guard against the fixture-only anti-pattern: drive the production tool path end to end. Depends on the tool, the skill, and the hook teardown (no stale hook path interferes).