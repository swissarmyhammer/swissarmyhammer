#!/usr/bin/env bash
#
# verify-app-bundle.sh — gate the release pipeline on a healthy `.app` bundle.
#
# The release workflow (.github/workflows/release-app.yml, job build-macos)
# calls this once per app against the freshly built bundle, after
# `cargo tauri build` and before the DMG is uploaded. A non-zero exit fails
# the release, catching regressions where the `kanban` CLI sidecar silently
# drops out of the bundle or fails signing. Keeping the checks in a script —
# rather than a workflow heredoc — makes them testable (see
# apps/kanban-app/tests/verify_bundle.rs).
#
# Checks performed:
#   * The bundle path exists and is a directory.
#   * With --require-cli: Contents/MacOS/kanban exists, has the executable
#     bit, and `Contents/MacOS/kanban --version` exits 0 printing a non-empty
#     version string.
#   * Unless --skip-signing: `codesign --verify --deep --strict <bundle>`
#     passes.
#
# Diagnostics go to stderr. Any failure exits non-zero with a clear message.
#
# Usage:
#   verify-app-bundle.sh <bundle.app> [--require-cli] [--skip-signing]
#
# Arguments:
#   <bundle.app>     Path to the .app bundle to verify (required, positional).
#
# Options:
#   --require-cli    Assert the bundle ships a working `kanban` CLI sidecar at
#                    Contents/MacOS/kanban. Pass this for the `kanban` app;
#                    omit it for apps with no CLI (e.g. `mirdan`).
#   --skip-signing   Skip the `codesign` verification. Use for unsigned
#                    local/test bundles; the release pipeline omits it so
#                    signing is enforced.

set -euo pipefail

# --- argument parsing -------------------------------------------------------

bundle=""
require_cli="false"
skip_signing="false"

die() {
    echo "verify-app-bundle.sh: $1" >&2
    exit 1
}

while [ $# -gt 0 ]; do
    case "$1" in
        --require-cli)  require_cli="true";  shift ;;
        --skip-signing) skip_signing="true"; shift ;;
        --*) die "unknown option: $1" ;;
        *)
            if [ -n "$bundle" ]; then
                die "unexpected extra argument: $1"
            fi
            bundle="$1"
            shift
            ;;
    esac
done

# --- validation -------------------------------------------------------------

[ -n "$bundle" ] || die "missing required <bundle.app> path argument"
[ -d "$bundle" ] || die "bundle does not exist or is not a directory: $bundle"

# --- CLI sidecar check ------------------------------------------------------

if [ "$require_cli" = "true" ]; then
    cli="$bundle/Contents/MacOS/kanban"

    [ -e "$cli" ] || die "bundle is missing the kanban CLI: expected $cli"
    [ -f "$cli" ] || die "kanban CLI is not a regular file: $cli"
    [ -x "$cli" ] || die "kanban CLI is not executable (missing executable bit): $cli"

    # `--version` must exit 0 and print a non-empty version string, proving
    # the bundled CLI is a real, runnable binary rather than a stub.
    if ! version="$("$cli" --version 2>/dev/null)"; then
        die "kanban CLI failed to run \`--version\`: $cli"
    fi
    # Trim surrounding whitespace before the emptiness check.
    version="$(printf '%s' "$version" | tr -d '[:space:]')"
    [ -n "$version" ] || die "kanban CLI \`--version\` printed an empty version string: $cli"

    echo "verify-app-bundle.sh: kanban CLI OK ($cli)" >&2
fi

# --- code signing check -----------------------------------------------------

if [ "$skip_signing" = "true" ]; then
    echo "verify-app-bundle.sh: skipping codesign verification (--skip-signing)" >&2
else
    if ! codesign --verify --deep --strict "$bundle" 2>&1; then
        die "codesign verification failed for bundle: $bundle"
    fi
    echo "verify-app-bundle.sh: codesign verification OK ($bundle)" >&2
fi

echo "verify-app-bundle.sh: bundle verified OK ($bundle)" >&2
