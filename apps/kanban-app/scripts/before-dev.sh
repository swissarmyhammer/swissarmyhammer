#!/usr/bin/env bash
#
# `beforeDevCommand` wrapper for `cargo tauri dev`.
#
# Tauri's `beforeDevCommand` accepts a single command, but `cargo tauri dev`
# needs two steps: stage the `kanban` CLI sidecar (so the bundled-binary
# resolution does not fail with a missing-sidecar error), and start the UI
# dev server.
#
# The sidecar is staged first and synchronously; the UI dev server runs last
# because it is a long-lived process Tauri waits on. Any arguments passed to
# this script are forwarded to `stage-cli-sidecar.sh`.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

# Step 1: build the standalone CLI and stage it for the sidecar bundle.
"${SCRIPT_DIR}/stage-cli-sidecar.sh" "$@"

# Step 2: start the UI dev server (the original beforeDevCommand behavior).
exec sh -c 'cd "$0/ui" && npm install && npm run dev' "${APP_DIR}"
