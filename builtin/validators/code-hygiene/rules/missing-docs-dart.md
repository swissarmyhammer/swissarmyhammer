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
    package="$(cd "$work" && pwd -P)"
    printf '%s\n' 'name: sah_missing_docs_probe' 'environment:' "  sdk: '>=3.0.0 <5.0.0'" > "$package/pubspec.yaml"
    cat > "$package/analysis_options.yaml" <<'ANALYSIS_OPTIONS'
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
    for file in "$@"; do
      copy="$package/lib/${file#/}"
      mkdir -p "$(dirname "$copy")"
      cp "$file" "$copy"
    done
    (cd "$package" && dart pub get --offline) > /dev/null 2>&1
    dart analyze --format=machine "$package" |
      awk -F'|' -v prefix="$package/lib/" '
        $3 == "PUBLIC_MEMBER_API_DOCS" && index($4, prefix) == 1 {
          printf "%s:%s: %s\n", substr($4, length(prefix) + 1), $5, $8
        }'
  doctor:
    check_command: "which dart awk"
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

The override carve-out needs the analyzer to RESOLVE the overridden member. The
probe package declares no dependencies, so an import of `package:flutter` does
not resolve, and an `@override` of a member that import declares reports.
Measured on a two-package probe: the package analyzed in place reports one
finding, and the probe over the same file reports two. `Object` always
resolves, so `toString`, `operator ==` and `hashCode` keep the carve-out
wherever the file stands.

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

`dart analyze` reads the one path this script names, which is the package
the script builds. A run with no argument copies no file into that
package, so the analyzer walks a package with no Dart file in it and
answers nothing. The build still costs a `dart pub get` and an analysis
pass.

The script counts its arguments first, and a count of zero exits 0 with no
finding, before the package exists. Measured over two Dart files, each
holding one undocumented class and one undocumented method, with no
argument: 0 findings and exit 0 before the guard, and the same after it.
The same script over the two files reports 4. The acceptance test
`the_shipped_dart_missing_docs_tool_rule_reads_only_the_files_it_is_given`
holds the answer of 0.

## The temporary directory the package stands in

`mktemp -d` makes the package directory, and `trap 'rm -rf "$work"' EXIT`
removes it. The package holds a `pubspec.yaml`, an `analysis_options.yaml`
and a copy of each file the run takes. Measured over one file: one run
raised the count of entries under `TMPDIR` by 1 before the trap, and
leaves that count unchanged after it.
