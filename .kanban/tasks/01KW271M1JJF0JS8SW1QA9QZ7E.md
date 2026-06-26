---
assignees:
- claude-code
depends_on:
- 01KW26D5MKKB4V5EJR0HYFRTGS
position_column: todo
position_ordinal: c380
project: expect
title: 'gui surface adapter: Windows UIA + Linux AT-SPI (behind cfg)'
---
## What
Extend the `gui` surface to Windows and Linux, filling in the per-OS backends behind the seam established by the macOS gui adapter task. Per `ideas/expect.md` §"Surface adapters" (gui row) and §"Drilling into a Tauri / Electron app".

- In `crates/swissarmyhammer-expect/src/surface/gui.rs` (per-OS modules behind `cfg`):
  - **Windows**: UIA (`IUIAutomation`), drive via `InvokePattern`, observe the UIA tree. WebView2 content is bridged into UIA; note the optional CDP escape hatch (`--remote-debugging-port`) for thin bridged trees but do not require it.
  - **Linux**: AT-SPI (`atspi` + `zbus`), bridged for WebKitGTK; drive/observe by role+name.
  - Reuse the shared a11y locator dialect + resolution (same as macOS/browser); identical drift semantics.
  - Provision/teardown per OS.

## Acceptance Criteria
- [ ] On Windows, the adapter presses a control by `role[name=…]` via UIA InvokePattern and snapshots the UIA tree; a WebView2/Tauri app's bridged content is reachable.
- [ ] On Linux, the adapter drives/observes via AT-SPI; a WebKitGTK/Tauri app's bridged content is reachable.
- [ ] Locator binding/drift matches the macOS/browser behavior.
- [ ] macOS path from the core task is unaffected.

## Tests
- [ ] Per-OS integration tests gated by `cfg(target_os)`, driving a minimal native/Tauri fixture by role+name and asserting an observed node. Skip cleanly on other OSes; document which CI runners exercise them.
- [ ] `cargo nextest run -p swissarmyhammer-expect gui` passes on the target OS (or skips cleanly).

## Workflow
- Use `/tdd`.