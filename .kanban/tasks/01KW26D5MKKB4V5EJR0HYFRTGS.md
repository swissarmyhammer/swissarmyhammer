---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw4p7c6mrc25gx0jvwqva44v
  text: 'Picked up. Research done. Plan: (1) extract the shared a11y drive dialect (press/type by role[name=…]) from browser.rs into new surface/a11y.rs as `A11yAction` (browser migrates to it — true dedupe, no copy), reusing A11ySelector (already shared in assertion.rs) + A11yNode + resolve_a11y. (2) New surface/gui.rs: pure FFI-free `RawAxNode` plain tree + `to_a11y_node` mapping (load-bearing unit coverage), a per-OS `GuiBackend` trait seam (macOS impl via the `accessibility` crate, Windows/Linux cfg-gated unimplemented stubs), GuiAdapter/GuiSut implementing SurfaceAdapter (launch app, press/type via AXPress/AX value, snapshot AX subtree, kill on teardown). AX crate choice: `accessibility` 0.1 (safe AXUIElement wrapper: application(pid), role()/title()/description()/value()/children(), perform_action) + `accessibility-sys` 0.2 (AXIsProcessTrusted, kAXPressAction) + core-foundation 0.10 — all already compatible with the lock. (3) Integration test tests/gui.rs gated on cfg(macos)+AXIsProcessTrusted()+a launchable fixture (env EXPECT_GUI_FIXTURE_APP), skips cleanly+logs otherwise.'
  timestamp: 2026-06-27T14:02:36.116547+00:00
- actor: claude-code
  id: 01kw4q7e3qxt3hnesah8614h21
  text: 'Implementation landed and green. Files: NEW crates/swissarmyhammer-expect/src/surface/gui.rs (GuiAdapter/GuiSut SurfaceAdapter, pure RawAxNode→A11yNode to_a11y_node mapping, per-OS GuiBackend trait seam, macOS AX backend via accessibility/accessibility-sys/core-foundation, non-macOS cfg-gated unimplemented stub, gui_automation_available gate); NEW src/surface/a11y.rs (shared A11yAction drive dialect + step_resolves_mechanically + unbound + DEFAULT_ACTION_TIMEOUT, extracted from browser.rs — browser migrated to it, no behavior change, parse tests moved); NEW tests/gui.rs (gated integration test: provision→drive press by role[name]→observe→compile/evaluate→teardown, env-driven EXPECT_GUI_FIXTURE_APP/DRIVE_STEP/ASSERT, skips cleanly without AX permission/fixture); assertion.rs A11ySelector::matches made pub(crate) for gui backend reuse; lib.rs exports A11yAction + gui types; Cargo.toml (root + crate) add macOS-only AX deps. AX crate choice: accessibility 0.1 + accessibility-sys 0.1 + core-foundation 0.9 (versions aligned to what `accessibility` itself resolves, to avoid a two-core-foundation type collision — original 0.2/0.10 pins failed to compile). Verified: cargo build OK, cargo clippy -p swissarmyhammer-expect --all-targets -D warnings clean, cargo fmt --check clean, cargo check --workspace OK (macOS), cargo nextest run -p swissarmyhammer-expect = 303 passed/0 failed (gui+a11y+surface filter = 81 passed). double-check round 1 flagged the live integration test never drove a press (criterion #1) — fixed by adding the drive(press)+assert loop; awaiting re-check. Leaving task in doing for /review.'
  timestamp: 2026-06-27T14:20:06.647210+00:00
depends_on:
- 01KW26CTFQH36STPXTAE8SQQJW
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffff8280
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