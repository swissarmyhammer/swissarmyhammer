---
position_column: todo
position_ordinal: ff8280
title: Stale @tauri-apps/api import/module-resolution errors in browser vitest project (~7 files)
---
## What

During the `#ui` batch (2026-06-24/25), runs of the `apps/kanban-app/ui` **browser** vitest project surfaced ~7 test files failing to LOAD with stale `@tauri-apps/api` import / module-resolution errors (distinct from the assertion failures tracked separately). These look like a Vite dep-optimization (`node_modules/.vite`) cache artifact or an unpinned-transitive-dep issue rather than a source bug — but they cause real red in the suite and need to be ruled out / resolved so the browser suite is trustworthy.

Relevant context: this repo's `apps/kanban-app/ui` lockfile is gitignored and CI uses `npm install`, so freshly-published transitive deps can break a previously-green build with no code change (this is why radix-ui was pinned). A stale `@tauri-apps/api` optimize-deps cache is the most likely cause.

## Acceptance Criteria
- [ ] Identify the exact ~7 failing files and the precise import/module error message (re-run the browser suite to capture them).
- [ ] Determine root cause: stale Vite optimize-deps cache vs an actual unpinned/incompatible `@tauri-apps/api` version vs a real import path bug.
- [ ] Resolve it: e.g. clear `node_modules/.vite` + clean `npm install`, and/or pin the offending dep, and/or fix the import — and document which it was so it doesn't recur.
- [ ] After the fix, those ~7 files LOAD and run (their own assertions may then pass or fall under the separate assertion-failures task).

## Tests
- [ ] Repro: `cd apps/kanban-app/ui && npx vitest run --project browser 2>&1 | grep -iE 'tauri|failed to (load|resolve|import)'` — capture the failing files + errors.
- [ ] After resolving: the same command shows no `@tauri-apps/api` resolution errors; the previously-erroring files load.

## Workflow
- Start by ruling out the transient cache: `rm -rf apps/kanban-app/ui/node_modules/.vite` then re-run. If that fixes it, document it as a cache artifact and consider a stable mitigation (e.g. an `optimizeDeps` entry) rather than a code change. If it's a real dep/version problem, pin it. This is investigation-led — do not write production code blindly. #test-failure #ui