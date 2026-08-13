---
name: unused-dependencies-rust
description: Cargo dependencies no source of the package names — checked by cargo machete, not by prompt.
match:
  files:
    - "**/Cargo.toml"
  project_types:
    - rust
tool:
  scope: workspace
  run: |
    set -e
    work="$(mktemp -d)"
    trap 'rm -rf "$work"' EXIT
    find . -name target -prune -o -name .git -prune -o -name node_modules -prune -o -name '*.toml' -print |
      sort |
      while IFS= read -r manifest; do
        grep -q '^\[package\]' "$manifest" || continue
        manifest="${manifest#./}"
        dir="${manifest%/*}"
        [ "$dir" = "$manifest" ] && dir="."
        scan="$dir/${manifest##*/}"
        if [ "${manifest##*/}" != "Cargo.toml" ]; then
          copy="$work/$(printf '%s' "$manifest" | tr -c 'A-Za-z0-9' '_')"
          mkdir -p "$copy"
          (cd "$dir" && find . -name target -prune -o -name '*.rs' -print) |
            while IFS= read -r source; do
              mkdir -p "$copy/${source%/*}"
              cp "$dir/$source" "$copy/$source"
            done
          cp "$manifest" "$copy/Cargo.toml"
          scan="$copy/Cargo.toml"
        fi
        status=0
        cargo-machete "$scan" > "$work/machete.txt" 2> "$work/machete.err" || status=$?
        if [ "$status" -ne 0 ] && [ "$status" -ne 1 ]; then
          cat "$work/machete.err" >&2
          printf 'unused-dependencies-rust: cargo machete exited %s over %s\n' "$status" "$manifest" >&2
          exit 1
        fi
        if grep -q '^error when handling ' "$work/machete.err"; then
          cat "$work/machete.err" >&2
          printf 'unused-dependencies-rust: cargo machete could not read %s\n' "$manifest" >&2
          exit 1
        fi
        awk '/^cargo-machete found/ {listing = 1; next}
             listing && /^[[:space:]]*$/ {listing = 0; next}
             listing && /^[[:space:]]/ {gsub(/^[[:space:]]+|[[:space:]]+$/, ""); print; next}
             listing && !/ -- / {listing = 0}' "$work/machete.txt" |
          while IFS= read -r dependency; do
            line="$(grep -nE "^[[:space:]]*\"?${dependency}\"?[[:space:]]*[=.]|^\[[a-z-]*dependencies\.${dependency}\]" "$manifest" | head -1 | cut -d: -f1)"
            printf '%s:%s: unused dependency `%s`: no source file of this package names it; delete it, or list it under `[package.metadata.cargo-machete] ignored` with a comment saying why\n' "$manifest" "${line:-1}" "$dependency"
          done
      done
  doctor:
    check_command: "which cargo cargo-machete find grep awk mktemp head cut sort tr mkdir cp cat"
    check_version_command: "cargo-machete --version"
  install:
    commands: ["cargo install cargo-machete@0.9.2 --locked"]
---

# Unused Dependencies — Rust

`cargo machete` reports every dependency a package declares that no source file
of that package names. It parses each `.rs` file of the package with `syn` and
collects the crate identifiers the code actually refers to, then subtracts them
from the manifest's dependency tables.

The claim is one sentence long and it is a fact about the package, not a
judgment: the dependency is written in the manifest and nothing compiles
against it.

## What the marker is, and that it works

Write the dependency into the manifest's own ignore list:

    [package.metadata.cargo-machete]
    # `regex-automata` is here so the `env-filter` feature can turn it on;
    # nothing in the source names it.
    ignored = ["regex-automata"]

Measured against machete 0.9.2 on a probe crate that uses `libc` and declares
an unused `serde`: the bare crate reports `serde`, and the same crate carrying
`ignored = ["serde"]` reports nothing.

The reason goes in a TOML comment, and a comment on the same line as the array
works: `ignored = ["serde"] # feature-only dependency` still suppresses.
Measured, because a suppression that silently stops working when a reason is
added is a real failure mode — periphery's `// periphery:ignore` behaves that
way, and `dead-code-swift` had to say so. TOML comments are part of the
grammar, so this marker does not.

This is the only exemption. Selection in the pipe is attribution, not
exemption: to keep one dependency, name it in `ignored` with a comment; to keep
it out of the report any other way, delete it.

Unlike `#[expect(dead_code)]`, this marker does not expire. Cargo has no
"unused ignore" warning, so an entry outlives the reason it was written for
unless a reader removes it. Write the comment for that reader.

## The mode this rule runs, and the mode it does not

The default mode, which reads source text. **Not `--with-metadata`.**

`--with-metadata` resolves dependency names through `cargo metadata`, which
also lets it write to `Cargo.lock`. The earlier verdict recorded in
`code-hygiene`'s `VALIDATOR.md` measured that mode and found it reports
`tauri-build` unused for `kanban-app` and for `mirdan-app`, whose build scripts
both call `tauri_build::build()`. The default mode does not: neither app
appears anywhere in this workspace's findings.

The default mode's own weak spot is the mirror image — a dependency renamed in
the manifest, or one that exists only to turn a feature on, is named by no
source file, so it is reported. That is what the ignore list is for, and the
measurement below says how often it comes up.

## The measurement

| Tree | Findings | Time |
|---|---|---|
| this workspace, 63 package manifests | **126** across 37 packages | 2 s |
| `BurntSushi/ripgrep` at HEAD | **1** | under 1 s |
| `tokio-rs/tracing` at HEAD | **6** | under 1 s |

Twelve of this workspace's findings were hand-checked and every one is real:
`swissarmyhammer-common`'s `indicatif`, named only inside a doc comment;
`swissarmyhammer-validators`' `chrono` and `sha2`, whose only textual hits are
the word "sync**hrono**us"; `swissarmyhammer`'s `serde`, `tokio`, `toml` and
`anyhow`, in a package holding three `.rs` files against some forty
dependencies; `model-loader`'s `sha2`; `swissarmyhammer-fields`' `ulid`; and
`swissarmyhammer-git`'s `anyhow`, `async-trait` and `tokio`. No false positive
in the sample.

ripgrep's one finding is `grep-index`'s `fst`, and no `.rs` file of that crate
names it. Five of tracing's six are the same shape.

The sixth is the false positive this rule ships with, and it is worth stating
exactly: `tracing-subscriber` declares `regex-automata` as an optional
dependency that the `env-filter` feature turns on with `dep:regex-automata`, so
that `matchers::BuildError` can implement `std::error::Error`. No source names
it, and deleting it breaks the feature. One finding in seven external ones, on
a shape the ignore list is built for. That rate is the price of the mode that
does not misreport build scripts.

## How the run is shaped

The scope is `workspace`. Machete answers a question about a whole package —
which of its sources name which of its dependencies — so it cannot be handed a
loose file, and the engine keeps only the findings in the changed files.

The script's unit of work is one package manifest, discovered as a `*.toml`
file declaring a `[package]` table. That definition, rather than the name
`Cargo.toml`, is what lets the doctor fixtures be manifests: this workspace
holds no `*.toml` outside a `Cargo.toml` that declares a package, so the two
definitions pick out the same 63 files here.

Machete reads only a file literally named `Cargo.toml` — measured: handed
`./renamed.toml`, a copy of a manifest that reports one unused dependency, it
reports nothing. A manifest under any other name is therefore copied into a
temporary package, beside the `*.rs` files of its own directory, and the
findings are mapped back onto the path the script started from. That is the
pattern `builtin/validators/README.md` states for a tool that reads its input
by convention rather than by flag.

A manifest already named `Cargo.toml` is scanned where it lies, and it has to
be. A workspace member cannot be copied out of its workspace: `cargo machete`
on a detached copy of `swissarmyhammer-fields/Cargo.toml` fails with "can't
load root workspace" and reports nothing, because `version.workspace = true`
has no root to inherit from.

The path handed to machete always carries a directory, `$dir/Cargo.toml`, never
the bare name. Machete derives the package directory from the parent of the path
it is given, so `cargo machete Cargo.toml` derives an empty parent and fails with
"can't load root workspace at :" — and then, having failed, still exits 0 and
prints "didn't find any unused dependencies". A single-crate repository, whose
only manifest sits at the root, would silently report nothing. Caught by the
acceptance test that runs this rule over a real one-package repository.

Machete prints no line number, so the script finds one: the first line of the
manifest whose key is the dependency. Both spellings resolve —
`ulid = { workspace = true }` and `walkdir.workspace = true` — as does a
`[dependencies.<name>]` table header. A dependency whose key the pattern cannot
find still reports, on line 1.

One machete process runs for each manifest rather than one for the whole tree,
which costs 2 s over this workspace against 0.8 s for a single whole-tree
run. That buys the uniform per-manifest path above, and it is well under the
6.7 s the `dead-code-rust` orphan scan already spends on the same tree.

## The script names the binary, not the cargo subcommand

`cargo machete <path>` rewrites its own argument list when the environment
carries `CARGO_PKG_NAME`, and cargo exports that name to every process it runs —
a build script, a `cargo run` binary, and every test binary. The subcommand name
then arrives as a PATH of its own. Measured with machete 0.9.2 over one probe
package that declares an unused `serde`:

| the command | environment | the paths machete read | status |
|---|---|---|---|
| `cargo machete ./Cargo.toml` | a plain shell | `./Cargo.toml` | 1, one finding |
| `cargo machete ./Cargo.toml` | `CARGO_PKG_NAME` set | `machete,./Cargo.toml` | 2, one finding and an error |
| `cargo-machete ./Cargo.toml` | either | `./Cargo.toml` | 1, one finding |
| `cargo machete --version` | `CARGO_PKG_NAME` set | `machete,--version` | 2, no version |
| `cargo-machete --version` | either | — | 0, `0.9.2` |

The second row is the shape a status gate has to answer for: machete wrote the
findings of the real path to stdout and then failed on the phantom path
`machete`, so the status says broken while the report says measured. The binary
carries no such ambiguity, so the script runs `cargo-machete` and
`doctor.check_version_command` reads `cargo-machete --version`. `check_command`
already asked `which` for that same name.

The row was found by the acceptance tests of this rule, which run inside a cargo
test binary and therefore carry `CARGO_PKG_NAME`.

## A manifest the tool could not read

Machete keeps one status for findings and another for a failure, and it also
answers a per-manifest failure at the status of a CLEAN run. Measured with
machete 0.9.2:

| the shape | status | stdout | stderr |
|---|---|---|---|
| one unused dependency | 1 | `cargo-machete found the following unused dependencies` | `Analyzing…`, `Done!` |
| no unused dependency | 0 | `didn't find any unused dependencies` | the same |
| a path that holds no file | 2 | nothing | `Error: Errors when walking over directories` |
| a manifest that does not parse as TOML | 0 | `didn't find any unused dependencies` | `error when handling <path>: TOML parse error` |
| a workspace member detached from its root | 0 | the same sentence | `error when handling <path>: can't load root workspace` |
| the bare name `Cargo.toml` | 0 | the same sentence | `error when handling Cargo.toml: can't load root workspace at :` |

The first two rows are measured runs, and the four under them are broken runs.
Status alone cannot tell them apart, because three of the four broken shapes
exit 0 and write the sentence a clean package writes. So the script makes two
tests for each manifest, and `builtin/validators/README.md` states both: "Where
the tool has a failure status of its own, run it into a file, test the status
against the findings status, and exit nonzero yourself", and "A failure status
and a clean answer can share a report... The script must then test STDERR".

- a status that is neither 0 nor 1 —
  `unused-dependencies-rust: cargo machete exited <status> over <manifest>`;
- an `error when handling ` line on stderr —
  `unused-dependencies-rust: cargo machete could not read <manifest>`.

Machete's own stderr is written beside each line, so the diagnosing agent reads
what machete said. `set -e` makes the `exit 1` inside the manifest loop the exit
status of the whole script: the loop stands in a pipeline, so it runs in a
subshell, and without `set -e` its exit would end the subshell alone.

An earlier shape of this script ended each manifest in a pipe, which took the
status of its LAST command and dropped machete's own. Measured over a manifest
that does not parse as TOML: that shape reported nothing and exited 0, and the
review read the package as clean; the shipped shape reports nothing and exits 1.
Measured over the same probe package with machete replaced by a command that
exits 127: the pipe shape wrote 0 findings and exited 0; the shipped shape exits
1 and names the status. The two acceptance tests
`the_shipped_rust_unused_dependency_tool_rule_breaks_on_a_manifest_it_cannot_read`
and `the_shipped_rust_unused_dependency_tool_rule_breaks_when_machete_cannot_run`
hold both answers.

Machete's exit 1 on findings is the status of a MEASURED run, and the script
takes it as one. The acceptance test
`the_shipped_rust_unused_dependency_tool_rule_reports_an_unused_dependency`
holds that half: a package with one unused dependency reports it and the run
does not break.

Measured over this whole workspace, the shipped script and the earlier pipe
answer alike: the same 126 findings across 37 packages, exit 0, in 2 s.
