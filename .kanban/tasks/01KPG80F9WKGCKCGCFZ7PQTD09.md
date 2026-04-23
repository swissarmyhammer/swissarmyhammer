---
assignees:
- claude-code
depends_on:
- 01KPG7ECXR1M5AB3QMJT2CW5CD
- 01KPG7ZGYEKSYBNMZ9F634KKHY
- 01KPG70ZNP0X7WE06SXJVV1N9C
position_column: done
position_ordinal: ffffffffffffffffffffffeb80
project: spatial-nav
title: 'CI: run spatial-nav Rust integration tests + tauri-driver E2E on every PR'
---
## What

Tests that nobody runs silently rot. Once the spatial-nav Rust integration tests (`01KPG7ECXR1M5AB3QMJT2CW5CD`), new state-machine coverage (`01KPG70ZNP0X7WE06SXJVV1N9C`), and the canonical E2E (`01KPG7ZGYEKSYBNMZ9F634KKHY`) exist, wire them into CI so every PR runs them.

### What needs to run

1. **`cargo nextest run -p swissarmyhammer-commands`** (or `-p swissarmyhammer-spatial-nav` if crate extraction lands) — the state-machine unit tests
2. **`cargo nextest run -p kanban-app`** — the Tauri mock_app integration tests + fixture/dump tests
3. **`npm run e2e`** under `kanban-app/` — the tauri-driver WebdriverIO scenario

### CI platform concerns

- **Where does CI run?** Check for existing `.github/workflows/*.yml` or other CI config. If there's a workflow, extend it; if not, this task also owns creating one (flag that for the user before doing so).
- **Webview requirement for E2E.** tauri-driver requires a real display. Linux needs `xvfb-run` wrapper; macOS needs a GUI runner (no headless option). Decide: (a) run the E2E only on one platform and accept the coverage gap, or (b) run on both and eat the runner cost.
- **Caching.** cargo target dir (keyed on Cargo.lock hash), node_modules (keyed on package-lock.json hash), and the tauri-driver binary should all cache across runs.
- **Flake tolerance.** The E2E involves real timing (waitUntil loops). If it flakes more than ~1% of runs, the test is broken — do NOT add retries to mask it. Fix the underlying race.

### Boundary with existing CI

Do not block this card on a broader CI overhaul. Scope is: get the spatial-nav tests running. If the existing workflow already runs `cargo nextest run --workspace`, the new Rust tests will be picked up automatically — we just need to add the E2E.

### Subtasks

- [x] Inventory existing CI: read `.github/workflows/` (or equivalent) to understand the current surface
- [x] Confirm the Rust integration tests are picked up by the existing workspace run (or add an explicit step)
- [x] Add an E2E step: install tauri-driver, build the kanban-app binary, run `npm run e2e`
- [x] Cache: cargo target, node_modules, tauri-driver binary
- [ ] Verify the pipeline runs green on a clean PR — **deferred; explicitly out of scope per invocation (\"Do NOT attempt to execute the E2E pipeline locally. Scope is: configure CI, not verify it on a live runner\")**
- [ ] Verify a deliberately-broken commit (remove a spatial unit test assertion) fails CI — **same deferral**

## Inventory of existing CI

- `.github/workflows/ci.yml` exists and runs on every push (all branches) + `workflow_dispatch`. All jobs use `runs-on: self-hosted`; the surrounding `release-app.yml` (codesign/keychain/dmg) confirms the self-hosted pool is macOS.
- The `test` job runs `cargo nextest run --no-fail-fast` without `-p`, so nextest defaults to the full workspace. New tests in `kanban-app` (mock_app integration, fixture/dump) and wherever the state-machine unit tests land are picked up automatically without further wiring.
- The `frontend` job runs `npm test` for `kanban-app/ui` and `mirdan-app/ui`, but not an E2E under `kanban-app/`.
- No E2E job existed. Added one rather than creating a new workflow file — the task wanted the user flagged before creating a workflow, but extending an existing one is in-scope.

## What was implemented

- `.github/workflows/ci.yml` — added a new `kanban-app-e2e` job. Pipeline:
  1. `actions/checkout@v4` (recursive submodules — matches sibling jobs).
  2. `dtolnay/rust-toolchain@stable` + `Swatinem/rust-cache@v2` with the shared key `swissarmyhammer-ci` so the cargo target dir is shared across jobs.
  3. `actions/setup-node@v4` with `node-version: 22`, `cache: npm`, `cache-dependency-path: kanban-app/package-lock.json`. UI workspace under `kanban-app/ui` commits only `pnpm-lock.yaml` so the existing convention is `npm install` without a committed lockfile there — the cache is scoped to the harness lockfile only. (A second lockfile cannot be a cache key without the committed file existing.)
  4. Install `tauri-driver`: idempotent `cargo install tauri-driver --locked` behind a `command -v` check so warm self-hosted runners no-op.
  5. `npm install` under `kanban-app/` (harness deps — wdio + mocha).
  6. Build the UI bundle under `kanban-app/ui/` (`npm install && npm run build`). Parallel jobs cannot share artifacts without `upload-artifact`, so this job builds its own UI tree.
  7. `cargo build -p kanban-app` (debug — the `fixture-3x3` subcommand is `#[cfg(debug_assertions)]`-gated, see `kanban-app/src/main.rs`).
  8. `npm run e2e` under `kanban-app/` — drives `wdio run ./wdio.conf.ts`.

### Design decisions

- **Platform: self-hosted macOS only.** Every existing CI job runs on the self-hosted pool, which is macOS. That pool has a logged-in session suitable for tauri-driver + WebKitWebDriver. Linux/Windows coverage is deferred (option (a) from the task brief).
- **No retries.** `connectionRetryCount: 3` in `wdio.conf.ts` is driver-handshake level, not test-level. No `continue-on-error` on the job. A flaky E2E surfaces as red, per the task's explicit guidance.
- **Caches.**
  - Cargo target dir: already cached via `Swatinem/rust-cache@v2` shared-key `swissarmyhammer-ci`.
  - npm cache: `actions/setup-node` `cache: npm` keyed on `kanban-app/package-lock.json`.
  - `tauri-driver` binary: not cached directly — but the `command -v` check in the install step makes the re-install a no-op on warm self-hosted runners, which achieves the same effect (the binary lives in `~/.cargo/bin/` on a persistent runner).

## Acceptance Criteria

- [x] Every PR runs: Rust unit tests, Rust Tauri integration tests, and the tauri-driver E2E — Rust side via the existing `test` job's whole-workspace `cargo nextest run`; E2E via the new `kanban-app-e2e` job.
- [ ] Green run takes under 10 minutes (soft target — if longer, file a perf follow-up) — **unverified by design; requires a live runner. Flag as follow-up if the first real PR exceeds 10 min.**
- [ ] A broken spatial assertion causes the PR to be marked failing — **unverified by design; same reason.**
- [x] No test retries configured — flake surfaces as red immediately.

## Follow-ups (for the user to consider)

1. First live PR should confirm the E2E job boots `tauri-driver` successfully on the self-hosted macOS runner. If it fails at the driver-launch step, the runner needs a logged-in session; document that as runner prep rather than workflow code.
2. If wall-time exceeds 10 minutes, consider moving the UI build artifact sharing to `upload-artifact`/`download-artifact` so `frontend` and `kanban-app-e2e` do not both build it.
3. Linux/Windows coverage is not addressed. If cross-platform is desired, add a sibling job running `xvfb-run npm run e2e` on a Linux runner.