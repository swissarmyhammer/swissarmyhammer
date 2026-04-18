---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffe280
project: spatial-nav
title: Investigate and fix board-integration.browser.test.tsx
---
## What

Multiple cards in the spatial-nav project noted "only pre-existing board-integration.browser.test.tsx failure" and walked past it. Nobody confirmed whether the failure is truly pre-existing or whether the spatial-nav refactor contributed to it.

### What to find out

- [x] Run `kanban-app/ui/src/components/board-integration.browser.test.tsx` — capture the full failure output
- [x] `git blame` / `git log` the test file to see when it last passed
- [x] Determine if the failure is:
  - Truly pre-existing (file out of date, unrelated change) → file a separate task and skip/xit the test with a TODO link
  - Caused by the spatial-nav refactor (missing FocusLayer wrapper, removed claimWhen, ulid mock, etc.) → fix it here

### Likely suspects (spatial-nav-related)

- Missing `FocusLayer` wrapper around the test subject (FocusScope now requires one for spatial registration — but tolerates its absence via `useFocusLayerKey()` returning null)
- Missing `transformCallback` in the `@tauri-apps/api/core` mock
- Missing `ulid` mock (real ulid produces different values per run, breaking snapshots)
- Missing `registerClaim`/`unregisterClaim` in the entity-focus mock
- `claimWhen` references in the test that no longer match the new `navOverride` prop

## Root Cause (resolved)

**The failure was NOT caused by the spatial-nav refactor.** It was an infrastructure gap: the test's `beforeAll` hook shells out to the `kanban` CLI binary at `target/debug/kanban`, but nothing in the test harness built that binary first. If a developer runs `npx vitest run` on a fresh checkout — or after `cargo clean` — `beforeAll` fails with:

```
/bin/sh: /.../target/debug/kanban: No such file or directory
```

which skips all 9 tests and looks like a test failure.

Verification:
- `git log` shows zero spatial-nav commits touched this file; last substantive change was before the spatial-nav work.
- None of the suspected spatial-nav hooks (`FocusLayer`, `transformCallback`, `ulid`, `registerClaim`, `claimWhen`) are referenced by this test at all — it mocks `@tauri-apps/api/core` at a coarser level and never touches focus navigation.
- After building the binary (`cargo build --bin kanban`) all 9 tests pass with no changes to the test itself.

## Fix

Made the test self-bootstrapping. Added `ensureKanbanBinary()` in `kanban-app/ui/src/test/integration-commands.ts` — when `target/debug/kanban` is missing, the browser-command layer runs `cargo build --bin kanban` automatically (once per vitest process, cached after the first call). After the build, tests execute normally. Subsequent invocations short-circuit via the cached flag + filesystem check.

Verified by deleting the binary, running `npx vitest run src/components/board-integration.browser.test.tsx` — the auto-build fires, then all 9 tests pass.

## Acceptance Criteria

- [x] Root cause identified and documented
- [x] Test either passes or is explicitly skipped with a link to a follow-up task describing the real bug