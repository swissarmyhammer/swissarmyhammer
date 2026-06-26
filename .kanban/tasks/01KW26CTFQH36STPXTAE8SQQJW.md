---
assignees:
- claude-code
depends_on:
- 01KW26C3MSTK5T2QVE4XBV9CH8
position_column: todo
position_ordinal: c080
project: expect
title: browser surface adapter (CDP a11y via chromiumoxide)
---
## What
Add the `browser` surface: drive and observe a web UI through the accessibility tree via CDP — pure Rust, no Node/Playwright. Per `ideas/expect.md` §"Surface adapters" (browser row) and §"Accessibility is the GUI's drive and observe channel".

- New `crates/swissarmyhammer-expect/src/surface/browser.rs` implementing `SurfaceAdapter`:
  - In-process mechanism: **CDP** `Accessibility` + `Input` via `chromiumoxide` (pure Rust). Provision launches/attaches to Chromium; teardown closes it.
  - Drive: press/type by `role[name=…]` (CDP `Input`). Observe: snapshot the a11y tree (CDP `Accessibility`).
  - Locator dialect: `role[name=…]` + tree relationship (`within`/`ancestor`); a11y-stable, NOT pixels. A control rename surfaces as honest structural drift. Extend the assertion compiler for the a11y locator dialect.
  - Sparse a11y → vision/OCR is an explicit last resort (note, don't build now); a sparse tree is itself a signal.
- Deterministic surface: mechanical a11y actuation is reproducible (reclassifies browser alongside cli/http) — runs once by default; non-determinism only via the agent fallback.

## Acceptance Criteria
- [ ] Against a fixture web page, the adapter presses a `button[name="…"]` and snapshots the resulting a11y tree.
- [ ] `role[name=…]` + `within`/`ancestor` locators bind and evaluate; a renamed control surfaces as structural drift.
- [ ] No Node and no Playwright — chromiumoxide/CDP only.
- [ ] Teardown closes the browser.

## Tests
- [ ] Integration test serving a tiny static HTML fixture, driving a button by role+name and asserting an observed a11y node value. (Gate behind a feature/availability check if Chromium isn't present in CI; document the skip.)
- [ ] `cargo nextest run -p swissarmyhammer-expect browser` passes (or skips cleanly without Chromium).

## Workflow
- Use `/tdd`.