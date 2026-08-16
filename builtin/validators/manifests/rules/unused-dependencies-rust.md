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
    decline() {
      printf 'sah-diagnostic: cargo machete could not read %s: %s\n' "$1" "$2" >&2
    }
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
        if [ "$status" -eq 2 ] && grep -q '^Error: Errors when walking over directories' "$work/machete.err"; then
          reason="$(awk '/^Error: Errors when walking over directories/ {walked = 1; next}
                         walked {print; exit}' "$work/machete.err")"
          decline "$manifest" "${reason#"$scan": }"
          continue
        fi
        if [ "$status" -ne 0 ] && [ "$status" -ne 1 ]; then
          cat "$work/machete.err" >&2
          printf 'unused-dependencies-rust: cargo machete exited %s over %s\n' "$status" "$manifest" >&2
          exit 1
        fi
        if grep -q '^error when handling ' "$work/machete.err"; then
          reason="$(grep '^error when handling ' "$work/machete.err" | head -1)"
          reason="${reason#error when handling }"
          decline "$manifest" "${reason#"$scan": }"
          continue
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
| this workspace, 63 package manifests | **122** across 36 packages | 1.3 s |
| `BurntSushi/ripgrep` at HEAD | **1** | under 1 s |
| `tokio-rs/tracing` at HEAD | **6** | under 1 s |

Twelve of this workspace's findings were hand-checked when the rule landed and
every one was real: `swissarmyhammer-common`'s `indicatif`, named only inside a
doc comment; `swissarmyhammer-validators`' `chrono` and `sha2`, whose only
textual hits are the word "sync**hrono**us"; `swissarmyhammer`'s `serde`,
`tokio`, `toml` and `anyhow`, in a package holding three `.rs` files against
some forty dependencies; `model-loader`'s `sha2`; `swissarmyhammer-fields`'
`ulid`; and `swissarmyhammer-git`'s `anyhow`, `async-trait` and `tokio`. No
false positive in the sample. Ten of the twelve still stand today; the two
`swissarmyhammer-validators` ones are gone from the tree, which is the rule
doing its work and is why the count above moved.

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
`Cargo.toml`, is what lets the doctor fixtures be manifests.

The two readings do not pick out the same files. Counted over this workspace:
68 `*.toml` files, 64 of them named `Cargo.toml`, and 63 declaring a package.
The four under another name — `.config/nextest.toml`, `dist-workspace.toml`,
`.cargo/config.toml` and `doc/book.toml` — declare none. The one `Cargo.toml`
that declares none is the virtual workspace root, which carries a `[workspace]`
table and no package; handed `./Cargo.toml` at that root, machete answers
`didn't find any unused dependencies` at exit 0. So the `[package]` reading
takes 63 files, the name reading would take 64, and the one file between them
is a manifest machete has nothing to say about.

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
it is given, so `cargo machete Cargo.toml` derives an empty parent and fails to
load a root workspace — and then, having failed, still exits 0 and prints
"didn't find any unused dependencies". A single-crate repository, whose only
manifest sits at the root, would silently report nothing. Caught by the
acceptance test that runs this rule over a real one-package repository. The
sentence it fails with is under "A manifest the tool could not read" below.

Machete prints no line number, so the script finds one: the first line of the
manifest whose key is the dependency. Both spellings resolve —
`ulid = { workspace = true }` and `walkdir.workspace = true` — as does a
`[dependencies.<name>]` table header. A dependency whose key the pattern cannot
find still reports, on line 1.

One machete process runs for each manifest rather than one for the whole tree,
which costs 1.3 s over this workspace against 0.3 s for a single whole-tree
run. Measured over 30 warm samples of each, run alternately so that both met the
same machine: 1.32 s to 1.49 s for the script and 0.26 s to 0.42 s for the
whole-tree run, median 1.34 s and 0.27 s. Both tails belong to whatever else the
machine was doing, so read the medians. That buys the uniform per-manifest path
above, and it is well under the 6.7 s the `dead-code-rust` orphan scan already
spends on the same tree. It buys one thing more: a manifest machete refuses is
one process of 63, so the other 62 keep their answers.

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

Machete keeps one status for findings and another for a failure, and a
per-manifest failure comes back under EITHER: at the status of a clean run when
it could read the path and not the manifest, and at its failure status when it
could not walk the path at all. Measured with machete 0.9.2:

| the shape | status | stdout | stderr |
|---|---|---|---|
| one unused dependency | 1 | `cargo-machete found the following unused dependencies` | `Analyzing…`, `Done!` |
| no unused dependency | 0 | `didn't find any unused dependencies` | the same |
| a manifest that does not parse as TOML | 0 | `didn't find any unused dependencies` | `error when handling <path>: TOML parse error` |
| a workspace member detached from its root | 0 | the same sentence | `error when handling <path>: can't load root workspace` |
| the bare name `Cargo.toml` | 0 | the same sentence | `error when handling Cargo.toml: can't load root workspace` |
| a path that holds no file | 2 | nothing | `Error: Errors when walking over directories:`, then `<path>: IO error for operation on <path>: No such file or directory (os error 2)` |
| a manifest inside a directory with mode 000 | 2 | nothing | the same two lines, ending `Permission denied (os error 13)` |
| a `Cargo.toml` that is a broken symbolic link | 2 | nothing | the same two lines, ending `No such file or directory (os error 2)` |

The first two rows are measured runs, and the six under them are manifests
machete did not measure. Status alone tells apart neither group: three of the
six exit 0 and write the sentence a clean package writes, and the other three
share their status with every other error machete answers, because `main` maps
every `Err` of `run_machete` to one `Error: ` line and exit 2. So the script
makes two tests for each manifest, and `builtin/validators/README.md` states
both: "Where the tool has a failure status of its own, run it into a file, test
the status against the findings status, and exit nonzero yourself", and "A
failure status and a clean answer can share a report... The script must then
test STDERR".

The `can't load root workspace` sentence takes one more segment when the
manifest declares a `[workspace]` table of its own: `cargo_toml` then writes the
root it looked for, and a bare name has an empty parent, so the line reads
`can't load root workspace at : No such file or directory (os error 2)`.
Measured both ways over the bare name, with the table and without it. The
acceptance probes carry `[workspace]`, so the ` at :` form is the one they meet.

The two tests answer three readings, because the readings say three different
things about the RUN.

### A walk failure — one item declined

`run_machete` collects a walk failure for each path it was handed and bails
after the loop with `Errors when walking over directories`. Those failures are
PER INVOKED PATH: the path that failed is analysed for nothing, and any other
path of the same process is measured normally. A script that runs one machete
process for each manifest therefore reads this failure for ONE manifest, and one
manifest of a run that measured the rest is ONE declined item.

Measured against the real binary, one process for each path, over four
constructions: a path that holds no file, a manifest inside a directory with
mode 000, a `Cargo.toml` that is a broken symbolic link, and a `Cargo.toml`
under a macOS ACL that denies `readattr` while leaving the bytes readable. All
four exit 2 and write `Error: Errors when walking over directories:` with one
line under it naming the path.

The fourth is the one the script itself reaches. The first three never get that
far: `find` lists neither a path that holds no file nor anything inside a
directory with mode 000, and the script's own `grep -q '^\[package\]'` guard
fails on a broken symbolic link before the loop calls the tool. An ACL that
denies `readattr` alone lets `find` list the file and `grep` read its bytes
while the walk still fails on the stat. Driven end to end through the script's
own loop, with a package declaring an unused `serde` staged before it:

| the script | stdout | stderr | exit |
|---|---|---|---|
| the earlier shape | the `serde` finding | machete's 4 raw lines, then `unused-dependencies-rust: cargo machete exited 2 over zwalkfail/Cargo.toml` | 1 |
| the shipped shape | the `serde` finding | 1 marked line naming `zwalkfail/Cargo.toml` | 0 |

The earlier shape lost the finding it had already written: `read_script_output`
answers `Err` for a nonzero exit before it reads stdout at all. Two walk
failures staged alone: no finding, two marked lines, exit 0.

The script declines this shape on machete's own two marks TOGETHER — status 2
AND the `Errors when walking over directories` sentence. Neither alone is
enough. A machete that answers status 2 for another reason judged nothing
either, and it says nothing about which manifests it could still have measured,
so that run breaks.

### An `error when handling ` line — one item declined

This shape is per MANIFEST too. Machete states it, exits the status of a clean
run, and the next manifest gets a machete process of its own that measures
normally. So the script writes a line opening `sah-diagnostic:` and goes on to
the next manifest:

    sah-diagnostic: cargo machete could not read unparsable/Cargo.toml: TOML parse error at line 6, column 14

The reason is machete's own first `error when handling ` line with the prefix
and the path taken off, and the path taken off is the `$scan` value the script
HANDED machete — not whatever stands before the first `: `. Measured with a
package staged at `a: b/Cargo.toml`, where machete writes
`error when handling a: b/Cargo.toml: TOML parse error at line 6, column 14`: a
strip to the first `: ` answered
`could not read a: b/Cargo.toml: b/Cargo.toml: TOML parse error at line 6,
column 14`, repeating the tail of the path inside the reason, and the strip of
`$scan` answers `could not read a: b/Cargo.toml: TOML parse error at line 6,
column 14`. The `$scan` value is quoted inside the pattern, so a path carrying a
glob character is matched as the text it is.

The reason goes INSIDE the marked line because a marked line is the whole of
what reaches the report: at exit 0 the engine keeps the marked lines and drops
everything else a script wrote to stderr as tool chatter. Machete writes
`Analyzing…` and `Done!` on every run, so a raw dump would reach no reader at
all.

An earlier shape of this script exited 1 here instead. Measured with machete
0.9.2 over a probe of two manifests — a package declaring an unused `serde`,
and under it `unparsable/Cargo.toml`, whose `[dependencies` table header never
closes:

| the script | stdout | stderr | exit |
|---|---|---|---|
| the earlier shape | the `serde` finding | machete's 13 raw lines, then `unused-dependencies-rust: cargo machete could not read unparsable/Cargo.toml` | 1 |
| the shipped shape | the `serde` finding | 1 marked line naming `unparsable/Cargo.toml` | 0 |

Both write the same finding, and only one of them reaches a reader with it. A
nonzero exit fails the WHOLE run, so the engine read none of the findings the
earlier shape had already written — which is the answer
`builtin/validators/README.md` refuses: "Do not exit nonzero for a declined
item."

Two more runs of the shipped shape over the same probe. The refusing manifest
ALONE: no finding, the same one marked line, exit 0. A second refusing manifest
staged beside the first: the `serde` finding, two marked lines, exit 0. And one
run of a shape neither table row holds — a renamed manifest whose
`version.workspace = true` loses its root in the copy: the `serde` finding, one
marked line reading `can't load root workspace: ...`, exit 0.

### Any other failing status — the run broke

A status that is neither 0 nor 1, and that machete did not pair with its walk
sentence, is a run that judged nothing and said nothing about how far it got. So
the README asks for a nonzero exit, and machete's own stderr is written beside
the line so the diagnosing agent reads what machete said.

Re-probed over the probe package of the acceptance tests once the walk reading
landed, one row for each way machete can fail:

| the run | stdout | marked lines | the script's own line | exit |
|---|---|---|---|---|
| a stub that exits 127 | nothing | none | `cargo machete exited 127 over Cargo.toml` | 1 |
| `cargo-machete` absent from `PATH` | nothing | none | `cargo machete exited 127 over Cargo.toml` | 1 |
| a stub that exits 2 with no walk sentence | nothing | none | `cargo machete exited 2 over Cargo.toml` | 1 |
| a stub that exits 3 WITH the walk sentence | nothing | none | `cargo machete exited 3 over Cargo.toml` | 1 |

The last row is the walk reading's own guard: the sentence declines at machete's
own failure status alone, so a tool that echoes it at some other status still
breaks the run.

`set -e` makes that `exit 1` the exit status of the whole script: the loop
stands in a pipeline, so it runs in a subshell, and without `set -e` its exit
would end the subshell alone.

### The acceptance tests that hold each reading

Each name opens with `the_shipped_rust_unused_dependency_tool_rule`.

| the test | what it holds |
|---|---|
| `..._reports_an_unused_dependency` | machete's exit 1 on findings is a MEASURED run: the package reports, and the run does not break |
| `..._declines_a_manifest_it_cannot_read` | the finding of the readable manifest AND one diagnostic naming the unparsable one |
| `..._states_the_reason_with_the_path_taken_off` | the whole diagnostic, word for word, over a manifest at `a: b/Cargo.toml` |
| `..._declines_a_manifest_it_cannot_walk` | the same two halves over a machete that exits 2 with the walk sentence |
| `..._breaks_when_machete_fails_over_no_walk` | a status 2 without that sentence breaks the run and places no finding |
| `..._breaks_when_machete_cannot_run` | a machete that cannot run at all breaks the run and places no finding |

The last two are the controls. A fix that answered every failure with a marked
line fails both of them.

Measured over this whole workspace, the shipped shape and the shape before the
walk reading landed answer alike, byte for byte: the same 122 findings across 36
packages, no manifest declined, exit 0, in 1.3 s.
