---
assignees:
- claude-code
position_column: doing
position_ordinal: '80'
project: spatial-nav
title: Actually execute the spatial-nav E2E suite and make it pass
---
## What

Task `01KPG7ZGYEKSYBNMZ9F634KKHY` landed the full E2E harness (`kanban-app/e2e/spatial-nav.e2e.ts`, `wdio.conf.ts`, `package.json`, `tsconfig.json`) but the suite has **never actually run** against a real `tauri-driver` instance. The whole purpose of that task was "the test we can point at and say the thing actually works." Right now we have code that typechecks — not a green E2E run.

This task's only goal: boot the suite, watch it pass, fix whatever breaks.

## Critical: YOU run the tests, not the user

**This task is assigned to an agent. The agent (you) must execute every step below itself. Do not stop and ask the user to run commands for you. Do not report "please verify locally" or "needs human validation." If a command fails, debug and retry. If infrastructure is missing, install it. The acceptance criteria is a green test run that YOU produced — not a checklist that you hand off.**

If a step genuinely cannot be executed from the sandbox (e.g. `tauri-driver` requires a display server that isn't present), stop, document exactly what's blocking you, and file a follow-up task describing the infrastructure gap. Do NOT claim success in that case.

## Environment check first

Before doing anything else, run these to understand what you have:

```bash
uname -a                    # OS + arch
which node && node --version
which npm && npm --version
which cargo && cargo --version
which tauri-driver || echo "tauri-driver not installed"
# macOS specifically:
xcode-select -p 2>/dev/null && echo "xcode tools present"
safaridriver --version 2>/dev/null || echo "safaridriver not found"
```

If you are on macOS, the E2E uses WKWebView via `WebKitWebDriver` on Linux or `safaridriver` on macOS. Figure out which applies to your environment and proceed.

## Procedure

1. **Install tauri-driver if missing.**
   - macOS: `cargo install tauri-driver --locked` (check latest version matches the Tauri 2.x series).
   - The driver binary must be on PATH before WDIO can run.

2. **Enable the webdriver for the local webview.**
   - macOS: `sudo safaridriver --enable` — this **requires sudo** and touches the system. If you do not have sudo and cannot obtain it, STOP and file a blocker task describing exactly what's needed. Do NOT skip this step.
   - Linux: ensure `WebKitWebDriver` is installed (`apt install webkit2gtk-driver`) and has `xvfb-run` if headless.

3. **Build the UI bundle.**
   ```bash
   cd kanban-app/ui
   npm install
   npm run build
   ```

4. **Build the kanban-app binary.**
   ```bash
   cd /Users/wballard/github/swissarmyhammer/swissarmyhammer-navigation
   cargo build --release -p kanban-app
   ```
   The E2E config points at the release binary — verify the path in `wdio.conf.ts` matches what cargo produced.

5. **Install E2E JS deps.**
   ```bash
   cd kanban-app
   npm install
   ```

6. **Run the suite.**
   ```bash
   cd kanban-app
   npm run e2e
   # or whatever the `scripts.e2e` entry in package.json actually is
   ```
   Capture the full output. If there is no `e2e` script, read `package.json` and pick the right entry point — `wdio wdio.conf.ts run` is the fallback.

7. **If it fails, fix it.** Common expected failures:
   - Selector drift — the test queries DOM attributes that may have been renamed
   - Timing — `waitUntil` timeouts too tight for release-build startup
   - `__spatial_dump` not callable from WDIO — may need a different invoke path than `invoke()`
   - Focus semantics differ between keyboard events and programmatic dispatch
   - Fixture path race between the app's file watcher and the E2E's fixture writer
   Debug, don't shim. Do not add `retries: 3` to hide flake. Do not skip failing assertions.

8. **Verify determinism.** Once green, run the suite **3 times in a row**. If any run fails, the test is flaky — diagnose and fix the race. Only after 3 consecutive green runs is the task done.

9. **Update `kanban-app/README.md`** if the steps above diverge from what it currently documents. The README should describe exactly what you did.

## Acceptance Criteria

- [ ] `tauri-driver` installed and on PATH, driver for the local webview enabled
- [ ] `kanban-app/e2e/spatial-nav.e2e.ts` has run to completion with zero failures against the actual binary
- [ ] Three consecutive green runs — no flake
- [ ] Any fixes required to get there are committed alongside this task, with `fix(spatial-nav e2e): ...` commit messages
- [ ] The README accurately describes the steps you actually ran
- [ ] If something blocked you (sudo, missing driver on this platform, etc.) — task is NOT complete; instead file a clearly-scoped blocker task and report back

## What success looks like

The final report on this task quotes the actual test output, showing `7 passing` (or however many `it()` blocks exist) with wall-clock time. Not "the test appears to work" — the actual WDIO output pasted into the task.