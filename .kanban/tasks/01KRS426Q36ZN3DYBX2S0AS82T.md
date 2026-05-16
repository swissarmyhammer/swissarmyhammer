---
assignees:
- claude-code
position_column: todo
position_ordinal: '9180'
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
- [ ] `src/lib/slugify.parity.node.test.ts` resolves `CORPUS_PATH` to the `crates/swissarmyhammer-common` location.
- [ ] `src/test/integration-commands.ts` resolves `KANBAN_BIN`, `DEFINITIONS_DIR`, and `BUILTIN_ENTITIES_DIR` to their correct post-move locations (`target/` at repo root, `crates/swissarmyhammer-kanban/builtin/*`).
- [ ] `npm test` in `apps/kanban-app/ui` is fully green with zero failures.

## Tests
- [ ] `npm test` (`tsc --noEmit && vitest run`) in `apps/kanban-app/ui` passes with 0 failed (currently 3 suites / 2+ tests failing). #test-failure