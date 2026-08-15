---
name: missing-docs-dart
description: Public Dart members need doc comments — checked by dart analyze, not by prompt.
match:
  files:
    - "**/*.dart"
  project_types:
    - flutter
supersedes: missing-docs
tool:
  scope: files
  run: |
    if [ "$#" -eq 0 ]; then
      exit 0
    fi
    work="$(mktemp -d)"
    trap 'rm -rf "$work"' EXIT
    work="$(cd "$work" && pwd -P)"
    printf '%s\n' 'name: sah_missing_docs_probe' 'environment:' "  sdk: '>=3.0.0 <5.0.0'" > "$work/probe-pubspec.yaml"
    cat > "$work/probe-options.yaml" <<'ANALYSIS_OPTIONS'
    analyzer:
      exclude:
        - "**/test/**"
        - "**/integration_test/**"
        - "**/test_driver/**"
        - "**/bin/**"
        - "**/tool/**"
        - "**/benchmark/**"
        - "**/web/**"
        - "**/example/*.dart"
        - "**/*.g.dart"
        - "**/*.freezed.dart"
        - "**/*.mocks.dart"
        - "**/*.gr.dart"
        - "**/*.config.dart"
        - "**/*.gen.dart"
        - "**/*.pb.dart"
        - "**/*.pbenum.dart"
        - "**/*.pbjson.dart"
        - "**/*.pbserver.dart"
    linter:
      rules:
        - public_member_api_docs
    ANALYSIS_OPTIONS
    : > "$work/files"
    for file in "$@"; do
      dir="$(cd "$(dirname "$file")" 2>/dev/null && pwd -P)"
      config="-"
      while [ -n "$dir" ]; do
        if [ -f "$dir/.dart_tool/package_config.json" ]; then
          config="$dir/.dart_tool/package_config.json"
          break
        fi
        if [ "$dir" = "/" ]; then
          break
        fi
        dir="$(dirname "$dir")"
      done
      printf '%s\t%s\n' "$config" "$file" >> "$work/files"
    done
    awk -F'\t' '{ print $1 }' "$work/files" | sort -u > "$work/configs"
    group=0
    while IFS= read -r config; do
      group=$((group + 1))
      package="$work/package-$group"
      mkdir -p "$package"
      cp "$work/probe-pubspec.yaml" "$package/pubspec.yaml"
      cp "$work/probe-options.yaml" "$package/analysis_options.yaml"
      awk -F'\t' -v config="$config" '$1 == config { print $2 }' "$work/files" > "$work/group"
      while IFS= read -r file; do
        copy="$package/lib/${file#/}"
        mkdir -p "$(dirname "$copy")"
        cp "$file" "$copy"
      done < "$work/group"
      (cd "$package" && dart pub get --offline) > /dev/null 2>&1
      if [ "$config" = "-" ]; then
        dart analyze --format=machine "$package"
      else
        dart analyze --format=machine --packages="$config" "$package"
      fi |
        awk -F'|' -v prefix="$package/lib/" '
          $3 == "PUBLIC_MEMBER_API_DOCS" && index($4, prefix) == 1 {
            printf "%s:%s: %s\n", substr($4, length(prefix) + 1), $5, $8
          }'
    done < "$work/configs"
  doctor:
    check_command: "which dart awk mktemp"
    check_version_command: "dart --version"
    fix_hint: "brew install dart-sdk"
---

# Missing Documentation — Dart

`dart analyze` reports every public member without a doc comment when the
`public_member_api_docs` lint is on. The lint is opt-in.

`dart analyze` takes no rule flag. It reads `analysis_options.yaml` by walking
up the directory tree from each analyzed file, so the only way for the rule to
own its configuration is to build the tree the analyzer walks. The script makes
a probe package in a temporary directory, copies the changed files under its
`lib/`, analyzes the package, and maps the temporary paths back to the paths it
was given. The project's own `analysis_options.yaml` is never read.

The probe package resolves nothing on its own, so the run names the project's
own `.dart_tool/package_config.json` beside it. "The package config that
resolves the file" below states which one it names, and what it does when the
project has none.

Two properties of the lint make the probe package necessary, and both fail
silently rather than loudly:

- `public_member_api_docs` reports only for a file inside a package's `lib/`
  directory. A loose file with the configuration beside it reports nothing.
- The analyzer needs `.dart_tool/package_config.json` to recognize the package,
  and only `dart pub get` writes it. Without it this lint stays quiet while
  other lints still report. `--offline` keeps the probe package, which declares
  no dependencies, off the network.

## The probe moves a file, so the exclude list puts it back

The probe copies each changed file under `lib/`, because that is the only place
the lint reads. The move is what makes the lint run at all. The move also
carries a file into a position the analyzer would never read in the project
itself, and a finding there is the rule's own artifact rather than a fact about
the code.

Measured on Dart SDK 3.11.0. One package holds `lib/a.dart`, `lib/src/b.dart`,
`test/c.dart`, `bin/d.dart`, `tool/e.dart`, `example/g.dart`,
`integration_test/h.dart` and a file at the package root. Each file holds the
same undocumented public function. The analyzer reports two of them:
`lib/a.dart` and `lib/src/b.dart`. It reads `lib/` and no other directory. The
probe over `test/widget_test.dart` — which holds `class TestHelper`,
`void reset()` and `void buildHarness()` — reports all three.

The probe's `analysis_options.yaml` therefore carries an `analyzer: exclude:`
list. The list names each package directory that is not `lib/`: `test/`,
`integration_test/`, `test_driver/`, `bin/`, `tool/`, `benchmark/` and `web/`.
A file the list excludes is copied and analyzed, and the lint stays silent
about it, which is the answer the project's own analyzer gives.

`example/` is named as `**/example/*.dart`, which is a Dart file directly in
that directory, and not the whole tree below it. Many repositories put a
package of its own under `example/`, and that package's `lib/` is a public API
the same as any other. Measured against the list: `example/lib/a.dart` and
`packages/pkg/lib/a.dart` report, and `example/a.dart` and
`packages/pkg/test/a.dart` stay silent.

A change under `test/` gets no finding from this rule. The rule supersedes
`missing-docs` for every `.dart` file, so no rule asks for a doc comment there.
That is deliberate. A Dart package's public API is its `lib/` directory, and a
test, a script, a benchmark and an entry point stand outside it.

The list cannot be an exact complement of `lib/`, because an exclude glob
cannot state a negation. It names the directories the pub package layout
defines instead.

## Generated code, which the tool reports and the prompt rule carves out

The `missing-docs` prompt rule carves out generated code. A project states the
same carve-out to its own analyzer as `analyzer: exclude:` entries for
`*.g.dart` and `*.freezed.dart`. This rule writes its own configuration, so
that entry of the project is never read, and the carve-out has to be restored
here.

Measured: a package that holds `lib/gen.g.dart` reports it; the same package
with `analyzer: exclude: ["**/*.g.dart"]` reports nothing there. Through the
probe, a `model.g.dart` reports each member, and a `model.g.dart` that holds
only `part of 'model.dart';` and a class reports two findings, because the
probe copies the part file without the library that declares it.

The exclude list therefore names each generator output suffix: `*.g.dart`
(build_runner), `*.freezed.dart` (freezed), `*.mocks.dart` (mockito),
`*.gr.dart` (auto_route), `*.config.dart` (injectable), `*.gen.dart`
(flutter_gen) and `*.pb.dart`, `*.pbenum.dart`, `*.pbjson.dart` and
`*.pbserver.dart` (the protobuf compiler). A suffix is the fixed output name of
one generator, so the entry is a structural fact and not a reading of the file.

A generated file that carries `// ignore_for_file: type=lint` at the top is
silent with or without the list. `freezed` writes that line. Measured: a
`model.freezed.dart` with the line reports nothing through the probe.

## What the tool carves out for itself

Each of these is measured on Dart SDK 3.11.0, and none of them needs a list.

- **A private member.** Dart privacy is the `_` prefix. `_privateField`,
  `_privateMethod`, `_privateTopLevel`, `_PrivateClass` and every member inside
  a private class report nothing. This is the prompt rule's private carve-out,
  reproduced by the language.
- **An override.** The lint reports no member that overrides a member the
  analyzer can resolve, because the documentation of the overridden member
  stands for it. `toString()` with no `@override`, `@override bool operator ==`,
  `@override int get hashCode` and an `@override` of a documented base method
  each report nothing. The `@override` annotation is not what does it:
  `toString()` without the annotation is silent too. This is the prompt rule's
  "Obvious implementations (Display, Debug, ToString, etc.)" carve-out,
  reproduced by the analyzer.
- **`main`.** A `void main()` reports nothing at any position.

The override carve-out needs the analyzer to RESOLVE the overridden member, and
the probe package declares no dependency of its own. The section below states
what the run names so that it resolves.

## The package config that resolves the file

An import of `package:flutter/material.dart` does not resolve inside a package
that declares no dependency. The analyzer then sees no override, and every
`@override Widget build(BuildContext context)`, `createState`, `initState`,
`dispose` and `didUpdateWidget` of a Flutter project reports a missing doc
comment, while the project's own `dart analyze` reports none of them.

`dart analyze --packages=<file>` names the package config the analyzer resolves
imports through. A project writes that file at
`<package>/.dart_tool/package_config.json`, and only `dart pub get` or
`flutter pub get` writes it.

Measured on Dart SDK 3.11.0 over a two-package probe. Package `app_framework`
declares `abstract class Widget` holding `String build()`, both documented.
Package `app` path-depends on it, and `packages/app/lib/screen.dart` holds an
`@override String build()` at row 6 and an undocumented `void undocumented()`
at row 8.

| the run | rows reported |
|---|---|
| `app` analyzed in place, with the lint on | 8 |
| the probe, no `--packages` | 6, 8 |
| the probe, `--packages=packages/app/.dart_tool/package_config.json` | 8 |

The flag is the whole difference. `Object` always resolves, so `toString`,
`operator ==` and `hashCode` keep the carve-out with the flag or without it.

The acceptance test
`the_shipped_dart_missing_docs_tool_rule_reads_the_package_config_of_the_file`
holds the run to row 8 alone.

### Which config, for which file

The run reads the config of the FILE, never one config for the workspace. For
each argument it walks up from that file's own directory to the first directory
holding `.dart_tool/package_config.json`, and that file is the config that
resolves it. A monorepo holds one for each package, and `dart analyze` takes
one `--packages` for one run, so the run groups its arguments by the config it
found for each and builds one probe package for each group.

Measured over a monorepo probe of two packages, `packages/alpha` and
`packages/beta`. Each path-depends on a framework package of its own, each
holds a package config of its own, and each library holds the same two members
as the `app` probe above.

| the run | rows reported |
|---|---|
| one probe package for each config | `alpha` 8, `beta` 8 |
| one probe package under the `alpha` config | `alpha` 8, `beta` 6 and 8 |
| one probe package, no `--packages` | `alpha` 6 and 8, `beta` 6 and 8 |

Neither config names the other package's framework, so the grouping is what
tells the first row from the second. The acceptance test
`the_shipped_dart_missing_docs_tool_rule_reads_one_package_config_for_each_package`
holds the first row.

### A package that has never run pub get

Such a package holds no package config, and the walk finds none. The run cannot
name a file that does not stand: measured,
`dart analyze --packages=<file that does not exist>` exits 64, writes a usage
error and lints nothing, so a run that named a missing file would break every
project that has not run pub get.

The run falls back to the probe package alone, which is what this rule did
before the flag. That answer is a SUPERSET — the override reports beside the
member that really is undocumented — and a superset is the one safe direction
here. A run that answered nothing instead would read exactly like a documented
package, and the gate would be gone with no word said.

Measured over the same `app` probe with its package config taken away: rows 6
and 8, exit 0. The acceptance test
`the_shipped_dart_missing_docs_tool_rule_reports_the_override_with_no_package_config`
holds both rows, so the fallback can never fall to silence.

A config the analyzer cannot READ is the same superset rather than silence.
Measured: `--packages=<file that is not JSON>` exits 0 and reports the lint
just as a run with no flag does.

### What the doctor run does

The doctor materializes the fixtures into a scratch directory that holds no
project, so the walk finds no package config above it and the run takes the
fallback. The fixture pair is unchanged by the flag. Measured over the shipped
fixtures in such a directory, with the flag and without it: the fail fixture
reports rows 15, 18, 20, 22 and 25, and the pass fixture reports nothing, at
exit 0 each time.

The pass fixture rests on no dependency. It overrides `toString`,
`operator ==` and `hashCode` alone, and `Object` resolves with no config, so
the carve-out those three stand on never needed one.

## What the tool does not carve out

The `missing-docs` prompt rule carves out "Simple getters/setters with
self-explanatory names". The lint has no such setting. Measured: `int get value`
and the `set value(int next)` beside it each report.

So a public getter and a public setter need a doc comment. The fail fixture
carries one of each for that reason, and the acceptance test
`the_shipped_dart_missing_docs_tool_rule_reports_every_fail_fixture_line` holds
the tool to reporting them, so the gap stays measured. The recourse is the
inline suppression at the end of this file.

A setter that has no getter of the same name is the one shape the lint stays
silent about. Measured: `set loneSetter(int next) {}` in a class, the same
setter with a block body, and a top-level setter each report nothing, while the
`set pairedValue(int next)` that stands beside `int get pairedValue` reports.
The fixture pairs its getter and its setter for that reason.

## How the run is shaped

The temporary directory is resolved with `pwd -P` before use. On macOS
`mktemp -d` returns a path through a symbolic link (`/var/...`) while `dart
analyze` reports the resolved path (`/private/var/...`), and the prefix strip
would match nothing.

The pipe ends in `awk` rather than `grep` because `grep` exits nonzero when it
matches nothing, which the engine reads as a broken tool on every clean run.

The scope is `files` because the probe package holds the files the script is
given.

The `pubspec.yaml` and the `analysis_options.yaml` a probe package needs are
written one time into the working directory and copied into each probe package
the run builds. A heredoc writes the analyzer configuration at column zero, and
a heredoc nested inside the loop that builds the packages would have to carry
the loop's own indentation into the file it writes.

The rule declares no install commands. `dart analyze` is a component of the
Dart SDK, not a package with its own version, so no install command can pin it.
The `doctor.fix_hint` states `brew install dart-sdk` instead. `sah doctor` shows
that hint as the fix; the install lifecycle never runs it.

Selection in the pipe is attribution, not exemption: to exempt one member,
write `// ignore: public_member_api_docs` above it in the code. Measured: the
marker on the line above silences the finding, and
`// ignore_for_file: public_member_api_docs` at the top of a file silences the
whole file.

## The run answers for its own arguments

`dart analyze` reads the one path each run of it names, which is a package the
script builds. The argument list is what builds those packages: a run with no
argument writes an empty file list, `sort -u` names no package config, the loop
that builds a probe package runs no time, and no `dart pub get` and no analysis
pass are made at all.

The script counts its arguments first anyway, and a count of zero exits 0 with
no finding, before the temporary directory exists. Measured over two Dart
files, each holding one undocumented class and one undocumented method, with no
argument: 0 findings and exit 0 with the guard, and the same with the guard
taken out, whose working directory then holds the two template files, two empty
lists and no probe package. The same script over the two files reports 4. The
acceptance test
`the_shipped_dart_missing_docs_tool_rule_reads_only_the_files_it_is_given`
holds both halves: the run with no argument, and the run over the two
files.

## The temporary directory the packages stand in

`mktemp -d` makes one working directory for the whole run, and
`trap 'rm -rf "$work"' EXIT` removes it. It holds `probe-pubspec.yaml` and
`probe-options.yaml`, the `files` list that pairs each argument with the
package config that resolves it, the `configs` list of the distinct configs,
the `group` list of the group being built, and one `package-<n>` directory for
each group. Each of those holds a `pubspec.yaml`, an `analysis_options.yaml`
and a copy of each file of its group under `lib/`.

Measured over one file: one run raised the count of entries under `TMPDIR` by 1
before the trap, and leaves that count unchanged after it. Measured over two
files standing in two packages: one run, one working directory holding
`package-1` and `package-2`, the same raise of 1 before the trap, and the same
count after it.
