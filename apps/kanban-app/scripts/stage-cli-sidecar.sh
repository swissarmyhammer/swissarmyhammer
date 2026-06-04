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
#   stage-cli-sidecar.sh [--target <triple>] [--profile <dev|release>]
#
# Triple resolution:
#   - If `--target <triple>` is passed, build and stage for that triple, and
#     pass `--target` to cargo so the binary lands under
#     target/<triple>/<profile-dir>. This covers CI's
#     `cargo tauri build --target aarch64-apple-darwin`.
#   - Otherwise, derive the host triple from `rustc -vV`'s `host:` line and
#     build WITHOUT `--target`, so the binary lands under target/<profile-dir>
#     and reuses the workspace's shared (already-compiled) dependency artifacts
#     instead of forcing a fresh target-specific build. The staged filename
#     still carries the host triple suffix Tauri's externalBin resolution
#     expects. This covers `just kanban-build`, which builds without `--target`.
#
# Profile:
#   - Defaults to `release` (what Tauri bundles ship).
#   - `--profile dev` builds the much faster unoptimized binary; the staging
#     test uses this so it exercises the path/exec-bit/runnability contract
#     without paying for a release build of the whole dependency tree.
#
# The script is run by the `before-build.sh` / `before-dev.sh` wrappers that
# tauri.conf.json points `beforeBuildCommand` / `beforeDevCommand` at.

set -euo pipefail

# Resolve key directories relative to this script so the script works no
# matter what the caller's current working directory is.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
REPO_ROOT="$(cd "${APP_DIR}/../.." && pwd)"

# Parse the optional --target and --profile arguments.
TARGET=""
PROFILE="release"
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
    --profile)
      if [ $# -lt 2 ]; then
        echo "error: --profile requires a name argument" >&2
        exit 1
      fi
      PROFILE="$2"
      shift 2
      ;;
    --profile=*)
      PROFILE="${1#--profile=}"
      shift
      ;;
    *)
      echo "error: unknown argument: $1" >&2
      echo "usage: stage-cli-sidecar.sh [--target <triple>] [--profile <dev|release>]" >&2
      exit 1
      ;;
  esac
done

# Track whether the caller pinned an explicit target; this decides whether we
# pass --target to cargo (and thus which target/ subtree the binary lands in).
EXPLICIT_TARGET=1
if [ -z "${TARGET}" ]; then
  EXPLICIT_TARGET=0
  TARGET="$(rustc -vV | awk '/^host: / { print $2 }')"
  if [ -z "${TARGET}" ]; then
    echo "error: could not determine host triple from 'rustc -vV'" >&2
    exit 1
  fi
fi

# Map the cargo profile name to its target/ output subdirectory. cargo's `dev`
# profile emits into `debug/`; every other profile (release, custom) uses its
# own name as the directory.
if [ "${PROFILE}" = "dev" ]; then
  PROFILE_DIR="debug"
  CARGO_PROFILE_ARGS=(--profile dev)
else
  PROFILE_DIR="${PROFILE}"
  CARGO_PROFILE_ARGS=(--profile "${PROFILE}")
fi

echo "Staging kanban CLI sidecar for target: ${TARGET} (profile: ${PROFILE})"

# Build the CLI. Only pass --target when the caller pinned one: an explicit
# target lands the binary under target/<triple>/<profile-dir>, while a host
# build (no --target) lands under target/<profile-dir> and reuses the shared
# dependency cache.
CARGO_TARGET_ARGS=()
BUILT_DIR="${REPO_ROOT}/target/${PROFILE_DIR}"
if [ "${EXPLICIT_TARGET}" -eq 1 ]; then
  CARGO_TARGET_ARGS=(--target "${TARGET}")
  BUILT_DIR="${REPO_ROOT}/target/${TARGET}/${PROFILE_DIR}"
fi

# Note the `${arr[@]+"${arr[@]}"}` guard on CARGO_TARGET_ARGS: macOS ships bash
# 3.2, where expanding an *empty* array as `"${arr[@]}"` under `set -u` aborts
# with "unbound variable". The host-build path leaves CARGO_TARGET_ARGS empty,
# so it must use the guarded form. CARGO_PROFILE_ARGS is always non-empty.
(cd "${REPO_ROOT}" && cargo build -p kanban-cli "${CARGO_PROFILE_ARGS[@]}" ${CARGO_TARGET_ARGS[@]+"${CARGO_TARGET_ARGS[@]}"})

# Locate the freshly built binary. Windows targets append `.exe`.
BIN_NAME="kanban"
case "${TARGET}" in
  *windows*) BIN_NAME="kanban.exe" ;;
esac

BUILT_BIN="${BUILT_DIR}/${BIN_NAME}"
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
