---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffe880
project: cli-in-app
title: Bundle the kanban CLI into Kanban.app as a Tauri sidecar
---
## What
Make `cargo tauri build` (and `cargo tauri dev`) embed the standalone `kanban` CLI binary inside the app bundle so it ships at `Kanban.app/Contents/MacOS/kanban`, signed and notarized as part of the bundle.

Use Tauri v2's `externalBin` (sidecar) mechanism — sidecars are copied into `Contents/MacOS/` alongside the main `kanban-app` binary and are signed/notarized with the bundle.

Files to create/modify:
- `apps/kanban-app/tauri.conf.json` — add `bundle.externalBin: ["binaries/kanban"]`. Tauri resolves this to `binaries/kanban-<target-triple>` and copies it into the bundle as `kanban`.
- `apps/kanban-app/scripts/stage-cli-sidecar.sh` (new) — builds `kanban-cli` in release for the requested target triple (`cargo build -p kanban-cli --release [--target <triple>]`), then copies the resulting `kanban` binary to `apps/kanban-app/binaries/kanban-<triple>`. Triple resolution: honor a `--target` arg if passed, else derive the host triple from `rustc -vV`.
- `apps/kanban-app/scripts/before-build.sh` and `apps/kanban-app/scripts/before-dev.sh` (new) — wrapper scripts that run `stage-cli-sidecar.sh` and then the existing UI build (`npm install && npm run build` / `npm run dev` in `./ui`). Tauri's `beforeBuildCommand`/`beforeDevCommand` accept only one command, so both steps must be combined.
- `apps/kanban-app/tauri.conf.json` — repoint `beforeBuildCommand`/`beforeDevCommand` at the new wrapper scripts (keep the `./ui` UI build behavior intact).
- `apps/kanban-app/.gitignore` (new or amended) — ignore the generated `binaries/` staging directory.

Notes:
- CI builds with `cargo tauri build --target aarch64-apple-darwin` (see `.github/workflows/release-app.yml`); the script's `--target` passthrough must cover this. `just kanban-build` builds without `--target`.
- Sidecar bundling is cross-platform: the same config ships `kanban.exe` inside a future Windows build with no extra work (Windows PATH registration is a separate deferred task).
- Do NOT merge the two binaries — `kanban-app` keeps `windows_subsystem = "windows"` which would suppress CLI console output on Windows. They stay separate binaries co-packaged in one bundle.

Implementation note: `apps/kanban-app/build.rs` was also updated to stage the sidecar before `tauri_build::build()`. `tauri-build` validates `externalBin` paths at compile time, so a plain `cargo build`/`cargo test -p kanban-app` (which skips the Tauri CLI and its `beforeBuildCommand`) would otherwise fail with a missing-sidecar error. The build script invokes the idempotent staging script with cargo's `TARGET`, making the crate self-sufficient for both the Tauri-CLI and plain-cargo build paths.

## Acceptance Criteria
- [x] `cargo tauri build` for the kanban app produces `Kanban.app/Contents/MacOS/kanban` that is executable and runs (`kanban --version` succeeds).
- [x] `cargo tauri dev` still launches without a missing-sidecar error.
- [x] The UI build still runs as part of `beforeBuildCommand`/`beforeDevCommand`.
- [x] `apps/kanban-app/binaries/` is git-ignored; no built binary is committed.

## Tests
- [x] New `apps/kanban-app/tests/sidecar.rs`: invoke `scripts/stage-cli-sidecar.sh` (host triple), assert a file matching `apps/kanban-app/binaries/kanban-*` exists, has the executable bit set, and that running it with `--version` exits 0 and prints a non-empty version string. Model on the existing `apps/kanban-cli/tests/build_artifacts.rs` build-artifact test.
- [x] Test command: `cargo test -p kanban-app --test sidecar` — passes.
- [x] Verify locally: `just kanban-build` then `test -x /Applications/Kanban.app/Contents/MacOS/kanban && /Applications/Kanban.app/Contents/MacOS/kanban --version`.

## Workflow
- Use `/tdd` — write the failing `sidecar.rs` test first, then add the script + config to make it pass.