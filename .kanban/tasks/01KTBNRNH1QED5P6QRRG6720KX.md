---
assignees:
- claude-code
depends_on:
- 01KTBNNTCCVS81QZV4CFQZV4X1
- 01KTBNQZFX33J2QA6E99HE0M5S
- 01KTBNGHCH7B3J3DVF9CXPADJ1
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff80
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
- [x] E2E exercises real scope → fan-out → guard → verify → synthesize through the registered tool against a real temp git repo + code_context index, across `review working`/`sha`/`file`.
- [x] Items 1–5 reported with correct validator + severity; item 6 refuted by the agent; item 7 refuted by the guard (no agent spent on it); the language item (rust) flagged by the `rust` validator.
- [x] The skill-side write path lands the report on a temp kanban task in the dated GFM format.
- [x] Runs in CI without a live model (deterministic agent), asserting pipeline behavior, not hardcoded output strings (no fixture-only shortcut).

## Tests
- [x] `crates/swissarmyhammer-tools/tests/integration/review_e2e.rs` green.
- [x] `cargo test -p swissarmyhammer-tools review_e2e` (4 passed) and `cargo build --workspace --tests` (exit 0) green.

## Workflow
- Used `/tdd`: stood up the planted-defect fixture + assertions first, then proved RED via a deliberate guard break (item 7 surfaced as a blocker), then GREEN with the guard restored. Mirrors `semantic_search_e2e.rs` for the real-pipeline structure. Drives the production tool path end to end (registered `ReviewTool` → `run_review_request` → real git diff + real scope + real probes + guard + verify + synthesize); only the agent (scripted playback) and embedder (mock) and index *contents* (deterministically seeded into the real on-disk schema) are deterministic — the seam the `review_op` tests already use.

## Implementation notes
- New file: `crates/swissarmyhammer-tools/tests/integration/review_e2e.rs` (+ one `mod review_e2e;` line in `tests/integration/mod.rs`). No production code changed; `apps/swissarmyhammer-cli` server wiring and `builtin/validators/**` untouched.
- Four planted `.rs` files (≤ default batch size 4) so each builtin validator (`duplication`/`reuse`/`data-driven`/`dead-code`/`no-secrets`/`rust`) gets one fan-out batch and the scripted agent fires once per validator. Items 4 (dead orphan) and 7 (claimed-dead-but-called) live in *separate* files so each file's `callers` fact attaches to only its own symbol — the guard refutes 7 (seeded `lsp_call_edge`) and passes 4 (no edge) deterministically.
- On-disk index built at `<repo>/.code-context/index.db` with the production `create_schema`, seeded so `find_duplicates`/`search_code`/`get_callgraph` hit deterministically (mock embedder, no 600 MB model).
- Scripted ACP agent matches each prompt on ALL of a needle set (validator+file for fan-out, claim for verify), and is shaped like a real `AcpAgentHandle` (streams onto the backend broadcast that `notification_rx` subscribes to, AND bridges onto the connection) so it exercises the production single-path notification collection.
- The `review working` test asserts counts: 3 blockers (1,4,5), 3 warnings (2,3,8), 6 confirmed, 2 refuted — and that the two refuted claims are absent from the rendered GFM. `sha`/`file` share the dispatch→driver path. The kanban-write test drives a real file-backed board (`InitBoard`/`AddTask`/`GetTask`) and asserts the engine `markdown` lands verbatim.

## Pre-existing unrelated failure (NOT introduced here)
- `integration::skill_e2e::test_skill_test_returns_body_content` fails on this branch independently of this change (verified by removing review_e2e.rs + the mod line and re-running: still 0 passed / 1 failed). It asserts the deployed `test` skill body contains "tester"; the deployed skill content has drifted. Out of scope for this task (different skill, file untouched here).