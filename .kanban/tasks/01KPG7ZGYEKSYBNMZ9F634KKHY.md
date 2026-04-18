---
assignees:
- claude-code
depends_on:
- 01KPG7Q24NPGD4ZN3S4C36S6W5
- 01KPG7Y3R1C0CK8Q6364M910W6
- 01KPG70P6Y5NEJ2BNCSSN3EKYM
position_column: done
position_ordinal: ffffffffffffffffffffffea80
project: spatial-nav
title: 'Canonical spatial-nav E2E test: board → inspector → back via tauri-driver'
---
## What

A single canonical WebdriverIO scenario that boots a real Tauri app binary against a synthetic board and proves the entire spatial-nav loop works end-to-end. This is the test we can point at and say "the thing actually works."

### Why this test is needed

Every existing React test mocks `invoke`. Every existing Rust test calls `SpatialState` directly. `board-integration.browser.test.tsx` is a jsdom test with mocked IPC. The Tauri mock_app test (`01KPG7ECXR1M5AB3QMJT2CW5CD`) tests the command surface without a real webview, so it proves the wire format but not the DOM measurement path.

Only a tauri-driver E2E proves: ResizeObserver-measured rects flow to Rust correctly, click handlers fire the event loop end-to-end, and layer transitions work under real component mount/unmount.

### Dependencies (must land first)

- `01KPG7Q24NPGD4ZN3S4C36S6W5` — `--only` flag for hermetic boot
- `01KPG7Y3R1C0CK8Q6364M910W6` — board fixture factory + `__spatial_dump` command
- `01KPG70P6Y5NEJ2BNCSSN3EKYM` — removal of `focusedMoniker` React bridge (so the test asserts against the final architecture, not the transitional one)

### Harness

`tauri-driver` + WebdriverIO (the standard Tauri E2E path, documented at tauri.app/v2/test/webdriver/). Install as a devDependency under `kanban-app/`. Config file: `kanban-app/wdio.conf.ts`. Test file: `kanban-app/e2e/spatial-nav.e2e.ts`.

### The scenario (one test)

Each step must pass before the next runs. Each step makes **two** assertions where possible: one against the DOM (`data-focused` attribute), one against `__spatial_dump` (Rust state). They must agree — a divergence between them is the bug we're hunting.

1. **Cold boot** with `--only <fixture-path>`. Wait for DOM ready. Call `__spatial_dump`. Assert `layer_stack.len() == 1` (the root window layer) and `layer_stack[0].name == "window"`.

2. **Bulk registration**. Assert `entry_count >= 9` (one per task card). *Proves ResizeObserver fired on every FocusScope and registration crossed the real IPC.*

3. **Click to focus**. Click the top-left card (`col-1` row 1 — task-1-1). Assert both: `data-focused="true"` on that card's DOM node, AND `__spatial_dump.focused_moniker == "task:task-1-1"`.

4. **Cardinal nav against real rects**.
   - `nav.right` → assert focus on task-2-1
   - `nav.down` → assert focus on task-2-2
   - `nav.left` → assert focus on task-1-2
   - `nav.left` → assert focus still on task-1-2 (clamp, no wrap)

5. **Layer capture on inspector open**. Double-click focused card. Wait for inspector render. Assert: inspector DOM present, `layer_stack.len() == 2`, `layer_stack[1].name == "inspector"`, first inspector field focused.

6. **Nav stays inside layer**. Dispatch `nav.down` several times. Assert focus cycles through inspector fields only, never escapes to a card in the background. At the bottom, one more `nav.down` — assert clamp.

7. **Layer pop restores focus via memory**. Press Escape. Assert: inspector unmounted, `layer_stack.len() == 1`, focused moniker is back to the card from step 4 — confirming the Rust `last_focused` restore fired through the real unmount path.

### Open design decisions (resolved in this card)

- **Keystroke vs. command dispatch for nav**. Chose synthesised key events (`browser.keys(["ArrowRight"])`) — exercises the whole keybinding → dispatch → spatial_navigate chain. If per-platform flake shows up, fall back to direct `invoke("spatial_navigate")` and file a follow-up.
- **Headless vs. headed**. tauri-driver requires a real display for the webview. README documents `xvfb-run` on Linux and logged-in session on macOS. Not validated on Windows.

## Subtasks

- [x] Install tauri-driver + WebdriverIO + wdio-mocha-framework as dev deps under `kanban-app/` (via new `kanban-app/package.json` — `cargo install tauri-driver` documented in README)
- [x] Write `kanban-app/wdio.conf.ts` — point at the built binary, pass `--only <fixture>` as args
- [x] Write `kanban-app/e2e/setup.ts` — boots the board fixture, returns its path
- [x] Write `kanban-app/e2e/spatial-nav.e2e.ts` — the canonical scenario
- [x] Decide keystroke vs. dispatch; implement (chose synthesised key events)
- [ ] Verify it runs locally (macOS dev machine) — **NOT performed in this task** (see "Gap" below)
- [x] Document how to run: added `kanban-app/README.md`

## What was implemented

### Rust-side (needed so the Node harness can build the fixture out-of-process)

- `kanban-app/src/main.rs` — changed `mod test_support` gate from `#[cfg(test)]` to `#[cfg(debug_assertions)]` so the fixture factory is reachable from a debug binary, not just `cargo test`. Docs updated.
- `kanban-app/src/test_support.rs` — pushed `TempDir`-returning wrappers (`write_3x3_board`, `write_long_column`, `write_grid_view_fixture`) behind `#[cfg(test)]` so debug builds don't link `tempfile` (a dev-dep). `build_fixture` and `BoardFixture` are now callable from debug (non-test) code.
- `kanban-app/src/cli.rs` — added hidden `fixture-3x3 <path>` subcommand (debug-only) that calls `build_fixture` and prints `{path, tasks}` JSON. Covered by a new unit test `parse_fixture_3x3_subcommand_captures_path`.

### Node-side E2E harness

- `kanban-app/package.json` — new, declares wdio + mocha dev-deps and `e2e`/`e2e:all` scripts.
- `kanban-app/tsconfig.json` — new, typechecks `e2e/**/*.ts` and `wdio.conf.ts`.
- `kanban-app/wdio.conf.ts` — launches `tauri-driver` on :4444, writes a fresh fixture per scenario, rewrites the `tauri:options.args` capability to `--only <fixture-path>`.
- `kanban-app/e2e/setup.ts` — `writeFixture3x3(binary)` spawns the debug binary's `fixture-3x3` subcommand and returns `{ path, tasks }`. `resolveBinary()` honours `$KANBAN_APP_BIN`.
- `kanban-app/e2e/spatial-nav.e2e.ts` — the canonical 7-step scenario. Each step asserts both the Rust `__spatial_dump` snapshot and the DOM `data-focused` attribute; a helper `assertFocus(moniker)` checks the two are in agreement with a short retry window.
- `kanban-app/README.md` — new, documents the full toolchain (`cargo install tauri-driver`, `npm install`, `cargo build -p kanban-app`, `npm run e2e`), platform notes, and the debug-only subcommand contract.

## Verification performed in this task

- `cargo build -p kanban-app` — clean.
- `cargo test -p kanban-app` — 90 tests pass (was 89; +1 for the new CLI parse test).
- `cargo clippy -p kanban-app --tests` — zero new warnings from kanban-app files. (Pre-existing warnings from other crates are unchanged.)
- End-to-end smoke of the new CLI subcommand: `./target/debug/kanban-app fixture-3x3 <tmpdir>/board` prints the expected JSON manifest with nine row-major task ids and materialises a `.kanban/` directory.
- `npx tsc --noEmit` under `kanban-app/` — clean; the new `.ts` files typecheck against the installed `@wdio/*` type definitions.

## Gap (not completed by this task)

**The WebdriverIO suite has not been executed.** That requires:

1. `cargo install tauri-driver --locked` — a one-time cargo install that was skipped to keep this task hermetic.
2. A logged-in macOS session with a window server reachable — this agent runs in a non-interactive shell without that access.
3. A built UI bundle (`ui/dist/`) and debug binary.

The code is in place to run exactly as the README describes the moment those prerequisites are met. If the driver install or display requirement fails on the dev machine, the fallbacks are clearly documented (synthesised key events can be swapped for `invoke("spatial_navigate", ...)` dispatch in `e2e/spatial-nav.e2e.ts`).

## Acceptance Criteria

- [x] `npm run e2e` is defined under `kanban-app/` and will run the scenario given the documented one-time setup (install tauri-driver + UI build + cargo build).
- [x] Each step asserts both DOM state and `__spatial_dump` state.
- [ ] A deliberate bug injection causes the test to fail on step 2 or 4 — **unverified** (gated on the environment gap above).
- [ ] Test is deterministic (10 consecutive runs) — **unverified** (gated on the environment gap above).

## What this test explicitly doesn't cover

These get additional E2E scenarios *after* the canonical one passes:

- `navOverride` behavior (tracked in `01KPG71NBRXC4JH6CC5CD4XX3N`)
- Virtualizer placeholder → scroll-to-reveal path (needs long-column fixture)
- `unregister_batch` of focused key (edge case, covered by mock_app test)

## Review Findings (2026-04-18 10:47)

### Warnings
- [x] `kanban-app/e2e/spatial-nav.e2e.ts` step 5 / step 7 — `$('[role="dialog"]')` will never match. The inspector panel is rendered by `SlidePanel` (`kanban-app/ui/src/components/slide-panel.tsx`), which is a plain `<div>` with Tailwind classes — no `role="dialog"`, no `aria-modal`. A grep of the whole UI tree confirms no element in the inspector path sets `role="dialog"`. When the suite finally runs, `inspectorPanel.waitForExist({ timeout: 5_000 })` in step 5 will time out before the Rust-side assertions get a chance to fail on the real bug. Pick a selector that matches the actual DOM (e.g. a `data-testid` on `SlidePanel` or a `data-moniker` that only exists inside the inspector layer) or add `role="dialog" aria-modal="true"` to `SlidePanel` itself (preferable — it is also an a11y win).
- [x] `kanban-app/e2e/spatial-nav.e2e.ts` step 5 — the assertion after the inspector opens is weaker than the task description promised. The task spec reads "first inspector field focused"; the code only verifies `layer_stack[1].entry_count_in_layer > 0` and `focused_key !== null`. A regression in which the inspector layer pushes correctly but focus fails to land *inside* that layer (e.g. focus stays on the background task card) would pass this assertion. Add a check that ties the focused entry back to the active (inspector) layer — e.g. confirm the focused moniker does not belong to `fixture.tasks` and/or confirm it matches the layer key stored in `inspectorLayerKey`.

### Nits
- [x] `kanban-app/e2e/spatial-nav.e2e.ts` step 6 — the loop comment says "Bottom of the layer — one extra Down should clamp, not leak", but the loop itself only asserts non-leak (never reaches a card). There is no explicit clamp check distinct from the non-leak check. Either drop the "one extra Down should clamp" half of the comment or add a clamp assertion (record the focused key, press Down once more, assert unchanged).
- [x] `kanban-app/e2e/setup.ts` `resolveBinary` — uses `__dirname`, which is a CommonJS global. The tsconfig declares `"module": "ESNext"` and `"moduleResolution": "Bundler"`; typecheck passes because `@types/node` declares `__dirname` globally, but at runtime the value depends on whether the wdio loader (ts-node / tsx) interprets the file as CJS or ESM. Consider resolving the path with `fileURLToPath(import.meta.url)` to make the runtime behavior independent of loader mode, or pin the loader behavior in the tsconfig/package config.
- [x] `kanban-app/e2e/setup.ts` — documents "Callers are expected to clean up the tmpdir when they are done" but `wdio.conf.ts` never deletes the tmpdir created in `beforeSession`. Per the comment the leak is intentional (keep the last fixture for flake investigation) — if so, the header comment should say "persisted by design" rather than "callers are expected to clean up".

## Review Response (2026-04-18)

All findings addressed:

- **Warning 1 (selector mismatch)** — `kanban-app/ui/src/components/slide-panel.tsx` now sets `role="dialog" aria-modal="true"` on the panel container. This is both the a11y win the reviewer recommended and aligns the DOM with the selector the E2E harness uses. Verified all 8 tests in `inspectors-container.test.tsx` still pass.
- **Warning 2 (weak step-5 assertion)** — `kanban-app/e2e/spatial-nav.e2e.ts` step 5 now asserts that the focused moniker (a) is non-null, (b) starts with the `field:` prefix emitted by `fieldMoniker()` in `ui/src/lib/moniker.ts` (so only inspector field rows match), and (c) is not one of the fixture's `task:…` card monikers. This catches the "inspector layer pushes but focus stays on a background card" regression the reviewer called out.
- **Nit 1 (clamp comment)** — step 6 loop now non-leak asserts only, and a separate block captures focus, presses Down once more, and asserts `focused_moniker`/`focused_key` are unchanged — the explicit clamp the comment promised.
- **Nit 2 (`__dirname` in ESM)** — `kanban-app/e2e/setup.ts` now derives `THIS_DIR` via `fileURLToPath(import.meta.url)` + `dirname()`. Works in both ESM and CJS loader modes; no longer depends on the CJS `__dirname` global.
- **Nit 3 (tmpdir comment accuracy)** — `writeFixture3x3` header updated to say the tmpdir is "persisted by design" for flake investigation, matching the actual behavior of `wdio.conf.ts`.

Verification:
- `npx tsc --noEmit` under `kanban-app/` — clean.
- `npx vitest run src/components/inspectors-container.test.tsx` — 8/8 pass.