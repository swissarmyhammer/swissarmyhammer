#!/usr/bin/env bash
#
# `beforeBuildCommand` wrapper for `cargo tauri build`.
#
# Tauri's `beforeBuildCommand` accepts a single command, but a release build
# needs two preparation steps: stage the `kanban` CLI sidecar, and build the
# UI bundle. This wrapper runs both in order.
#
# Any arguments passed to this script are forwarded to `stage-cli-sidecar.sh`,
# so CI can invoke it with `--target <triple>` (Tauri does not forward build
# args here, so the staging script also derives the host triple on its own).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

# Step 1: build the standalone CLI and stage it for the sidecar bundle.
"${SCRIPT_DIR}/stage-cli-sidecar.sh" "$@"

# Step 2: build the UI bundle (the original beforeBuildCommand behavior).
(cd "${APP_DIR}/ui" && npm install && npm run build)
