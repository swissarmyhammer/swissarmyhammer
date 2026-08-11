---
name: magic-numbers-dart
description: Unnamed Dart literals need constants — checked by solid_lints, not by prompt.
match:
  files:
    - "**/*.dart"
  project_types:
    - flutter
supersedes: magic-numbers
tool:
  scope: files
  run: |
    if [ "$#" -eq 0 ]; then
      exit 0
    fi
    work="$(mktemp -d)"
    trap 'rm -rf "$work"' EXIT
    package="$(cd "$work" && pwd -P)"
    printf '%s\n' 'name: sah_magic_numbers_probe' 'environment:' "  sdk: '>=3.5.0 <4.0.0'" \
      'dev_dependencies:' '  custom_lint: 0.8.1' '  solid_lints: 0.3.3' > "$package/pubspec.yaml"
    printf '%s\n' 'analyzer:' '  plugins:' '    - custom_lint' 'custom_lint:' '  rules:' \
      '    - no_magic_number' > "$package/analysis_options.yaml"
    for file in "$@"; do
      copy="$package/lib/${file#/}"
      mkdir -p "$(dirname "$copy")"
      cp "$file" "$copy"
    done
    if ! (cd "$package" && { dart pub get --offline || dart pub get; }) > "$package/pub-get.log" 2>&1; then
      cat "$package/pub-get.log" >&2
      exit 1
    fi
    (cd "$package" && dart run custom_lint --format=json) |
      jq -c --arg prefix "$package/lib/" '
        .diagnostics[] | select(.code == "no_magic_number")
        | {file: (.location.file | ltrimstr($prefix)),
           line: .location.range.start.line,
           message: .problemMessage}'
  doctor:
    check_command: "which dart jq mktemp"
    check_version_command: "dart --version"
    fix_hint: "brew install dart-sdk"
---

# Magic Numbers — Dart

`solid_lints` reports every unnamed numeric literal. The `no_magic_number` rule
names that check. `solid_lints` is a `custom_lint` plugin, so `dart run
custom_lint` runs it, and the rule turns on only the one lint it owns.

The plugin is a dependency of the PROBE PACKAGE this rule writes, never of the
project under review. That is what makes a `custom_lint` rule shippable here:
the script builds the package, states the two versions in its `pubspec.yaml`,
copies the changed files in, and removes the package with the temporary
directory. The project's own `pubspec.yaml` and `analysis_options.yaml` are
read by nothing.

## What the tool already carves out

The `magic-numbers` prompt rule carves out a literal a declaration already
names, and `no_magic_number` honors each form. Measured against a probe file
holding one of each: a top-level `const`, a top-level `final`, a top-level
`var`, a `static const`, a stored field, a mutable field, a local `const`, a
local `final`, a local `var` and a plain local each report nothing. So do a
collection literal, an index expression, a `DateTime` constructor, an
enumeration constant argument, a default parameter value and a `const`
constructor invocation.

The literal that stands INSIDE a declaration's initializer is a finding, and
that is correct: in `final wrapped = List.filled(6004, 0)` the declaration
names `wrapped`, and `6004` is an argument that nothing names. Measured: the
tool reports it.

## The value allow-list cannot be set, so `100` reports

`allowed` is the one value threshold the rule states, and `solid_lints` 0.3.3
cannot read it. Its parameter parser writes
`json['allowed'] as Iterable<num>?`, and `custom_lint` hands it a `YamlList`,
so any `allowed` list makes the plugin answer

    PLUGIN_ERROR ... type 'YamlList' is not a subtype of type 'Iterable<num>?'
    in type cast

and report nothing at all. The rule therefore states no `allowed` key and keeps
the built-in default, which is `[-1, 0, 1]`.

That default is the prompt carve-out list without `100`. `magic-numbers-go`,
`magic-numbers-typescript` and `magic-numbers-swift` each state
`0, 1, -1, 100`, and this rule cannot. So `part * 100` REPORTS, and the recourse
is the inline suppression at the end of this file. `magic-numbers-python` states
the same shape of gap for the same shape of reason.

The fix is in the `solid_lints` source on the main branch — the parser now
reads `allowedRaw is Iterable` — but it is released only in `1.0.0-dev.1`, which
drops `custom_lint_builder` for the `analysis_server_plugin` API. Measured on
Dart 3.11.0: a probe package holding `solid_lints: 1.0.0-dev.1`, with the
plugin declared as the package README states, reports nothing at all under
`dart analyze`, with or without `include: package:solid_lints/analysis_options.yaml`.
The SDK's `dart analyze` command does not load an analyzer plugin, so that
version has no command-line runner yet. When it gains one, `allowed` becomes
`[0, 1, -1, 100]` and this section becomes the record of why it was not.

## The shift carve-out cannot be expressed

The prompt rule names two conventional values, and this rule restores neither.
`100` is a VALUE the tool refuses to take. A `<< 8` is a POSITION — the operand
of a shift — and no value allow-list can state a position, which is the same
wall `magic-numbers-go` and `magic-numbers-typescript` meet. `no_magic_number`
takes one other key, `allowed_in_widget_params`, and it names a Flutter widget
argument rather than a shift.

So a shift operand REPORTS. The fail fixture carries `word << 8` for that
reason, and the acceptance test
`the_shipped_dart_magic_numbers_tool_rule_reports_every_fail_fixture_line`
holds the tool to reporting it, so the gap stays measured.

## Why `solid_lints` and not `dart_code_linter`

`dart_code_linter` 4.1.9 is the maintained fork of the discontinued
`dart_code_metrics`, and its `no-magic-number` rule reads an `allowed` list
correctly. Both tools were measured over the same corpus, and `solid_lints`
was taken because it reads the prompt rule's carve-outs and `dart_code_linter`
does not.

Measured over `dart-lang/http` at `a9176ac`, 324 `.dart` files copied into one
probe package:

| Tool | Findings | Run |
|---|---|---|
| `solid_lints` 0.3.3, allowed `[-1, 0, 1]` | 683 | 13 s |
| `dart_code_linter` 4.1.9, allowed `[0, 1, -1, 100]` | 653 | 5 s |

645 findings are the same finding. The 38 that only `solid_lints` reports are
15 uses of the value `100` — the carve-out it cannot state — and 23 literals
that stand inside a declaration's initializer, which `dart_code_linter` drops
because it exempts the whole subtree of a variable declaration. The 8 that only
`dart_code_linter` reports are default parameter values, every one of them:
`int retries = 3`, `this.headerTableSize = 4096`, `this.maxFrameSize = 1 << 14`.

So `dart_code_linter` buys a value carve-out with a syntactic one: it breaks
the "a default parameter" carve-out the prompt rule states, and it goes silent
over `var buffer = Uint8List(FRAME_HEADER_SIZE + 6 * settings.length)`, where
the `6` is a real finding. A gate that reports a carved-out shape and misses a
real one is the worse trade, and `100` has an inline suppression while a
missing finding has nothing.

## Measured on a real repository

`dart-lang/http` at `a9176ac` (a 324-file monorepo): **683** findings in
**13 s**, all of them `no_magic_number` and no other code. Hand-checked over a
sample of 15: each names a literal that stands in a comparison, a call
argument, a return, or an operation, and each is true by position — a status
code `200` passed to `Response.bytes`, a `!= 256` comparison, an `i -= 8`. Two
shapes in the sample are the two gaps this file states: a shift operand
(`(_size << 4)`), and the value `100`.

The count is a whole-repository count. The engine keeps only the findings in
the changed files, so one review reads the part of it that the change touches.

## How the run is shaped

`dart run custom_lint` reads the configuration of the package it runs in, so
the rule owns its whole invocation only when it owns the package. Two silent
failures were measured on the way to that shape, and both are answered here:

- `--root-folder` does NOT move where the configuration is read. A run with
  `--root-folder` pointed at a directory holding the rule's own
  `analysis_options.yaml`, over source somewhere else, resolved an EMPTY rule
  list and reported `0` findings with exit `0`. The same holds for analyzing a
  path outside the probe package: 323 files read, 0 findings, because each file
  keeps the configuration of its own package. The script therefore copies each
  file into the probe package, exactly as `missing-docs-dart` does.
- `dart pub get` can fail — an empty package cache with no network reaches it
  first — and a failure there makes the whole run report nothing rather than
  break. The script tests it, prints its log on stderr and exits `1`, so the
  engine reads a broken tool instead of a clean file. Measured against an
  unreachable package host: exit `1`, with
  `Got socket error trying to find package custom_lint` on stderr.

`dart pub get --offline` runs first, and the online form runs only when it
fails, so a warm package cache keeps the whole rule off the network. Neither
form is the cost: measured on a fresh probe package, `--offline` takes 0.33 s
and the online form 0.49 s, while `dart run custom_lint` takes 6.4 s, because
`custom_lint` compiles its plugin into each package it meets.

The temporary directory is resolved with `pwd -P` before use. On macOS
`mktemp -d` answers a path through a symbolic link (`/var/...`) while
`custom_lint` reports the resolved path (`/private/var/...`), and the prefix
strip would match nothing.

`--format=json` writes one JSON document on stdout and nothing else, so `jq`
reads stdout whole. A clean run writes `{"version":1,"diagnostics":[]}` and
exits `0`; a run with findings exits `1`, and `jq` normalizes that to `0`. A
plugin that throws writes its failure on stdout, which `jq` cannot parse, so
that run exits nonzero and reads as a broken tool.

The scope is `files` because the probe package holds the files the script is
given.

The rule declares no install commands. The two versions it depends on are
pinned in the `pubspec.yaml` the script writes, which is the whole install, and
`dart` itself is a component of the Dart SDK rather than a package with its own
version. The `doctor.fix_hint` states `brew install dart-sdk`. `sah doctor`
shows that hint as the fix; the install lifecycle never runs it.

Selection in the pipe is attribution, not exemption. To exempt one literal,
write `// ignore: no_magic_number` on the line ABOVE it, with the reason after
it. Measured: the marker on the line above silences the finding, and the same
marker at the end of the line does NOT — `custom_lint` reads the preceding line
only. `// ignore_for_file: no_magic_number` at the top of a file exempts the
whole file.

## The run answers for its own arguments

This script copies each file it takes into a package of its own, and it
runs `custom_lint` inside that package. The tool therefore reads no
default target, and a run with no argument hands it a package that holds
no Dart file. The cost of that run is the whole package build: a
`pubspec.yaml`, a `dart pub get`, and a lint pass over nothing.

The script counts its arguments first, and a count of zero exits 0 with no
finding, before it makes the package. Measured over two Dart files, each
comparing against one unnamed literal and returning another, with no
argument: 0 findings and exit 0 before the guard, and the same after it.
The same script over the two files reports 4. The acceptance test
`the_shipped_dart_magic_numbers_tool_rule_reads_only_the_files_it_is_given`
holds both halves: the run with no argument, and the run over the two
files.

## The temporary directory the package stands in

`mktemp -d` makes the package directory, and `trap 'rm -rf "$work"' EXIT`
removes it. A package carries a `pubspec.yaml`, an `analysis_options.yaml`
and a copy of each file the run takes, so the directory left behind was
the largest of the roster. Measured over one file: one run raised the
count of entries under `TMPDIR` by 1 before the trap, and leaves that
count unchanged after it.
