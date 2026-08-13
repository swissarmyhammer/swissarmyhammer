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
    set -e
    work="$(mktemp -d)"
    trap 'rm -rf "$work"' EXIT
    status=0
    cargo check --workspace --all-targets --message-format=json --quiet \
      > "$work/check.json" || status=$?
    filtered=0
    jq -r 'select(.reason == "build-finished") | "ran"' "$work/check.json" \
      > "$work/ran.txt" || filtered=$?
    jq -r 'select(.reason == "compiler-message")
           | .message
           | select(.level == "error")
           | (.code.code // "")
           | select(. == "" or test("^E[0-9]+$"))' "$work/check.json" \
      > "$work/rustc-errors.txt" || filtered=$?
    jq -r 'select(.reason == "compiler-message")
           | .message
           | select(.level == "error")
           | "an error"' "$work/check.json" \
      > "$work/errors.txt" || filtered=$?
    jq -s -r '[.[] | select(.reason == "compiler-artifact")
                   | select(.target.kind | index("custom-build"))
                   | .package_id]
              - [.[] | select(.reason == "build-script-executed") | .package_id]
              | .[]' "$work/check.json" \
      > "$work/unrun-build-scripts.txt" || filtered=$?
    jq -c 'select(.reason == "compiler-message")
           | .message
           | select(.code.code == "dead_code")
           | select(.spans | length > 0)
           | {file: .spans[0].file_name, line: .spans[0].line_start, message: .message}
           | select(.file | startswith("/") | not)' "$work/check.json" \
      > "$work/findings.json" || filtered=$?
    if [ "$filtered" -ne 0 ]; then
      printf 'dead-code-rust: jq could not read the cargo report\n' >&2
      exit 1
    fi
    if [ "$status" -ne 0 ] && [ ! -s "$work/ran.txt" ]; then
      printf 'dead-code-rust: cargo could not check the workspace\n' >&2
      exit 1
    fi
    if [ -s "$work/rustc-errors.txt" ]; then
      printf 'dead-code-rust: cargo could not compile the workspace\n' >&2
      exit 1
    fi
    if [ -s "$work/unrun-build-scripts.txt" ]; then
      printf 'dead-code-rust: a build script did not run, so cargo did not check every crate\n' >&2
      exit 1
    fi
    if [ "$status" -ne 0 ] && [ ! -s "$work/errors.txt" ]; then
      printf 'dead-code-rust: cargo stopped the build and wrote no compiler error\n' >&2
      exit 1
    fi
    sort -u "$work/findings.json"
    mkdir -p "$work/modules" "$work/paths"
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
        if [ ! -f "$work/modules/$key" ]; then
          find "$crate" -name target -prune -o -name .git -prune -o -name '*.rs' \
            -exec awk '
              function emit(nm,   segs, s) {
                if (nstack == 0) { print "s " nm; return }
                segs = ""
                for (s = 1; s <= nstack; s++) segs = segs stack[s] "/"
                print "p " pfx stem "/" segs nm ".rs"
                print "p " pfx stem "/" segs nm "/mod.rs"
                print "p " pfx segs nm ".rs"
                print "p " pfx segs nm "/mod.rs"
              }
              function stems(t, pat,   v) {
                while (match(t, pat)) {
                  v = substr(t, RSTART, RLENGTH)
                  t = substr(t, RSTART + RLENGTH)
                  sub(/^[^"]*"/, "", v)
                  sub(/"$/, "", v)
                  sub(/.*\//, "", v)
                  sub(/\.rs$/, "", v)
                  if (v != "") print "s " v
                }
              }
              function scan(t,   j, m, ch, nm) {
                j = 1
                m = length(t)
                while (j <= m) {
                  if (match(substr(t, j), /^[^{};m]+/)) { j += RLENGTH; continue }
                  ch = substr(t, j, 1)
                  if (ch == "{") {
                    depth++
                    if (pend != "") { nstack++; stack[nstack] = pend; opened[nstack] = depth; pend = "" }
                    j++
                    continue
                  }
                  if (ch == "}") {
                    if (nstack > 0 && opened[nstack] == depth) nstack--
                    depth--
                    j++
                    continue
                  }
                  if (ch == ";") { pend = ""; j++; continue }
                  if ((j == 1 || substr(t, j - 1, 1) !~ /[A-Za-z0-9_]/) &&
                      match(substr(t, j), /^mod[ \t]+[A-Za-z_][A-Za-z0-9_]*/)) {
                    nm = substr(t, j, RLENGTH)
                    sub(/^mod[ \t]+/, "", nm)
                    emit(nm)
                    pend = nm
                    j += RLENGTH
                    continue
                  }
                  j++
                }
              }
              BEGIN {
                quote = sprintf("%c", 39)
                path_pattern = "#\\[[ \t]*path[ \t]*=[ \t]*\"[^\"]*\""
                include_pattern = "include![ \t]*\\([ \t]*\"[^\"]*\""
                plain_pattern = "^[^\"" quote "/rb]+"
              }
              FNR == 1 {
                instr = 0
                inraw = 0
                inblk = 0
                rawh = 0
                hashes = ""
                depth = 0
                nstack = 0
                pend = ""
                name = FILENAME
                sub(/^\.\//, "", name)
                stem = name
                sub(/^.*\//, "", stem)
                sub(/\.rs$/, "", stem)
                pfx = name
                if (pfx ~ /\//) { sub(/\/[^\/]*$/, "", pfx); pfx = pfx "/" } else pfx = ""
              }
              {
                line = $0
                code = ""
                bare = ""
                cont = (instr || inraw)
                i = 1
                n = length(line)
                while (i <= n) {
                  if (inblk) {
                    if (substr(line, i, 2) == "*/") { inblk = 0; i += 2; continue }
                    if (match(substr(line, i), /^[^*]+/)) i += RLENGTH; else i++
                    continue
                  }
                  if (inraw) {
                    c = substr(line, i, 1)
                    if (c == "\"" && substr(line, i + 1, rawh) == hashes) {
                      inraw = 0
                      if (!cont) code = code substr(line, i, 1 + rawh)
                      i += 1 + rawh
                      cont = 0
                      continue
                    }
                    if (match(substr(line, i), /^[^"]+/)) {
                      if (!cont) code = code substr(line, i, RLENGTH)
                      i += RLENGTH
                    } else i++
                    continue
                  }
                  if (instr) {
                    c = substr(line, i, 1)
                    if (c == "\\") {
                      if (!cont) code = code substr(line, i, 2)
                      i += 2
                      continue
                    }
                    if (c == "\"") {
                      instr = 0
                      if (!cont) code = code c
                      i++
                      cont = 0
                      continue
                    }
                    if (match(substr(line, i), /^[^"\\]+/)) {
                      if (!cont) code = code substr(line, i, RLENGTH)
                      i += RLENGTH
                    } else i++
                    continue
                  }
                  if (match(substr(line, i), plain_pattern)) {
                    code = code substr(line, i, RLENGTH)
                    bare = bare substr(line, i, RLENGTH)
                    i += RLENGTH
                    continue
                  }
                  c = substr(line, i, 1)
                  two = substr(line, i, 2)
                  if (two == "//") break
                  if (two == "/*") { inblk = 1; i += 2; continue }
                  if (c == "\"") { instr = 1; code = code c; i++; continue }
                  if (c == quote) {
                    if (substr(line, i + 1, 1) == "\\") {
                      k = i + 3
                      while (k <= n && substr(line, k, 1) != quote) k++
                      i = k + 1
                      continue
                    }
                    if (substr(line, i + 2, 1) == quote) { i += 3; continue }
                    code = code c
                    bare = bare c
                    i++
                    continue
                  }
                  if (i == 1 || substr(line, i - 1, 1) !~ /[A-Za-z0-9_]/) {
                    k = i
                    if (c == "b") k++
                    if (substr(line, k, 1) == "r") {
                      k++
                      h = 0
                      while (substr(line, k, 1) == "#") { h++; k++ }
                      if (substr(line, k, 1) == "\"") {
                        inraw = 1
                        rawh = h
                        hashes = ""
                        while (length(hashes) < h) hashes = hashes "#"
                        code = code substr(line, i, k - i + 1)
                        i = k + 1
                        continue
                      }
                    }
                  }
                  code = code c
                  bare = bare c
                  i++
                }
                scan(bare)
                stems(code, path_pattern)
                stems(code, include_pattern)
              }
            ' {} + >"$work/index"
          awk '$1 == "s" { print substr($0, 3) }' "$work/index" | sort -u >"$work/modules/$key"
          awk '$1 == "p" { print substr($0, 3) }' "$work/index" | sort -u >"$work/paths/$key"
        fi
        grep -qxF "${base%.rs}" "$work/modules/$key" && continue
        grep -qxF "${file#./}" "$work/paths/$key" && continue
        grep -qE '^[[:space:]]*//[/!]?[[:space:]]*sah:ignore[[:space:]]+orphan-module[[:space:]]+[^[:space:]]' "$file" && continue
        printf '%s:1: orphan module: no `mod %s;`, `#[path]` or `include!` in this crate names this file, so nothing compiles it — name it, delete it, or write `// sah:ignore orphan-module <reason>` in it\n' "${file#./}" "${base%.rs}"
      done
  doctor:
    check_command: "which cargo jq find grep awk sort tr mktemp mkdir"
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
however dead it is. The second half of the script closes that hole with an
`awk` index of every name the crate declares.

A file is exempt when it is a crate root or a module root by name — `lib.rs`,
`main.rs`, `mod.rs`, `build.rs` — or when it sits in a directory cargo compiles
file by file: `tests/`, `benches/`, `examples/`, and `src/bin/`, where every
file is its own target root. Everything else must be named by a
`mod <stem>;` declaration, by a `#[path = "..."]` attribute, or by an
`include!("...")`, somewhere in its own crate. The crate is the nearest ancestor
directory holding a `Cargo.toml`, so a `mod` declaration in a sibling crate does
not excuse a file.

### `include!` compiles a file no declaration names

`include!("foo.rs")` pastes a file into the module that holds the macro. That
file is compiled, and no `mod` declaration and no `#[path]` attribute anywhere
names it. The line this rule writes claims that nothing compiles the file, and
for such a file the claim is false. Measured over a probe package holding
`include!("generated.rs")` beside `src/generated.rs`, against the earlier index:
the run reported `src/generated.rs:1` for a file the compiler reads. The index
now reads the string of each `include!` the same way it reads the string of each
`#[path]`, and the run over that probe reports nothing.

### The index reads the code, never a comment and never a string

The earlier index came from
`grep -rhoE '\bmod[[:space:]]+[A-Za-z_][A-Za-z0-9_]*'`, which matched the word
wherever it stood. A `mod` inside a comment and a `mod` inside a string literal
each entered the index, and each excused a real orphan. Both are silent
failures: the scan reports nothing and the dead file stays.

One `awk` pass now lexes each `.rs` file of the crate. It drops a `//` comment
and a `/* */` block, and it keeps the body of a string literal out of the text
the `mod` scan reads — an ordinary string, a raw string of any hash count, and a
string that runs over several lines alike. A character literal is read as one,
so a `'{'` does not move the brace depth. The `#[path]` and `include!` scans read
the text WITH its strings, because the payload of each of those IS a string.

Measured over three probe packages that name the orphan in a line comment, in a
block comment at column zero, and in a `pub const` string: the earlier index
reported 0 findings for each, and this index reports the orphan.

### A nested declaration names a file under its own module directory

A `mod x;` inside an inline module does not name `x.rs` beside the file. It
names `<owner>/<inline module>/x.rs`, because an inline module adds its own name
to the module directory. The earlier index held bare stems, so a `mod orphan;`
inside a `#[cfg(test)] mod tests` excused every `orphan.rs` of the crate.
Measured over a probe package holding `src/orphan.rs` beside the
`src/tests/orphan.rs` that declaration really names: the earlier index reported
0 findings, and the real orphan went missing.

A declaration at the TOP LEVEL of a file still enters the index as a bare stem,
which is the coarse shape this rule shipped with. A NESTED declaration enters it
as a resolved PATH instead. The path is written two ways, because whether a file
is a crate root is cargo's decision and not a fact the file name carries:
`<dir>/<stem>/<segments>/x.rs` for a file that is a module of its own, and
`<dir>/<segments>/x.rs` for a file that is a target root, each beside its
`mod.rs` form. A file any of those four names is exempt, so the scan never
accuses a file a plausible resolution names.

This needs no `cfg(test)` test of its own. The nesting is what moves the
directory, and `#[cfg(test)] mod tests` is one inline module among many.

### The marker, for what the scan cannot read

A crate can compile a file through a `mod` name a macro builds, through an
`include!` whose path is an expression such as
`include!(concat!(env!("OUT_DIR"), "/gen.rs"))`, or through a build script. The
scan reads none of those. `builtin/validators/README.md` states the answer for
that residue: "An exemption a person would argue for in prose must become an
inline suppression the tool reads."

Write the marker in the file itself, on a comment line:

    // sah:ignore orphan-module <reason>

The reason is required, and it names what compiles the file. Measured over two
probe packages: the marker beside a reason reports nothing, and the same marker
with nothing after it reports the orphan, because a claim with no subject
suppresses nothing. A `///` and a `//!` carry the marker too, so a file that
opens with documentation needs no comment of another shape.

Naming the file is still the fix. The marker is the claim that something the
scan cannot read already names it.

### What the scan costs, and what it reports

The index is built one time for each crate rather than one time for each file,
and it stands under `modules/` and `paths/` in the one temporary directory the
script makes. Measured over this whole workspace, 1219 `.rs` files, as the
lowest of three runs of each: the `awk` index takes **11.3 s** against **7.9 s**
for the earlier `grep` index.

Measured on this workspace when the rule shipped: **5** orphan files, every one
hand-checked and every one real — `crates/swissarmyhammer/src/security.rs`
(15 KB of code nothing compiles), `crates/markdowndown/src/error.rs`,
`crates/markdowndown/src/fetch.rs`,
`crates/swissarmyhammer-common/src/sample_avp_test.rs`, and
`crates/swissarmyhammer-tools/src/mcp/notifications.rs` (an empty file). No
false positive. Each of the five has since gone, and both indexes report **0**
orphan files over this workspace today, so the `awk` index adds no false
positive here.

Seven acceptance tests hold this half to the measurements above:
`the_shipped_rust_dead_code_tool_rule_reports_a_file_no_declaration_names`,
`the_shipped_rust_dead_code_tool_rule_keeps_a_file_an_include_compiles`,
`the_shipped_rust_dead_code_tool_rule_reads_the_orphan_marker`,
`the_shipped_rust_dead_code_tool_rule_reports_an_orphan_whose_marker_states_no_reason`,
`the_shipped_rust_dead_code_tool_rule_reads_no_module_declaration_from_a_comment`,
`the_shipped_rust_dead_code_tool_rule_reads_no_module_declaration_from_a_string_literal`
and
`the_shipped_rust_dead_code_tool_rule_reads_a_nested_module_declaration_as_its_own_file`.

## A workspace the tool cannot check

`cargo check` exits nonzero for four different reasons, and one status carries
all four:

- cargo could not start a run at all.
- cargo made a run, and a crate failed to compile. The lint runs after that
  compilation, so cargo never ran it over that crate.
- cargo made a run, and the BUILD SCRIPT of a crate broke. cargo runs a build
  script before it compiles the crate that script serves, so that crate was
  never checked. This repository holds fifteen build scripts.
- cargo made a run, checked every crate, and a lint stands at deny level.

The first three are broken runs. The fourth is a MEASURED run, and the findings
it holds must stand. `builtin/validators/README.md` states the answer for this
shape: "One status can carry both a measured run and a broken run. The status of
a failure is then the same as the status of a finding. The script must then test
the REPORT beside the status, and accept the shared status only for the report
shape a measured run writes." The sibling `complexity-rust` makes the same four
tests over the same report.

The deny-level shape is not a corner case for THIS rule. Under
`RUSTFLAGS="-D warnings"` a `dead_code` diagnostic itself arrives at level
`error` and cargo exits 101, so a gate that read the status alone would break
the run exactly when the rule has a finding. Measured with cargo 1.97.1 over one
package holding one dead item: the raw report holds the error code `dead_code`,
the filter selects on the CODE and keeps the finding, and the run answers 1
finding at exit 0.

The script therefore reads the RAW report, which carries what the filter drops,
and breaks the run in four places:

- the status is nonzero and the raw report holds no `build-finished` entry —
  `dead-code-rust: cargo could not check the workspace`;
- the raw report holds an error-level message with a rustc code or with no code
  — `dead-code-rust: cargo could not compile the workspace`;
- the raw report holds a `custom-build` artifact whose package writes no
  `build-script-executed` entry —
  `dead-code-rust: a build script did not run, so cargo did not check every crate`;
- the status is nonzero and the raw report holds no error-level message at all
  — `dead-code-rust: cargo stopped the build and wrote no compiler error`.

Each of the five `jq` calls writes to a file and tests its own status, because
the script writes `set -e` with no `pipefail` and a pipeline takes the status of
its LAST command. A status other than 0 writes `dead-code-rust: jq could not
read the cargo report` to stderr and exits 1.

Every shape below was measured with cargo 1.97.1. Each package that holds a
finding holds one private `fn unused_helper` nothing reaches.

| the shape | status | `build-finished` | error codes | the run answers |
|---|---|---|---|---|
| a healthy package, one dead item | 0 | `success: true` | none | 1 finding, exit 0 |
| a package that does not parse: `pub struct Undocumented` | 101 | `success: false` | no code | 0 findings, exit 1 |
| a workspace of two members, one that does not parse beside one dead item | 101 | `success: false` | no code | 0 findings, exit 1 |
| `[lints.rust] unused_variables = "deny"` beside one dead item | 101 | `success: false` | `unused_variables` | 1 finding, exit 0 |
| `RUSTFLAGS="-D warnings"` beside one dead item | 101 | `success: false` | `dead_code` | 1 finding, exit 0 |
| a package whose build script breaks | 101 | `success: false` | none | 0 findings, exit 1 |
| the same package under a build script that runs | 0 | `success: true` | none | 1 finding, exit 0 |
| a directory that holds no `Cargo.toml` | 101 | none, 0 bytes | none | 0 findings, exit 1 |
| `jq` replaced by a command that exits 127 | 0 | `success: true` | none | 0 findings, exit 1 |

The earlier shape of this script was one pipe that ended in `sort -u`. Measured
over each of the four broken rows: the pipe wrote no finding and exited 0, and
the engine read the whole tree as clean; the `jq` row wrote the orphan half
alone and exited 0, so the whole `dead_code` half went missing without a word.

The two-member row is the shape `scope: workspace` makes reach a real
repository, and this repository holds more than 20 members. The member that
compiles fills the findings file, so a gate that read that file would never
reach its status test.

Measured over this whole workspace, the shipped script and the earlier pipe
answer alike: 0 findings, exit 0, and the same bytes on stdout.

Six acceptance tests hold the script to this table:
`the_shipped_rust_dead_code_tool_rule_breaks_on_a_crate_that_does_not_compile`,
`the_shipped_rust_dead_code_tool_rule_breaks_on_a_workspace_member_it_cannot_compile`,
`the_shipped_rust_dead_code_tool_rule_breaks_on_a_build_script_that_breaks`,
`the_shipped_rust_dead_code_tool_rule_measures_a_package_beside_a_build_script_that_runs`,
`the_shipped_rust_dead_code_tool_rule_measures_a_workspace_beside_a_deny_level_lint`
and
`the_shipped_rust_dead_code_tool_rule_breaks_when_the_filter_cannot_read_the_report`.

The `trap` removes the temporary directory when the script exits. It covers a
clean run, a run with findings and a broken run alike, and it leaves the exit
status of the script alone.

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
