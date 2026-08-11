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
        cargo machete "$scan" |
          awk '/^cargo-machete found/ {listing = 1; next}
               listing && /^[[:space:]]*$/ {listing = 0; next}
               listing && /^[[:space:]]/ {gsub(/^[[:space:]]+|[[:space:]]+$/, ""); print; next}
               listing && !/ -- / {listing = 0}' |
          while IFS= read -r dependency; do
            line="$(grep -nE "^[[:space:]]*\"?${dependency}\"?[[:space:]]*[=.]|^\[[a-z-]*dependencies\.${dependency}\]" "$manifest" | head -1 | cut -d: -f1)"
            printf '%s:%s: unused dependency `%s`: no source file of this package names it; delete it, or list it under `[package.metadata.cargo-machete] ignored` with a comment saying why\n' "$manifest" "${line:-1}" "$dependency"
          done
      done
  doctor:
    check_command: "which cargo cargo-machete find grep awk mktemp head cut sort tr"
    check_version_command: "cargo machete --version"
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
appears anywhere in this workspace's 141 findings.

The default mode's own weak spot is the mirror image — a dependency renamed in
the manifest, or one that exists only to turn a feature on, is named by no
source file, so it is reported. That is what the ignore list is for, and the
measurement below says how often it comes up.

## The measurement

| Tree | Findings | Time |
|---|---|---|
| this workspace, 64 package manifests | **141** across 40 packages | 2.5 s |
| `BurntSushi/ripgrep` at HEAD | **1** | under 1 s |
| `tokio-rs/tracing` at HEAD | **6** | under 1 s |

Twelve of this workspace's 141 were hand-checked and every one is real:
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
definitions pick out the same 64 files here.

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
which costs 2.5 s over this workspace against 0.8 s for a single whole-tree
run. That buys the uniform per-manifest path above, and it is well under the
6.7 s the `dead-code-rust` orphan scan already spends on the same tree.

The pipe ends in a `while` loop, so machete's exit 1 on findings is not read as
a broken script.
