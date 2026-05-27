# kanban-app

The kanban desktop app — a Tauri 2 application with a React frontend for
browsing and editing the same `.kanban/` board the `kanban` CLI and MCP server
use.

This README is for **contributors**. End-user install instructions (Homebrew
cask, DMG, building from source) live in the [kanban CLI
README](../kanban-cli/README.md#desktop-app).

## The bundled CLI sidecar

`Kanban.app` ships the standalone `kanban` CLI inside its own bundle. Installing
the app gives the user both the GUI and the command-line tool — there is no
separate CLI install step on macOS.

This is implemented with two pieces that exist solely to support that "install
one thing, get both" story:

### `scripts/stage-cli-sidecar.sh` — build & stage the sidecar

The CLI is a **Tauri sidecar** (an `externalBin` entry in `tauri.conf.json`).
Tauri copies a sidecar into the bundle as `Kanban.app/Contents/MacOS/kanban`,
alongside the main `kanban-app` executable, and signs/notarizes it with the
bundle.

Tauri resolves the configured `binaries/kanban` sidecar to a triple-suffixed
file `binaries/kanban-<target-triple>`. `stage-cli-sidecar.sh` produces exactly
that file: it builds `kanban-cli` in release mode (`cargo build -p kanban-cli
--release`, honoring an optional `--target <triple>`) and copies the result to
`binaries/kanban-<triple>`. The `binaries/` directory is generated staging
output and is git-ignored.

The script runs from three places:

- `scripts/before-build.sh` / `scripts/before-dev.sh` — the wrappers that
  `tauri.conf.json` points `beforeBuildCommand` / `beforeDevCommand` at (they
  also run the `./ui` frontend build).
- `build.rs` — `tauri-build` validates `externalBin` paths at compile time, so
  a plain `cargo build`/`cargo test -p kanban-app` (which skips the Tauri CLI
  and its `beforeBuildCommand`) would fail with a missing-sidecar error
  otherwise. The build script invokes the idempotent staging script with
  cargo's `TARGET` so the crate builds standalone too.

`kanban-app` and `kanban-cli` stay **separate binaries** co-packaged in one
bundle — they are not merged. `kanban-app` is built with
`windows_subsystem = "windows"`, which would suppress CLI console output on
Windows; the CLI must remain its own binary.

### `src/cli_install.rs` — self-install onto `PATH` at launch

A bundled binary at `Contents/MacOS/kanban` is not on the user's `PATH`. How it
gets there depends on the install method:

- **Homebrew cask** — the cask's `binary` stanza links the bundled CLI onto
  `PATH` at install time. The app does nothing; `already_installed` detects the
  cask-created link and short-circuits.
- **DMG drag to `/Applications`** — there is no package manager, so the app
  installs the CLI itself. `cli_install::run` (spawned on a background thread at
  launch by `cli_install::spawn`) creates a `kanban` symlink pointing at the
  bundled CLI in a directory that is both user-writable and on the default
  `PATH` — preferring the Homebrew `bin` directory, falling back to
  `/usr/local/bin`.

The self-install is **silent, idempotent, self-healing, and non-destructive**:

- No menu item, no click — it runs once per launch on a background thread.
- Re-running is a no-op when the link is already correct; a stale link left by
  a moved or replaced `Kanban.app` is repaired in place.
- A pre-existing real (non-symlink) `kanban` file — an unrelated tool of the
  same name — is never overwritten.

When the only viable directory is root-owned `/usr/local/bin`, creating the
symlink needs administrator rights, so the app shows an explanatory dialog and
then the macOS admin password prompt. Self-install is gated solely on the
`kanban` symlink — there is no remembered-attempt state. If the symlink is
absent (the user declined, or it was later deleted), the next launch offers to
install it again; once the link is present, `already_installed` short-circuits
and the app stays silent.

See the module documentation at the top of `src/cli_install.rs` for the full
design rationale and the per-function contracts.
