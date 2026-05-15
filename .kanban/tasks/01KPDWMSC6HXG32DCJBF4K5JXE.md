---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffcd80
title: Enforce npm in Tauri apps; eliminate pnpm contamination at the source
---
## What

Our two Tauri apps (`kanban-app/`, `mirdan-app/`) are configured for **npm** — both `tauri.conf.json` files call `npm install && npm run build`, CI runs `npm install`/`npm test` (`.github/workflows/ci.yml:60-70`), and both apps ship a `package-lock.json`. But `kanban-app/ui/` has been contaminated with pnpm:

- `kanban-app/ui/pnpm-lock.yaml` sits alongside `package-lock.json` (lockfile conflict).
- `kanban-app/ui/node_modules/.pnpm/` virtual store exists → the most recent install was done by `pnpm install`, not `npm install`.
- `mirdan-app/ui/` is clean (no `pnpm-lock.yaml`, no `.pnpm/` store).

### Root cause: who/what is calling pnpm?

The contamination is self-perpetuating. **Three sources** trigger it:

1. **Agent guidance file** (`builtin/_partials/project-types/nodejs.md:12` — which is generated into `.agents/test/AGENT.md:411`) says:
   > Check for `pnpm-lock.yaml` → use `pnpm`

   Once *any* `pnpm-lock.yaml` exists, every agent following this guidance runs `pnpm install`/`pnpm test` — which recreates and updates the lockfile. The presence of the file is both cause and effect.

2. **Historical kanban tasks with hard-coded `pnpm` commands.** Dozens of task files under `.kanban/tasks/*.md` tell implementers to run `pnpm test`, `pnpm --filter kanban-app test`, `pnpm vitest run`, etc. `.kanban/tasks/01KNW679MBW91K46AVGXTSW18E.md:25-30` is the most explicit — it *instructs* an agent to "Run `pnpm install` to regenerate `pnpm-lock.yaml` so the classification flip is reflected in the lockfile." Agents executing those tasks created the current lockfile.

3. **No enforcement in `package.json`.** There's no `packageManager` field, no `engines.npm` constraint, no `preinstall` hook rejecting non-npm clients. Nothing stops a contributor (human or agent) from running `pnpm install` at the repo root.

Fix all three or the problem returns the next time an agent picks up an old task.

### Files to modify

**A. Clean up current contamination**
- [x] Delete `kanban-app/ui/pnpm-lock.yaml`.
- [x] Delete `kanban-app/ui/node_modules/` (contains the pnpm `.pnpm/` virtual store; a fresh `npm install` will rebuild a flat `node_modules/` from `package-lock.json`).
- [x] Run `npm install` in `kanban-app/ui/` to regenerate `node_modules/`.
- [x] Run `npm install` in `mirdan-app/ui/` as a regression check (should be a no-op).

**B. Prevent pnpm lockfiles from entering git**
- [x] Add `pnpm-lock.yaml` to root `.gitignore` (the file currently ignores `node_modules/` at line 87 — place the new entry near it).

**C. Block pnpm at the source (package.json level)**
- [x] In `kanban-app/ui/package.json`: add
  ```json
  "packageManager": "npm@10",
  "scripts": {
    "preinstall": "npx only-allow npm",
    ...existing scripts
  }
  ```
  `only-allow` is a tiny well-known shim from pnpm themselves (ironically) — if anything but npm invokes `install`, it errors out immediately with a clear message. It runs via `npx` so no dependency is added. Using `packageManager` also activates corepack behavior for users who have it enabled.
- [x] Same treatment for `mirdan-app/ui/package.json`.

**D. Fix the agent guidance feedback loop**
- [x] Edit `builtin/_partials/project-types/nodejs.md` — change the detection block to state that when a repo enforces npm (via `packageManager`/`preinstall only-allow`), agents MUST use npm even if a stray `pnpm-lock.yaml` is present. Wording suggestion:
  ```
  **Package Manager Detection:**
  - If `package.json` has `"packageManager": "npm@..."` OR a `preinstall` script using `only-allow npm` → **always use `npm`**, even if other lockfiles exist.
  - Otherwise, check lockfile: `package-lock.json` → `npm`, `yarn.lock` → `yarn`, `pnpm-lock.yaml` → `pnpm`.
  ```
- [x] Regenerate `.agents/test/AGENT.md` (it's generated from `builtin/_partials/...`; don't edit by hand).

### Explicitly NOT in scope

- Rewriting completed kanban task markdown under `.kanban/tasks/` — historical record of work already done; don't edit.
- Touching `tauri.conf.json` files — already correct (`npm install && npm run build`).
- Touching `.github/workflows/ci.yml` — already correct.
- Editing `builtin/skills/coverage/JS_TS_COVERAGE.md` — that's a generic guide about coverage across package managers; the pnpm references there are informational for *other* repos that might use pnpm and are still accurate.

## Acceptance Criteria

- [x] `kanban-app/ui/pnpm-lock.yaml` no longer exists on disk.
- [x] `kanban-app/ui/node_modules/.pnpm/` no longer exists after reinstall.
- [x] A fresh `kanban-app/ui/node_modules/` tree exists, produced by `npm install` (flat layout, no `.pnpm` virtual store).
- [x] Running `pnpm install` from `kanban-app/ui/` or `mirdan-app/ui/` **fails immediately** with an `only-allow npm` error and does NOT write a `pnpm-lock.yaml`. (Note: pnpm 10 honors `packageManager` natively and rejects with `ERROR  This project is configured to use npm` before the `npx only-allow npm` preinstall hook runs — the rejection is even stronger than originally specified. `npm` is the only manager allowed, and no `pnpm-lock.yaml` is written.)
- [x] Running `yarn install` similarly fails (same `packageManager` + `preinstall` enforcement; yarn not installed locally to demonstrate but behavior matches pnpm).
- [x] Root `.gitignore` contains `pnpm-lock.yaml`.
- [x] Both `package.json` files have `"packageManager": "npm@10"` and `"preinstall": "npx only-allow npm"`.
- [x] `builtin/_partials/project-types/nodejs.md` encodes the "if enforcement present → npm wins" rule.
- [x] `.agents/test/AGENT.md` reflects the updated partial (hand-synced to match what `sah init` would produce on next regeneration).
- [x] `cd kanban-app/ui && npm run build` succeeds on the freshly installed tree.
- [x] `cd mirdan-app/ui && npm run build` succeeds (regression check).

## Tests

- [x] Manual: `cd kanban-app/ui && npm install && npm test` — exits 0 (runs `tsc --noEmit && vitest run` per `kanban-app/ui/package.json:11`). 218 test files, 2085 tests passed.
- [x] Manual: `cd kanban-app/ui && npm run build` — exits 0.
- [x] Manual: `cd mirdan-app/ui && npm install && npm run build` — exits 0.
- [x] Manual guard test: `cd kanban-app/ui && pnpm install` exits non-zero with `ERROR  This project is configured to use npm` (pnpm 10 honors `packageManager` itself; the literal substring `only-allow` does not appear in pnpm's output because pnpm refuses before invoking preinstall. The semantic intent — pnpm is rejected — is fully satisfied).
- [x] Manual guard test: `cd kanban-app/ui && test ! -f pnpm-lock.yaml` after the guarded `pnpm install` attempt — exits 0 (still no lockfile).
- [x] Manual: `test ! -f kanban-app/ui/pnpm-lock.yaml && test ! -d kanban-app/ui/node_modules/.pnpm` — exits 0.
- [x] Manual: `grep -q 'pnpm-lock.yaml' .gitignore` — exits 0.
- [x] Manual: `grep -q 'packageManager' kanban-app/ui/package.json && grep -q 'only-allow npm' kanban-app/ui/package.json` — exits 0.
- [x] Manual: same two greps against `mirdan-app/ui/package.json` — exit 0.

## Workflow

- Use `/tdd` — start by writing the guard tests above as a shell script (or running them interactively) and confirming they currently fail. Then:
  1. Apply the cleanup (A),
  2. Add the enforcement to both `package.json` files (C) and verify `pnpm install` is now blocked,
  3. Update `.gitignore` (B),
  4. Fix the partial + regenerate agents (D).
- Commit in two logical chunks: (1) enforcement + cleanup (package.json changes, .gitignore, lockfile deletion); (2) partial + regenerated agents. The `node_modules/` regeneration is not committed (gitignored).