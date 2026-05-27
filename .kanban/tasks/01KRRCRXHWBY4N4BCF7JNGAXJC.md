---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffed80
project: cli-in-app
title: Make the CLI-install admin prompt attribute to Kanban and explain its purpose
---
## What
When `Kanban.app` self-installs the `kanban` CLI into a root-owned directory, it currently escalates by spawning the `osascript` executable as a subprocess (`apps/kanban-app/src/cli_install.rs`, `install_with_escalation`, the `std::process::Command::new("osascript")` call). Because the privileged request comes from the spawned `osascript` process, the macOS authentication dialog reads **"osascript wants to make changes"** — opaque and slightly alarming — and gives the user no clue it is about installing a command-line tool.

Fix both problems:

1. **Attribution** — run the AppleScript *in-process* via `NSAppleScript` instead of spawning the `osascript` binary. When the AppleScript is executed inside the running `Kanban.app` process, macOS attributes the `with administrator privileges` request to the app, so the auth dialog reads **"Kanban wants to make changes"**.
2. **Purpose** — before triggering the password prompt, show a brief explanatory dialog so the user understands what is happening: that Kanban wants to install the `kanban` command-line tool so it can be used from the terminal. An in-process `NSAppleScript` `display dialog` with `Install` / `Not Now` buttons keeps this self-contained in `cli_install.rs` (no `AppHandle` plumbing). Only proceed to the privileged `ln -sf` when the user chooses `Install`; on `Not Now`, take the same path as a declined prompt.

## Platform gating — ONLY the new AppleScript code is macOS-gated
Scope decision (confirmed with the user): macOS-gate ONLY the new AppleScript / `NSAppleScript` / `objc2-foundation` code. Do NOT gate the whole `cli_install` module, and do NOT touch the existing `#[cfg(unix)]` / `#[cfg(not(unix))]` symlink stubs from the prior task — leave `resolve_bundled_cli`, `install_cli_symlink`, `pick_target_dir`, `create_symlink`/`replace_symlink`, `is_writable`, `homebrew_bin` exactly as they are.

Honest non-macOS behavior — NO pretending:
- `objc2-foundation` goes ONLY under `[target.'cfg(target_os = "macos")'.dependencies]` in `apps/kanban-app/Cargo.toml`.
- `install_with_escalation` becomes a `#[cfg(target_os = "macos")]` function — it genuinely runs AppleScript via `NSAppleScript`, and the function exists only on macOS.
- Do NOT write a `#[cfg(not(target_os = "macos"))]` twin of `install_with_escalation` that "logs and writes the marker." There is no AppleScript on Linux/Windows — do not fake one, do not write the `.cli-install-attempted` marker there (no prompt was shown, so there is nothing to record).
- In `run()`, `#[cfg]`-split the `needs_escalation` branch: on macOS it calls `install_with_escalation`; on non-macOS it honestly logs (e.g. `tracing::debug!`) that privileged CLI install is macOS-only and returns, doing nothing else.
- `build_install_applescript` (the pure `String` builder below) stays platform-neutral — no macOS types — so it compiles and is unit-tested on any host.

Files to modify:
- `apps/kanban-app/src/cli_install.rs`:
  - Extract a pure helper `fn build_install_applescript(bundled: &Path, link: &Path) -> String` (platform-neutral) that renders the full AppleScript source — the explanatory `display dialog` text plus the `do shell script "ln -sf '<bundled>' '<link>'" with administrator privileges` — with correct AppleScript string escaping (the existing backslash/quote escaping in `install_with_escalation` must be preserved and moved here).
  - Make `install_with_escalation` a `#[cfg(target_os = "macos")]` function: keep the one-shot `.cli-install-attempted` marker guard exactly as-is; build the script via the new helper; execute it in-process with `NSAppleScript` instead of `Command::new("osascript")`; keep all `tracing` logging; still write the marker regardless of accept/decline.
  - `#[cfg]`-split the `needs_escalation` branch in `run()` as described above. No non-macOS escalation function.
  - Keep the `NSAppleScript` execution isolated and documented as deliberately not unit-tested, mirroring the convention already documented on `pick_target_dir`.
- `apps/kanban-app/Cargo.toml` — add the `objc2-foundation` dependency (the crate exposing `NSAppleScript`) under the existing `[target.'cfg(target_os = "macos")'.dependencies]` block ONLY, with the feature flags needed for `NSAppleScript`.
- `apps/kanban-app/tests/cli_install.rs` — add unit tests for `build_install_applescript` (see Tests).

## Acceptance Criteria
- [x] The privileged-install escalation runs the AppleScript in-process via `NSAppleScript`; `cli_install.rs` no longer spawns the `osascript` executable (`Command::new("osascript")` is gone). As a consequence macOS attributes the auth dialog to "Kanban", not "osascript".
- [x] The escalation flow shows an explanatory dialog naming the `kanban` command-line tool and offering an explicit `Install` / `Not Now` choice before the password prompt; choosing `Not Now` skips the privileged step and is treated like a declined prompt.
- [x] The one-shot `.cli-install-attempted` marker behavior on macOS is unchanged — at most one lifetime prompt per machine, written regardless of accept/decline.
- [x] `objc2-foundation` appears ONLY under `[target.'cfg(target_os = "macos")'.dependencies]`; `install_with_escalation` and every `NSAppleScript`/`objc2-foundation` use site is `#[cfg(target_os = "macos")]`-gated.
- [x] There is NO non-macOS twin of `install_with_escalation`. On non-macOS, `run()`'s escalation branch honestly logs-and-returns: no AppleScript is faked and the `.cli-install-attempted` marker is NOT written.
- [x] `cargo build -p kanban-app` compiles on macOS with the new dependency, AND the `cli_install.rs` module cross-compiles for `x86_64-unknown-linux-gnu` (no `objc2-foundation`/`NSAppleScript` symbol reaches a non-macOS build). See Tests note: the full `cargo check --target x86_64-unknown-linux-gnu` is blocked by a pre-existing, unrelated `glib-sys` GTK-sysroot cross-compile failure in Tauri's Linux deps.
- [x] `build_install_applescript` produces AppleScript source containing the explanatory text, an `Install` default button, and a correctly path-escaped `ln -sf` under `with administrator privileges`.

## Tests
- [x] In `apps/kanban-app/tests/cli_install.rs`, add unit tests for `build_install_applescript` (the helper is platform-neutral, so these run on any host):
  - asserts the rendered source contains the explanatory phrase identifying the `kanban` command-line tool.
  - asserts it contains `with administrator privileges` and an `ln -sf` linking the given `bundled` path to the given `link` path.
  - asserts a path containing characters needing AppleScript escaping (e.g. a quote or backslash) is escaped correctly and not emitted raw.
- [x] Test command: `cargo test -p kanban-app --test cli_install` — passes (13 tests: 9 existing cli_install tests plus 4 new `build_install_applescript` tests).
- [x] Non-macOS compile gate: the full `cargo check -p kanban-app --target x86_64-unknown-linux-gnu` fails on a PRE-EXISTING, unrelated `glib-sys v0.18.1` build-script error (GTK pkg-config cross-compilation not configured on this macOS host — a transitive Tauri Linux dep, nothing this change touches). Per the task's escape clause, this is reported rather than fixed. To prove this change introduces no new non-macOS breakage, `cli_install.rs` was cross-compiled standalone (with its real `dirs`/`tracing` deps) via a throwaway crate: `cargo check --target x86_64-unknown-linux-gnu` succeeded and `objc2-foundation` was correctly absent from the Linux dependency tree (`cargo tree --target x86_64-unknown-linux-gnu -i objc2-foundation` → "nothing to print"); the same crate also `cargo check`ed for `aarch64-apple-darwin` with `objc2-foundation` present.
- [x] `cargo clippy -p kanban-app --all-targets -- -D warnings` — clean.
- [x] The in-process `NSAppleScript` invocation and the system dialogs are GUI/privilege side effects that cannot be unit-tested; isolated behind `build_install_applescript` (pure, tested) and the boundary is documented in a code comment on `install_with_escalation`, exactly as `pick_target_dir` documents its untested escalation branch. The non-macOS honest-skip path is verified by the Linux `cargo check` of the module compiling.

## Workflow
- Use `/tdd` — write the `build_install_applescript` tests first against the signature above, then implement the helper and rewire `install_with_escalation`.