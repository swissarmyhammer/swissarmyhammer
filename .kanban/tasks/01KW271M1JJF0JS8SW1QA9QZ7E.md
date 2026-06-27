---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw4swgjted4pgrkga0hpt7ae
  text: 'Picked up. Studied macOS backend (src/surface/gui.rs): GuiBackend trait seam, RawAxNode->A11yNode via pure to_a11y_node, MacBackend live AX reader. Plan: reuse RawAxNode + to_a11y_node (its doc already declares it the shared seam "filled from AXUIElement, IUIAutomation, ...") for all OSes. Add cfg(windows) WindowsBackend over uiautomation 0.25 (InvokePattern/ValuePattern, role = ControlType Display e.g. "Button") and cfg(linux) LinuxBackend over atspi 0.30 + zbus (AccessibleProxy/ActionProxy/EditableTextProxy, role normalized to single token via new pure atspi_role_token since the role[name=] grammar [A-Za-z][A-Za-z0-9_-]* rejects AT-SPI''s spaced "push button"). Target-specific deps so macOS never pulls them. OS-agnostic unit tests (run on macOS): atspi_role_token + parameterized role-vocabulary mapping/locator-bind/drift across macOS/Windows/Linux roles. Linux atspi is async -> backend owns a tokio Runtime + block_on (mirrors browser adapter).'
  timestamp: 2026-06-27T15:06:34.458922+00:00
- actor: claude-code
  id: 01kw4tq1xn8ng1tfsz378emk16
  text: |-
    Implemented. Files: crates/swissarmyhammer-expect/src/surface/gui.rs (windows + linux backends behind GuiBackend seam; new pure atspi_role_token; cfg routing macos/windows/linux/stub; OS-agnostic tests), tests/gui.rs (OS-neutral docs), Cargo.toml (workspace + crate target-specific deps), Cargo.lock.

    Crate choices: Windows = uiautomation 0.25 (IUIAutomation; InvokePattern to press, ValuePattern to type/read value; role = ControlType Display e.g. "Button"). Linux = atspi 0.30 + zbus (AccessibleProxy walk; ActionProxy.do_action(0) to press; EditableTextProxy.set_text_contents to type; TextProxy to read value; role = atspi_role_token(get_role_name) e.g. "push-button"). Backend owns a tokio Runtime + block_on (mirrors browser adapter). Both reuse the existing RawAxNode + to_a11y_node shared mapping (RawAxNode doc already declared it the shared FFI seam) and A11ySelector::matches — identical drift semantics. Deps are [target.'cfg(target_os="windows")'] / [target.'cfg(target_os="linux")'] so macOS pulls neither (confirmed: resolved into Cargo.lock, not compiled on macOS).

    New pure logic atspi_role_token: AT-SPI role names have spaces ("push button") but the shared role[name=] grammar role class is [A-Za-z][A-Za-z0-9_-]*, so whitespace runs collapse to '-'. TDD'd (RED then GREEN).

    OS-agnostic coverage (runs on macOS): atspi_role_token_collapses_whitespace_to_a_single_token; parameterized every_backend_role_vocabulary_maps_binds_and_drifts over macOS/Windows/Linux role vocabularies — maps role/name/value via to_a11y_node, binds a role[name=] locator (Holds), renames -> Drifted. Linux row asserted == atspi_role_token("push button") so no hardcode drift.

    macOS verification GREEN: cargo nextest run -p swissarmyhammer-expect = 305 passed (incl. 9 gui unit tests + integration test skipping cleanly); cargo check --workspace clean; cargo clippy -p swissarmyhammer-expect --all-targets -D warnings clean; cargo fmt --check clean (also proves the cfg-gated win/linux modules parse).

    Cross-target: x86_64-pc-windows-msvc NOT installed -> Windows verification requires Windows CI. x86_64-unknown-linux-gnu installed but cross-check fails in C dep `ring` (needs x86_64-linux-gnu-gcc, absent on this macOS box) before reaching my Linux Rust -> Linux verification requires Linux CI. cfg gating correct by construction (target deps + cfg attrs).

    double-check: found Linux read_raw hardcoded value:None (asymmetric vs macOS/Windows value reads) -> FIXED by reading AT-SPI Text interface (TextProxy get_text), re-ran macOS suite green. Live win/linux API-call shapes are by-construction (followed uiautomation 0.25 / atspi 0.30 docs) and exercised only on platform CI.
  timestamp: 2026-06-27T15:21:04.181367+00:00
depends_on:
- 01KW26D5MKKB4V5EJR0HYFRTGS
position_column: doing
position_ordinal: '8280'
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