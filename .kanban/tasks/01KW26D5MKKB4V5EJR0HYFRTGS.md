---
assignees:
- claude-code
depends_on:
- 01KW26CTFQH36STPXTAE8SQQJW
position_column: todo
position_ordinal: c180
project: expect
title: 'gui surface adapter: macOS AX + Tauri bridge (testable core)'
---
## What
Add the `gui` surface for macOS — drive and observe native desktop apps through the OS accessibility API (AX), including Tauri webview content via the bridged a11y tree. This is the testable core (the CI runner is a Mac). Windows/Linux are a separate follow-on task. Per `ideas/expect.md` §"Surface adapters" (gui row) and §"Drilling into a Tauri / Electron app".

- New `crates/swissarmyhammer-expect/src/surface/gui.rs` implementing `SurfaceAdapter` for macOS:
  - macOS **AX** (`AXUIElement`): drive via `AXUIElementPerformAction`/`AXPress`, observe the AX subtree. A Tauri WKWebView's web content appears under an `AXWebArea` (e.g. a `<button aria-label>` shows as a named `button`) — no CDP/Node needed.
  - Drive/observe by `role[name=…]`; reuse the a11y locator dialect + resolution from the browser task (identical, a11y-stable, not pixels). A control rename ⇒ structural drift.
  - Provision launches the app; teardown closes it.
  - Structure the OS backend behind a small per-OS trait/seam so the Windows/Linux follow-on slots in cleanly behind `cfg`.
- Validate against the in-repo `kanban-app` (Tauri): `surface: gui`, native AX, NO CDP/Node. Quality caveat: the bridged tree is only as good as the web app's semantics.

## Acceptance Criteria
- [ ] On macOS, the adapter launches a native/Tauri app, presses a control by `role[name=…]`, and snapshots the resulting AX tree.
- [ ] A Tauri app's bridged web content appears as real AX nodes; the adapter drives it with no CDP/Node.
- [ ] Locator binding/drift behaves like the browser surface (rename ⇒ structural drift).
- [ ] Teardown closes the app; the per-OS seam is in place (Windows/Linux are `unimplemented`/cfg-gated stubs, not built here).

## Tests
- [ ] Integration test on the macOS CI runner driving a minimal Tauri fixture (or `kanban-app`) by role+name and asserting an observed AX node. Gate to macOS; skip cleanly off-platform.
- [ ] `cargo nextest run -p swissarmyhammer-expect gui` passes on macOS (or skips cleanly off-platform).

## Workflow
- Use `/tdd`.