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
    set -e
    if [ "$#" -eq 0 ]; then
      exit 0
    fi
    work="$(mktemp -d)"
    trap 'rm -rf "$work"' EXIT
    work="$(cd "$work" && pwd -P)"
    version_report="$(dart --version 2>&1)"
    sdk_version="$(printf '%s\n' "$version_report" | sed -n 's/.*Dart SDK version: \([0-9][0-9.]*\).*/\1/p')"
    if [ -z "$sdk_version" ]; then
      printf '%s\n' "$version_report" >&2
      printf 'missing-docs-dart: dart --version names no version, so the probe package cannot state the language version this SDK parses with\n' >&2
      exit 1
    fi
    printf '%s\n' 'name: sah_missing_docs_probe' 'environment:' "  sdk: '^$sdk_version'" > "$work/probe-pubspec.yaml"
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
      if [ ! -r "$file" ]; then
        printf 'sah-diagnostic: missing-docs-dart cannot read %s, so its members are unread\n' "$file" >&2
        continue
      fi
      if ! iconv -f UTF-8 -t UTF-8 "$file" > /dev/null 2>&1; then
        printf 'sah-diagnostic: missing-docs-dart cannot decode %s as UTF-8, so its members are unread\n' "$file" >&2
        continue
      fi
      dir="$(cd "$(dirname "$file")" && pwd -P)"
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
    awk -F'\t' '{ print $1 }' "$work/files" > "$work/named-configs"
    sort -u "$work/named-configs" > "$work/configs"
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
      pub_status=0
      dart pub get --offline --directory "$package" > "$work/pub-get" 2>&1 || pub_status=$?
      if [ "$pub_status" -ne 0 ] || [ ! -f "$package/.dart_tool/package_config.json" ]; then
        cat "$work/pub-get" >&2
        printf 'missing-docs-dart: dart pub get exited %s and left the probe package with no package config, so the lint reads no member of it\n' "$pub_status" >&2
        exit 1
      fi
      analysis_status=0
      if [ "$config" = "-" ]; then
        dart analyze --format=machine "$package" \
          > "$work/analysis" 2> "$work/analysis-error" || analysis_status=$?
      else
        dart analyze --format=machine --packages="$config" "$package" \
          > "$work/analysis" 2> "$work/analysis-error" || analysis_status=$?
      fi
      if [ "$analysis_status" -gt 3 ]; then
        cat "$work/analysis-error" "$work/analysis" >&2
        printf 'missing-docs-dart: dart analyze exited %s and judged no code\n' "$analysis_status" >&2
        exit 1
      fi
      awk -F'|' -v prefix="$package/lib/" '
        $3 == "PUBLIC_MEMBER_API_DOCS" && index($4, prefix) == 1 {
          printf "%s:%s: %s\n", substr($4, length(prefix) + 1), $5, $8
        }' "$work/analysis"
    done < "$work/configs"
  doctor:
    check_command: "which dart awk sed sort iconv cp mkdir dirname cat mktemp"
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
  other lints still report. So the run reads the status of `dart pub get` and
  tests that file, and "Every status the run reads" below states what it writes
  when either answer is wrong.

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

## Every status the run reads

Two commands of this script can fail on a machine that HAS the Dart SDK, and
each of them fails silently: the run then writes no row and exits 0, which the
engine reads as a clean file. So the run puts each of the two into a file, tests
its status, writes a line of its own, and exits 1.

### `dart pub get`

The probe package is worth nothing until the analyzer recognizes it, and only
`.dart_tool/package_config.json` makes it one. Measured on Dart SDK 3.11.0 over
a probe package holding one undocumented class and one undocumented method:

| the run | rows | status |
|---|---|---|
| `dart pub get` succeeds | 2 | 0 |
| the same package with its `.dart_tool` taken away | 0 | 0 |
| `sdk: '>=9.0.0 <10.0.0'`, so `pub get` exits 1 and writes no `.dart_tool` | 0 | 0 |

A probe whose `environment: sdk:` window leaves the installed SDK outside it
reaches the third row. Measured with `sdk: '>=3.5.0 <3.6.0'` on Dart SDK 3.11.0:
`dart pub get --offline` and `dart pub get` each exit 1, each writes
`Because sah_probe requires SDK version >=3.5.0 <3.6.0, version solving failed.`,
and neither writes `.dart_tool`. That silence defeats EVERY path of this rule,
the `--packages` path and the fallback alike, so it is not a shape of the
missing-config fallback below. The script reads its constraint out of
`dart --version`, so no installed SDK stands outside it.

The run therefore reads the status of `dart pub get` AND tests that the package
config stands, and it names the failure rather than analyzing a package the
analyzer does not recognize. The acceptance test
`the_shipped_dart_missing_docs_tool_rule_breaks_when_pub_get_cannot_run` holds
it, with the `pub` run of `dart` replaced by a command that exits 127 and the
`analyze` run left standing. Measured over that probe: the earlier shape wrote
0 rows and exited 0, which reads as a clean file; the shipped shape writes no
row, that line, and exit 1.

`--offline` stays. The probe package declares no dependency, so pub resolves it
with nothing from the network and nothing from the cache. Measured with
`PUB_CACHE` naming an empty directory, with `PUB_CACHE` naming a directory that
does not exist, and with `PUB_CACHE` naming a directory that cannot be written:
`dart pub get --offline` exits 0 and writes the package config each time, and a
run without the flag answers the same. So `--offline` cannot fail where a
networked run would stand, and it keeps a review off the network.

### `dart analyze`

`dart analyze` keeps one status for issues and another for a failure. Measured
on Dart SDK 3.11.0:

| the run | status | stdout |
|---|---|---|
| a lint finding alone, which is INFO | 0 | each finding |
| the same run under `--fatal-infos` | 1 | each finding |
| an unused local variable raised to WARNING | 2 | each finding |
| an unresolved import, and a file that does not parse | 3 | each finding |
| `--packages=<file that does not exist>` | 64 | none |
| a subcommand `dart` does not know | 64 | none |

0 through 3 are the four issue severities, and every one of the four writes its
rows. 64 is the usage error, and it judges nothing. So the run accepts 0 through
3 and breaks above them. 3 is load-bearing rather than tolerated: the fallback
path leaves every `package:` import unresolved, which is an ERROR, so a run that
reported correctly with no package config takes status 3.

The earlier shape was one pipe that ended in `awk`, so the script took awk's
status and answered exit 0 for every failure of the analyzer. The acceptance
test
`the_shipped_dart_missing_docs_tool_rule_breaks_when_the_analyzer_cannot_run`
holds the shipped shape, with the `analyze` run of `dart` replaced by a command
that exits 127 and the `pub` run left standing. Measured over that probe: the
pipe wrote 0 rows and exited 0; the shipped shape writes no row, that line, and
exit 1.

`the_shipped_dart_missing_docs_tool_rule_reports_both_members_when_dart_runs`
is the control for both tests. It drives the same staged file with neither run
stubbed and holds it to two rows, so a gate that broke every run it could not
read at a glance cannot pass the pair.

### Every other command

`set -e` stands at the head of the script, so no other command can fail without
stopping the run. Eighteen commands of the earlier shape threw their status
away. The two `dart` calls above are two of them. The other sixteen are
`mktemp`, the two `pwd -P` resolutions, the two template writes, the four list
writes, the `awk` that headed the config pipe, the two `mkdir -p` calls, the
three `cp` calls, and the reporting `awk`, whose status was read on the last
group alone. A `cp` that failed dropped one file out of the probe package with
no word said, which is the same silence in a smaller size.

Two commands are shaped so that `set -e` reaches them at all.
`awk ... | sort -u` became two commands writing two files, because a pipeline
takes the status of its last command alone and the script writes no `pipefail`.
`dart pub get` runs under `--directory` rather than inside
`(cd "$package" && ...)`, so every command of the script runs at the repository
root and no subshell stands between a failure and the script.

The run tests each file it is given before it copies it, and it takes a file it
cannot read OUT of the work list rather than breaking the run. `set -e` alone
answers such a run too, and it answers with a nonzero status that throws away
every finding the run did make. Measured with the two tests taken out, over
`lib/judged.dart` beside each refusing path: `cp: lib/absent.dart: No such file
or directory` and exit 1, and `cp: lib/forbidden.dart: Permission denied` and
exit 1. The section "A path the run cannot judge" below states each shape.

## A path the run cannot judge

A `files`-scope run is handed paths, and the engine reads the work-list rather
than the disk, so a path can reach the run and refuse it. Each way it refuses is
ONE item of a run that judged the other files, and
`builtin/validators/README.md` states the channel: a line opening
`sah-diagnostic:` on stderr, at exit 0.

`dart analyze` states NOTHING for a file it cannot read. Measured on Dart SDK
3.11.0 over one probe package whose `lib/` holds `judged.dart` — one
undocumented class and one undocumented method — beside a file whose bytes are
not UTF-8 and a file with no read permission, each holding two undocumented
members of its own:

| the run | status | stdout | stderr |
|---|---|---|---|
| the whole package | 0 | the 2 rows of `judged.dart` | 0 bytes |
| the file whose bytes are not UTF-8, named alone | 0 | 0 rows | 0 bytes |
| the file with no read permission, named alone | 0 | 0 rows | 0 bytes |
| a path that holds no file, named alone | 64 | 0 rows | `Directory or file doesn't exist: lib/absent.dart` and the usage text |

The analyzer drops a file it cannot read and says nothing at all, so rows 2 and
3 read exactly like a documented file. `dart analyze --help` names 3 options —
`--fatal-infos`, `--[no-]fatal-warnings` and `--help` — and none of them names a
decode failure; `dart --verbose analyze` over the file whose bytes are not UTF-8
writes 0 bytes on both channels. There is no message of the tool to read, so the
script tests each path itself. `function-length-dart` makes the same two tests
for the same reason:

- `[ ! -r "$file" ]` answers a path that holds no file and a file whose mode
  refuses a read, and it writes
  `sah-diagnostic: missing-docs-dart cannot read <path>, so its members are
  unread`.
- `iconv -f UTF-8 -t UTF-8 "$file"` answers a file whose bytes are not UTF-8,
  and it writes `sah-diagnostic: missing-docs-dart cannot decode <path> as
  UTF-8, so its members are unread`. Measured against the three staged paths and
  against a directory nobody may read: `iconv` exits 1 for each of the four, and
  it exits 0 for a healthy file.

Each test writes its line and takes the file out of the work list with
`continue`, so no `cp` of that file is made. A run whose every path refuses
writes an empty file list, `sort -u` names no package config, the loop that
builds a probe package runs no time, and the run exits 0 with its marked lines.

Measured with the shipped script over `lib/judged.dart` beside each refusing
path, in a repository that holds no package config:

| the refusing path | the earlier shape | the shipped script |
|---|---|---|
| a path that holds no file | 0 rows, exit 1, `missing-docs-dart cannot read lib/absent.dart` | 2 rows, exit 0, 1 diagnostic |
| a file whose bytes are not UTF-8 | 2 rows, exit 0, 0 bytes on stderr | 2 rows, exit 0, 1 diagnostic |
| a file with no read permission | 0 rows, exit 1, `missing-docs-dart cannot read lib/forbidden.dart` | 2 rows, exit 0, 1 diagnostic |

Rows 1 and 3 of the earlier shape threw the 2 rows away, which is what
`builtin/validators/README.md` refuses: a nonzero exit fails the WHOLE run, so
one unjudged path throws away every finding the run did make. Row 2 is the
opposite defect: `[ ! -r "$file" ]` is FALSE for a file whose bytes are not
UTF-8 — the mode lets a reader open that one — so the file passed the guard,
`cp` copied it, the analyzer dropped it in silence, and the engine read it as a
clean file.

Measured with the shipped script over `lib/judged.dart` beside all three paths:
2 rows on stdout, 3 marked lines on stderr, exit 0.

Three acceptance tests hold the three rows, one for each —
`the_shipped_dart_missing_docs_tool_rule_declines_a_path_that_holds_no_file`,
`..._declines_a_file_it_cannot_decode` and `..._declines_a_file_it_may_not_read`.
Each stages `lib/judged.dart` beside the path, and holds the run to reporting
its 2 rows AND to stating one diagnostic that names the path. A run that lost
either half fails them.

## How the run is shaped

The temporary directory is resolved with `pwd -P` before use. On macOS
`mktemp -d` returns a path through a symbolic link (`/var/...`) while `dart
analyze` reports the resolved path (`/private/var/...`), and the prefix strip
would match nothing.

The report is read with `awk` rather than `grep` because `grep` exits nonzero
when it matches nothing, which the engine reads as a broken tool on every clean
run. `awk` reads the analysis file the run wrote, rather than a pipe, so the
status of `dart analyze` reaches the test above instead of being dropped.

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

Selection in the report filter is attribution, not exemption: to exempt one
member,
write `// ignore: public_member_api_docs` above it in the code. Measured: the
marker on the line above silences the finding, and
`// ignore_for_file: public_member_api_docs` at the top of a file silences the
whole file.

### The probe package states the language version of the installed SDK

A package's language version is the LOWER bound of its `environment: sdk:`
constraint, and the analyzer refuses syntax newer than that version. A fixed
floor therefore hides real code as the language moves. A declaration the floor
does not know is a syntax error, every member inside it goes off the report, and
the run still exits 0.

Measured on Dart SDK 3.11.0 over one library. Its first declaration is an
`extension type`, which arrived in Dart 3.3, and it holds a getter and a method.
A plain class stands under it and holds a field and a method.

| the probe constraint | what the run reports |
|---|---|
| `>=3.0.0 <5.0.0`, the earlier fixed floor | rows 7, 8 and 10, and `dart analyze` writes `This requires the 'inline-class' language feature to be enabled` as a SYNTACTIC_ERROR |
| `^3.11.0`, read out of `dart --version` | rows 1, 2, 4, 7, 8 and 10 |

The earlier floor loses the three members of the extension type and exits 0,
which reads exactly like a documented declaration. That is the shape
`builtin/validators/README.md` names as a tool that reads a dirty file as clean.
The acceptance test
`the_shipped_dart_missing_docs_tool_rule_reports_a_member_of_a_newer_declaration`
holds the run to all six rows.

The script therefore reads the version out of `dart --version` with `sed` and
writes `sdk: '^<version>'`. The caret keeps the constraint correct across a
major version as well, because the version comes from the SDK that runs.
`function-length-dart` states the same measurement for its own probe.

A `dart` that answers no version takes the run to exit 1 with the line
`missing-docs-dart: dart --version names no version, so the probe package cannot
state the language version this SDK parses with`. The run must not guess a
version, because a guessed floor is the defect this section removes.

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
package config that resolves it, the `named-configs` list of the config of each
argument and the `configs` list of the distinct ones, the `group` list of the
group being built, the `pub-get` output and the `analysis` and `analysis-error`
output of the group being built, and one `package-<n>` directory for each
group. Each of those holds a `pubspec.yaml`, an `analysis_options.yaml` and a
copy of each file of its group under `lib/`.

Measured over one file: one run raised the count of entries under `TMPDIR` by 1
before the trap, and leaves that count unchanged after it. Measured over two
files standing in two packages: one run, one working directory holding
`package-1` and `package-2`, the same raise of 1 before the trap, and the same
count after it.
