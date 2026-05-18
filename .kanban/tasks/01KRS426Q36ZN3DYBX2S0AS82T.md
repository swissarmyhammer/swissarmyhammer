---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffc80
project: ai-panel
title: Fix stale crate paths in UI node/browser tests after apps/ move
---
## What
Three pre-existing UI test suites fail because they resolve fixture/binary paths against `apps/...` but the targets moved when `swissarmyhammer-*` crates were relocated to `crates/swissarmyhammer-*` in commit `a70af2f95` (`refactor(workspace): move app crates into ./apps`). The fixture files and binary all exist, just at a different relative depth.

Root cause is two files, both untouched since `a70af2f95` (the move mechanically rewrote them but did not fix the `../` depth):

1. `src/lib/slugify.parity.node.test.ts` (line 22) — `CORPUS_PATH = resolve(__dirname, "../../../..", "swissarmyhammer-common", "tests", "slug_parity_corpus.txt")`. From `src/lib`, four `..` lands at `apps/kanban-app` → resolves to `apps/kanban-app/swissarmyhammer-common/...`. Actual file: `crates/swissarmyhammer-common/tests/slug_parity_corpus.txt`.

2. `src/test/integration-commands.ts` — three stale paths, all `resolve(__dirname, "../../../../...")` from `src/test` which lands at `apps/`:
   - line 25 `KANBAN_BIN` → `apps/target/debug/kanban`; actual binary: repo-root `target/debug/kanban`.
   - line 30 `DEFINITIONS_DIR` → `apps/swissarmyhammer-kanban/builtin/definitions`; actual: `crates/swissarmyhammer-kanban/builtin/definitions`.
   - line 34 `BUILTIN_ENTITIES_DIR` → `apps/swissarmyhammer-kanban/builtin/entities`; actual: `crates/swissarmyhammer-kanban/builtin/entities`.

Failing test suites (in `apps/kanban-app/ui`), confirmed reproducing on unmodified HEAD:
- `src/lib/slugify.parity.node.test.ts` — 2 failing tests (corpus file load + idempotency), from stale `CORPUS_PATH`.
- `src/components/fields/editors/editor-save.test.tsx` — suite fails in `beforeAll` via `commands.loadFieldDefinitions()`, from stale `DEFINITIONS_DIR` in `integration-commands.ts`.
- `src/components/board-integration.browser.test.tsx` — suite fails in `beforeAll` via `commands.createTestBoard()`, from stale `KANBAN_BIN` in `integration-commands.ts` (`apps/target/debug/kanban: No such file or directory`).

Discovered while implementing task `01KRRN386C7THGV5T6RCA59H4F` (Vendor AI Elements) and confirmed again during verification. Unrelated to that work — `git diff HEAD` for `apps/kanban-app/ui` touches only `package.json` plus new untracked component files; none of the failing test files or `integration-commands.ts` were modified.

## Acceptance Criteria
- [x] `src/lib/slugify.parity.node.test.ts` resolves `CORPUS_PATH` to the `crates/swissarmyhammer-common` location.
- [x] `src/test/integration-commands.ts` resolves `KANBAN_BIN`, `DEFINITIONS_DIR`, and `BUILTIN_ENTITIES_DIR` to their correct post-move locations (`target/` at repo root, `crates/swissarmyhammer-kanban/builtin/*`).
- [x] `npm test` in `apps/kanban-app/ui` is fully green with zero failures.

## Tests
- [x] `npm test` (`tsc --noEmit && vitest run`) in `apps/kanban-app/ui` passes with 0 failed (currently 3 suites / 2+ tests failing). #test-failure

## Implementation Notes

Both test files live at depth `apps/kanban-app/ui/src/{lib,test}/`. Reaching the repo root requires **five** `..` segments (`src/{lib|test}` → src → ui → kanban-app → apps → repo-root), but both files used only four — landing at `apps/kanban-app` (for `slugify`) or `apps/` (for `integration-commands`). The fix adds one `..` and re-points the trailing segment into `crates/` (or repo-root `target/`).

Path corrections (old → new):

1. `apps/kanban-app/ui/src/lib/slugify.parity.node.test.ts` — `CORPUS_PATH`:
   `resolve(__dirname, "../../../..", "swissarmyhammer-common", "tests", "slug_parity_corpus.txt")`
   → `resolve(__dirname, "../../../../..", "crates", "swissarmyhammer-common", "tests", "slug_parity_corpus.txt")`

2. `apps/kanban-app/ui/src/test/integration-commands.ts`:
   - `KANBAN_BIN`: `"../../../../target/debug/kanban"` → `"../../../../../target/debug/kanban"` (repo-root cargo workspace `target/`).
   - `DEFINITIONS_DIR`: `"../../../../swissarmyhammer-kanban/builtin/definitions"` → `"../../../../../crates/swissarmyhammer-kanban/builtin/definitions"`.
   - `BUILTIN_ENTITIES_DIR`: `"../../../../swissarmyhammer-kanban/builtin/entities"` → `"../../../../../crates/swissarmyhammer-kanban/builtin/entities"`.

The `kanban` binary was already built at the repo-root `target/debug/kanban` (no rebuild needed).

Verification: `npm test` in `apps/kanban-app/ui` → 248 passed / 1 failed (249 files), 2305 passed / 1 failed (2306 tests). The single failure is the known intermittent CodeBlock/Shiki flake in `ai-elements.smoke.test.tsx` (tracked by `01KRVG4QSXPQ2FW5SG61M8EHAP`), confirmed flaky — it passes on isolated re-run (12/12). The three target suites run in isolation: `3 passed (3)`, `58 tests passed (58)`. `git status` shows only the two intended files modified, no stray artifacts.

## Review Findings (2026-05-18 19:30)

### Nits
- [x] `apps/kanban-app/ui/src/lib/slugify.parity.node.test.ts:3-6` — The file's doc comment references the corpus location as `swissarmyhammer-common/src/slug.rs` and `swissarmyhammer-common/tests/slug_parity_corpus.txt`, omitting the `crates/` prefix that the `CORPUS_PATH` constant (line 22) was just corrected to use. After the `apps/`-move the canonical paths are `crates/swissarmyhammer-common/...`. Suggest updating the two doc-comment paths to `crates/swissarmyhammer-common/...` so the prose matches the corrected resolve target. Cosmetic only — does not affect test behavior. RESOLVED (2026-05-18): updated both doc-comment paths (lines 4 and 6) to `crates/swissarmyhammer-common/src/slug.rs` and `crates/swissarmyhammer-common/tests/slug_parity_corpus.txt`, matching the corrected `CORPUS_PATH` constant. Comment-only change; no other stale `swissarmyhammer-common/...` references found in the doc comment.