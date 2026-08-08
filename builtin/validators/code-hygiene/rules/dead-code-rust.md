---
name: dead-code-rust
description: Rust items nothing reaches, and Rust files no module declares — checked by the compiler, not by prompt.
match:
  files:
    - "**/*.rs"
  project_types:
    - rust
supersedes: dead-code
tool:
  scope: workspace
  run: |
    cargo check --workspace --all-targets --message-format=json --quiet |
      jq -c 'select(.reason == "compiler-message")
             | .message
             | select(.code.code == "dead_code")
             | select(.spans | length > 0)
             | {file: .spans[0].file_name, line: .spans[0].line_start, message: .message}
             | select(.file | startswith("/") | not)' |
      sort -u
    index="$(mktemp -d)"
    trap 'rm -rf "$index"' EXIT
    find . -name target -prune -o -name .git -prune -o -name '*.rs' -print |
      while IFS= read -r file; do
        base="${file##*/}"
        case "$base" in lib.rs | main.rs | mod.rs | build.rs) continue ;; esac
        dir="${file%/*}"
        crate="$dir"
        while [ "$crate" != "." ] && [ ! -f "$crate/Cargo.toml" ]; do
          parent="${crate%/*}"
          [ "$parent" = "$crate" ] && break
          crate="$parent"
        done
        case "${dir#"$crate"/}" in tests | tests/* | benches | benches/* | examples | examples/* | src/bin | src/bin/*) continue ;; esac
        key="$(printf '%s' "$crate" | tr -c 'A-Za-z0-9' '_')"
        if [ ! -f "$index/$key" ]; then
          {
            grep -rhoE '\bmod[[:space:]]+[A-Za-z_][A-Za-z0-9_]*' "$crate" --include='*.rs' --exclude-dir=target 2>/dev/null | awk '{print $2}'
            grep -rhoE '#\[path[[:space:]]*=[[:space:]]*"[^"]*"' "$crate" --include='*.rs' --exclude-dir=target 2>/dev/null | sed -e 's/.*"\(.*\)"/\1/' -e 's#.*/##' -e 's/\.rs$//'
          } | sort -u >"$index/$key"
        fi
        grep -qxF "${base%.rs}" "$index/$key" && continue
        printf '%s:1: orphan module: no `mod %s;` declaration in this crate names this file, so nothing compiles it\n' "${file#./}" "${base%.rs}"
      done
  doctor:
    check_command: "which cargo jq find grep sed awk sort tr mktemp"
    check_version_command: "cargo --version"
    fix_hint: "rustup toolchain install stable"
---

# Dead Code — Rust

The Rust compiler already answers the dead-code question, and it answers it
better than a reader can. `dead_code` is a rustc lint, warn by default, and it
runs over the whole crate graph: it knows every caller a private item could
have, and it exempts a `pub` item, an entry point, and an `extern "C"` export by
itself, because those have callers the crate cannot see.

That is the whole reason this rule supersedes the `dead-code` prompt rule for
Rust. The prompt rule's carve-outs are not judgments here — three of them are
compiler behavior, and the fourth, staged work, becomes an annotation.

## The staging contract

Write `#[expect(dead_code, reason = "...")]` on an item a later change will
consume. Nothing else counts. A staged item with no annotation is dead.

`#[expect]` is the marker, not `#[allow]`, because `#[expect]` expires by
itself. Measured on a probe crate: an `#[expect(dead_code, ...)]` on a genuinely
dead item is silent, and the same annotation on an item that later gains a
caller raises `unfulfilled_lint_expectations`, so the compiler asks for the
annotation back the moment the staging is over. `#[allow(dead_code)]` never
expires and silently outlives the plan that justified it.

The `reason` is not decoration. It names the change that lands the consumer, so
the next reader can tell staged work from a leftover.

## What the compiler exempts on its own, and what this run reports

A `pub` item reachable from the crate root is the crate's surface for callers
outside the repository, so `dead_code` never reports it. A `#[test]` function
and a `#[cfg(test)]` helper are compiled into the test target, where the harness
is their caller. `main` is an entry point. An `#[unsafe(no_mangle)]` or
`extern "C"` item is an FFI export. None of these needs a carve-out written in
prose.

What is left is the narrow set a tool decides alone: a private item, or a
private field, variant, or method, that no file of the crate reaches.

Measured on this workspace: `cargo check --workspace --all-targets` reports
**0** `dead_code` findings. The gate costs nothing here and catches the next one.

## The orphan-module half

The compiler cannot report a file it never reads. A `.rs` file no `mod`
declaration names is not part of any crate, so `cargo check` is silent about it
however dead it is. The second half of the script closes that hole with `grep`.

A file is exempt when it is a crate root or a module root by name — `lib.rs`,
`main.rs`, `mod.rs`, `build.rs` — or when it sits in a directory cargo compiles
file by file: `tests/`, `benches/`, `examples/`, and `src/bin/`, where every
file is its own target root. Everything else must be named by a
`mod <stem>;` declaration, or by a `#[path = "..."]` attribute, somewhere in its
own crate. The crate is the nearest ancestor directory holding a `Cargo.toml`,
so a `mod` declaration in a sibling crate does not excuse a file.

The index is built one time for each crate rather than one time for each file,
which is what keeps the scan at **6.7 s** over this whole workspace.

Measured on this workspace: **5** orphan files, every one hand-checked and every
one real — `crates/swissarmyhammer/src/security.rs` (15 KB of code nothing
compiles), `crates/markdowndown/src/error.rs`, `crates/markdowndown/src/fetch.rs`,
`crates/swissarmyhammer-common/src/sample_avp_test.rs`, and
`crates/swissarmyhammer-tools/src/mcp/notifications.rs` (an empty file). No
false positive.

To exempt an orphan file, name it — that is the fix. There is no suppression for
a file nothing compiles, because there is nothing to suppress it in.

## How the run is shaped

The scope is `workspace` because cargo checks a package, never a loose file, and
because a `mod` declaration for one file can live in any other file of its
crate. The engine keeps only the findings in the changed files.

`--all-targets` checks the library target and the test targets separately, so
one item in one file arrives twice; `sort -u` collapses the pair. The same run
on a probe crate re-emits its warnings from the cargo cache on a second, fully
fresh run, so a repeated review still reports.

`select(.file | startswith("/") | not)` drops the generated code cargo writes
under `OUT_DIR`, which arrives as an absolute path and is not editable.

The `jq` filter selects the `dead_code` code and drops every other diagnostic
cargo emits. Selection here is attribution, not exemption: to exempt one item,
write `#[expect(dead_code, reason = "...")]` on it in the code.

The rule declares no install commands. `cargo` and the `dead_code` lint are the
Rust toolchain itself, not a package with its own version, so no install command
can pin them. The `doctor.fix_hint` states `rustup toolchain install stable`
instead. `sah doctor` shows that hint as the fix; the install lifecycle never
runs it.
