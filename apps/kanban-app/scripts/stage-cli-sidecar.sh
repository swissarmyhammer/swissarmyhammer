#!/usr/bin/env bash
#
# Build the standalone `kanban` CLI in release mode and stage it where Tauri's
# `externalBin` (sidecar) mechanism expects to find it.
#
# Tauri v2 resolves a sidecar declared in tauri.conf.json as `binaries/kanban`
# to `binaries/kanban-<target-triple>`, then copies it into the app bundle as
# `Kanban.app/Contents/MacOS/kanban` (signed and notarized with the bundle).
# This script produces that triple-suffixed file.
#
# Usage:
#   stage-cli-sidecar.sh [--target <triple>]
#
# Triple resolution:
#   - If `--target <triple>` is passed, build and stage for that triple.
#     This covers CI's `cargo tauri build --target aarch64-apple-darwin`.
#   - Otherwise, derive the host triple from `rustc -vV`'s `host:` line.
#     This covers `just kanban-build`, which builds without `--target`.
#
# The script is run by the `before-build.sh` / `before-dev.sh` wrappers that
# tauri.conf.json points `beforeBuildCommand` / `beforeDevCommand` at.

set -euo pipefail

# Resolve key directories relative to this script so the script works no
# matter what the caller's current working directory is.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
REPO_ROOT="$(cd "${APP_DIR}/../.." && pwd)"

# Parse the optional --target argument.
TARGET=""
while [ $# -gt 0 ]; do
  case "$1" in
    --target)
      if [ $# -lt 2 ]; then
        echo "error: --target requires a triple argument" >&2
        exit 1
      fi
      TARGET="$2"
      shift 2
      ;;
    --target=*)
      TARGET="${1#--target=}"
      shift
      ;;
    *)
      echo "error: unknown argument: $1" >&2
      echo "usage: stage-cli-sidecar.sh [--target <triple>]" >&2
      exit 1
      ;;
  esac
done

# Fall back to the host triple when no --target was supplied.
if [ -z "${TARGET}" ]; then
  TARGET="$(rustc -vV | awk '/^host: / { print $2 }')"
  if [ -z "${TARGET}" ]; then
    echo "error: could not determine host triple from 'rustc -vV'" >&2
    exit 1
  fi
fi

echo "Staging kanban CLI sidecar for target: ${TARGET}"

# Build the CLI in release mode. Passing --target keeps cargo's output layout
# predictable: with --target the binary lands under target/<triple>/release,
# without it under target/release.
CARGO_TARGET_ARGS=()
if [ -n "${TARGET}" ]; then
  CARGO_TARGET_ARGS=(--target "${TARGET}")
fi

(cd "${REPO_ROOT}" && cargo build -p kanban-cli --release "${CARGO_TARGET_ARGS[@]}")

# Locate the freshly built binary. Windows targets append `.exe`.
BIN_NAME="kanban"
case "${TARGET}" in
  *windows*) BIN_NAME="kanban.exe" ;;
esac

BUILT_BIN="${REPO_ROOT}/target/${TARGET}/release/${BIN_NAME}"
if [ ! -f "${BUILT_BIN}" ]; then
  echo "error: expected built CLI binary not found at ${BUILT_BIN}" >&2
  exit 1
fi

# Stage the binary with the triple suffix Tauri's externalBin resolution
# expects. The `binaries/` directory is git-ignored staging output.
STAGE_DIR="${APP_DIR}/binaries"
mkdir -p "${STAGE_DIR}"

STAGED_NAME="kanban-${TARGET}"
case "${TARGET}" in
  *windows*) STAGED_NAME="kanban-${TARGET}.exe" ;;
esac
STAGED_BIN="${STAGE_DIR}/${STAGED_NAME}"

cp "${BUILT_BIN}" "${STAGED_BIN}"
chmod +x "${STAGED_BIN}"

echo "Staged sidecar: ${STAGED_BIN}"
