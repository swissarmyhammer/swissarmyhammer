#!/usr/bin/env bash
#
# gen-cask.sh — render a Homebrew cask Ruby file for an app DMG.
#
# The release workflow (.github/workflows/release-app.yml, job
# publish-homebrew-cask) calls this once per app to produce the cask Ruby that
# is committed to swissarmyhammer/homebrew-tap. Extracting the rendering into a
# script keeps the cask logic testable (see apps/kanban-app/tests/cask_gen.rs)
# instead of buried in a workflow heredoc.
#
# The full cask Ruby is written to stdout. Diagnostics go to stderr.
#
# Required arguments:
#   --name      <slug>     cask token / file basename, e.g. "kanban"
#   --product   <name>     app bundle product name, e.g. "Kanban" (-> Kanban.app)
#   --version   <version>  bare version, no leading "v", e.g. "0.10.0"
#   --sha256    <hash>     SHA-256 of the DMG
#   --dmg-name  <file>     release asset file name, e.g. "Kanban_aarch64.dmg"
#   --desc      <text>     cask description
#   --homepage  <url>      cask homepage URL
#
# Optional arguments:
#   --cli-binary <path>    bundle-relative path to a CLI binary inside the
#                          .app, e.g. "Contents/MacOS/kanban". When supplied,
#                          the cask additionally emits:
#                            * a `binary` stanza symlinking that binary onto
#                              PATH at install time, and
#                            * a `conflicts_with formula: "<name>"` stanza so
#                              the cask and the standalone cargo-dist formula
#                              never both own the CLI symlink on PATH.
#                          Omit it for apps that ship no CLI (e.g. mirdan).

set -euo pipefail

# --- argument parsing -------------------------------------------------------

name=""
product=""
version=""
sha256=""
dmg_name=""
desc=""
homepage=""
cli_binary=""

die() {
    echo "gen-cask.sh: $1" >&2
    exit 1
}

while [ $# -gt 0 ]; do
    case "$1" in
        --name)       name="${2-}";       shift 2 ;;
        --product)    product="${2-}";    shift 2 ;;
        --version)    version="${2-}";    shift 2 ;;
        --sha256)     sha256="${2-}";     shift 2 ;;
        --dmg-name)   dmg_name="${2-}";   shift 2 ;;
        --desc)       desc="${2-}";       shift 2 ;;
        --homepage)   homepage="${2-}";   shift 2 ;;
        --cli-binary) cli_binary="${2-}"; shift 2 ;;
        *) die "unknown argument: $1" ;;
    esac
done

# --- validation -------------------------------------------------------------

[ -n "$name" ]     || die "missing required --name"
[ -n "$product" ]  || die "missing required --product"
[ -n "$version" ]  || die "missing required --version"
[ -n "$sha256" ]   || die "missing required --sha256"
[ -n "$dmg_name" ] || die "missing required --dmg-name"
[ -n "$desc" ]     || die "missing required --desc"
[ -n "$homepage" ] || die "missing required --homepage"

# The release tag is the version prefixed with "v"; the DMG is published under
# that tag on the GitHub release.
tag="v${version}"
download_url="https://github.com/swissarmyhammer/swissarmyhammer/releases/download/${tag}/${dmg_name}"

# --- render -----------------------------------------------------------------

printf 'cask "%s" do\n' "$name"
printf '  version "%s"\n' "$version"
printf '  sha256 "%s"\n' "$sha256"
printf '\n'
printf '  url "%s"\n' "$download_url"
printf '  name "%s"\n' "$product"
printf '  desc "%s"\n' "$desc"
printf '  homepage "%s"\n' "$homepage"
printf '\n'
printf '  app "%s.app"\n' "$product"

if [ -n "$cli_binary" ]; then
    printf '\n'
    # `#{appdir}` is a Homebrew interpolation evaluated by Ruby at install
    # time, not a shell expansion — emit it literally.
    printf '  binary "#{appdir}/%s.app/%s"\n' "$product" "$cli_binary"
    printf '  conflicts_with formula: "%s"\n' "$name"
fi

printf 'end\n'
