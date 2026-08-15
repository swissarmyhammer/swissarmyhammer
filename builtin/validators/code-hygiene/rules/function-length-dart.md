---
name: function-length-dart
description: Dart functions stay under the length gate — checked by dart_code_linter, not by prompt.
match:
  files:
    - "**/*.dart"
  project_types:
    - flutter
supersedes: function-length
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
    package="$work/package"
    mkdir -p "$package/lib"
    : > "$work/copied"
    index=0
    for file in "$@"; do
      if [ ! -r "$file" ]; then
        printf 'function-length-dart cannot read %s\n' "$file" >&2
        exit 1
      fi
      if ! iconv -f UTF-8 -t UTF-8 "$file" > /dev/null 2>&1; then
        printf 'function-length-dart cannot decode %s as UTF-8\n' "$file" >&2
        exit 1
      fi
      case "$file" in
        */test/*|test/*|*/integration_test/*|integration_test/*|*/test_driver/*|test_driver/*|*/benchmark/*|benchmark/*|*_test.dart|*.g.dart|*.freezed.dart|*.mocks.dart|*.gr.dart|*.config.dart|*.gen.dart|*.pb.dart|*.pbenum.dart|*.pbjson.dart|*.pbserver.dart)
          continue
          ;;
      esac
      index=$((index + 1))
      copy="lib/probe_$index.dart"
      cp "$file" "$package/$copy"
      printf '%s\t%s\n' "$copy" "$file" >> "$work/copied"
    done
    if [ ! -s "$work/copied" ]; then
      exit 0
    fi
    version_report="$(dart --version 2>&1)"
    sdk_version="$(printf '%s\n' "$version_report" | sed -n 's/.*Dart SDK version: \([0-9][0-9.]*\).*/\1/p')"
    if [ -z "$sdk_version" ]; then
      printf '%s\n' "$version_report" >&2
      printf 'function-length-dart: dart --version names no version, so the probe package cannot state the language version this SDK parses with\n' >&2
      exit 1
    fi
    printf '%s\n' 'name: sah_function_length_probe' 'environment:' "  sdk: '^$sdk_version'" \
      'dev_dependencies:' '  dart_code_linter: 4.2.0' > "$package/pubspec.yaml"
    printf '%s\n' 'analyzer:' '  errors:' '    todo: ignore' > "$package/analysis_options.yaml"
    pub_status=0
    dart pub get --offline --directory "$package" > "$work/pub-get" 2>&1 || pub_status=$?
    if [ "$pub_status" -ne 0 ]; then
      pub_status=0
      dart pub get --directory "$package" > "$work/pub-get" 2>&1 || pub_status=$?
    fi
    if [ "$pub_status" -ne 0 ] || [ ! -f "$package/.dart_tool/package_config.json" ]; then
      cat "$work/pub-get" >&2
      printf 'function-length-dart: dart pub get exited %s and left the probe package with no package config, so the metric run reads no file of it\n' "$pub_status" >&2
      exit 1
    fi
    metrics_status=0
    ( cd "$package" && dart run dart_code_linter:metrics analyze \
      --reporter=json --json-path="$work/metrics.json" \
      --source-lines-of-code=250 lib ) > "$work/metrics-out" 2> "$work/metrics-error" || metrics_status=$?
    if [ "$metrics_status" -ne 0 ]; then
      cat "$work/metrics-error" "$work/metrics-out" >&2
      printf 'function-length-dart: dart_code_linter exited %s and measured no function\n' "$metrics_status" >&2
      exit 1
    fi
    if [ ! -s "$work/metrics.json" ]; then
      printf 'function-length-dart: dart_code_linter wrote no report, so it read no Dart file this run names\n' >&2
      exit 1
    fi
    jq -r '.records[] | .path' "$work/metrics.json" | sort > "$work/measured"
    cut -f1 "$work/copied" | sort > "$work/wanted"
    comm -23 "$work/wanted" "$work/measured" > "$work/unmeasured"
    if [ -s "$work/unmeasured" ]; then
      awk -F'\t' 'NR == FNR { source[$1] = $2; next } { print source[$1] }' \
        "$work/copied" "$work/unmeasured" >&2
      printf 'function-length-dart: dart_code_linter wrote no record for the file above, so it measured no function of it\n' >&2
      exit 1
    fi
    jq -r '.records[] | select((.functions // {}) | length == 0) | .path' "$work/metrics.json" \
      | sort > "$work/silent"
    if [ -s "$work/silent" ]; then
      analysis_status=0
      dart analyze --format=machine "$package/lib" > "$work/analysis" 2> "$work/analysis-error" \
        || analysis_status=$?
      if [ "$analysis_status" -gt 3 ]; then
        cat "$work/analysis-error" "$work/analysis" >&2
        printf 'function-length-dart: dart analyze exited %s and judged no code\n' "$analysis_status" >&2
        exit 1
      fi
      awk -F'|' -v prefix="$package/" '$2 == "SYNTACTIC_ERROR" { print substr($4, length(prefix) + 1) }' \
        "$work/analysis" | sort -u > "$work/unparsed"
      comm -12 "$work/silent" "$work/unparsed" > "$work/broken"
      if [ -s "$work/broken" ]; then
        awk -F'\t' 'NR == FNR { source[$1] = $2; next } { print source[$1] }' \
          "$work/copied" "$work/broken" >&2
        printf 'function-length-dart: the file above does not parse and measured no function, so its length is unread\n' >&2
        exit 1
      fi
    fi
    jq -r '.records[] | .path as $path | .functions // {} | to_entries[]
           | . as $function | .value.metrics[]?
           | select(.metricsId == "source-lines-of-code")
           | select(.level == "warning" or .level == "alarm")
           | [$path, ($function.value.codeSpan.start.line | tostring),
              "\($function.key) runs \(.value) source lines of code, over the gate of 250"]
           | @tsv' "$work/metrics.json" > "$work/findings"
    awk -F'\t' 'NR == FNR { source[$1] = $2; next } { printf "%s:%s: %s\n", source[$1], $2, $3 }' \
      "$work/copied" "$work/findings"
  doctor:
    check_command: "which dart jq awk sed cut sort comm iconv mktemp"
    check_version_command: "dart --version"
    fix_hint: "brew install dart-sdk"
---

# Function Length — Dart

`dart_code_linter` computes a per-function `source-lines-of-code` metric and
takes its threshold as a command-line flag. That metric IS the
`function-length` prompt rule's own definition: it counts the distinct lines
holding a token inside the body, so a blank line and a comment-only line each
count for nothing. Measured on a probe of 302 code lines interleaved with 60
blank lines and 60 comment-only lines: the metric reads 302.

The gate is 250, which is the number the prompt rule states in as many words —
"functions longer than 250 lines of actual code". The metric reports when the
value is strictly OVER the threshold, so the flag carries the prompt rule's own
number with no derivation.

This rule supersedes `function-length`, the ONE size gate this set states.
Nothing measures complexity, for Dart or for any other language. `VALIDATOR.md`
records the Dart survey and the measurement that settled it.

## Every number below was measured with `dart_code_linter` 4.2.0

on Dart SDK 3.11.0 and Flutter 3.41.2.

## What the gate reaches

`source-lines-of-code` reads four declaration kinds. Measured over one body of
300 code lines in each shape:

| the declaration | reported |
|---|---|
| a top-level function | yes |
| a method | yes |
| a getter | yes |
| a constructor | yes |
| an expression-bodied (`=>`) function | yes |
| a closure held in a top-level variable | no |

The fixture pair carries the first four kinds, each at 252 or 253 code lines in
the fail fixture and 247 or 248 in the pass fixture, so a tool upgrade that
stopped reading a whole kind cannot pass the pair.

The last row is the one gap. `function-length` states "All Function Types:
Methods, closures, lambdas, standalone functions", and a closure gets no record
of its own. It is a narrow gap rather than the whole carve-out, because a
closure's lines count toward the function that HOLDS it: measured, a `void
topFn()` whose only statement is a 300-line closure reads 303, and a `void
main()` holding one 300-line `test(...)` closure reads 302. Only a closure
standing in a top-level variable, inside no function at all, escapes. That is
the trade this rule makes, and `function-length-swift` records the mirror of
it — Swift needs a second swiftlint rule to reach a closure at all.

## The corpus every count below was measured over

Three Dart repositories, cloned at HEAD:

| repository | commit |
|---|---|
| dart-lang/http | `a9176ac` |
| dart-lang/shelf | `fb3f931` |
| flutter/packages | `a3e763e` |

3931 `.dart` files, 3630 of which carry at least one function, 63241 functions
in all. Each sweep below is arithmetic on the tool's own per-function numbers.

## Why the gate needs a test carve-out, and what shape it takes

`dart_code_linter` folds a closure into the function that holds it, and a Dart
test file is one `main` holding every `group` and `test` closure of the file. So
`main` in a test file measures the WHOLE FILE, which is an artifact of how the
tool attributes a closure rather than a fact about a function a reader must hold
in their head.

Measured over the corpus:

| gate | findings | in test files | on `main` in a test file |
|---|---|---|---|
| 100 | 966 | 589 | 561 |
| 150 | 648 | 485 | 465 |
| 200 | 468 | 421 | 409 |
| 250 | 400 | 376 | 369 |
| 300 | 352 | 336 | 330 |
| 400 | 296 | 287 | 282 |

At the gate of 250, 376 of 400 findings stand in a test file and 369 of those
are the file's own `main`. No threshold separates that population, because its
size is the size of the file. Under this set's contract a tool finding is a
requirement, so without the carve-out this gate would make 369 suppressions
mandatory on idiomatic Dart test files.

### The carve-out reads a PATH, and this is what that costs

this set names the mark: identify a test from its attribute or framework naming
convention at the **definition**, never from the file name. This rule cannot
meet that standard, and the reason is
structural rather than a missing option.

A Dart test is an anonymous closure handed to `test(...)`, `testWidgets(...)` or
`group(...)`. It is not a declaration and it carries no name, so there is no
definition to read. The tool measures the enclosing `main`, whose own
definition says nothing. `dart_code_linter` 4.2.0 offers exclusion by GLOB
alone — `metrics-exclude` and `--exclude` both take patterns and neither takes a
declaration name — so a path is the only mark available.

The cost was measured rather than assumed. Over the corpus at the gate of 250,
7 functions in test files are NOT `main`, and every one of them is itself a
test-registration function: `uiTestGroup` and `stateMachineTestGroup` in
`cupertino_ui/test/refresh_test.dart`, three `runTests` in the
`google_maps_flutter` integration tests, `runAsyncTests` in the
`shared_preferences_android` integration tests, and `runAllTests` in
`script/tool/test/update_excerpts_command_test.dart`. So the path carve-out
silences ZERO genuine helpers in this corpus, which is why the trade is taken
here and refused in `function-length-go` — that rule measured 11 real helpers
its own path exclusion would have dropped, and reads a function NAME instead
because `go test` states its convention at the definition.

A helper in a test file therefore gets no length finding from this rule. That is
the whole of the loss, it is stated rather than hidden, and it is the reason a
change under `test/` reports nothing here.

### What the script excludes

The script filters its own argument list rather than passing the tool an
`--exclude` list, so that a run whose every file is excluded is a CLEAN answer
and not a tool error. The section "A run whose every file the carve-out
excludes" below states that measurement.

The excluded shapes are the pub package layout's test directories — `test/`,
`integration_test/`, `test_driver/` and `benchmark/` — the `package:test` file
naming convention `*_test.dart`, and each generator's fixed output suffix:
`*.g.dart`, `*.freezed.dart`, `*.mocks.dart`, `*.gr.dart`, `*.config.dart`,
`*.gen.dart`, `*.pb.dart`, `*.pbenum.dart`, `*.pbjson.dart` and
`*.pbserver.dart`. `missing-docs-dart` carries the same two lists for the same
reasons, and a suffix is the fixed output name of one generator rather than a
reading of the file.

`dart_code_linter` also refuses `*.g.dart` and `*.freezed.dart` on its own —
`_isSupported` in `lint_analyzer.dart` tests both suffixes — so those two are
excluded whether the script names them or not. The script names them anyway,
because the script's own count of what it copied is what decides the clean
answer above.

## What the run reports on a real repository

The shipped script over every `.dart` file of `flutter/packages` at `a3e763e` —
3508 files: **22 findings in 55 s**, at exit 0.

Every one of the 22 is TRUE as a measurement: each names a declaration whose
body really does run more than 250 code lines, and the largest are
`runPigeonIntegrationTests` at 2175, `GoogleFonts.asMap` and
`GoogleFonts._asMapOfTextThemes` at 1895 each, and `_tokenize` at 1219.

9 of the 22 are shapes the PROMPT rule carves out and this tool does not:

| the finding | the carve-out the prompt rule states |
|---|---|
| `runPigeonIntegrationTests`, 2175 | a test, registered under `lib/` rather than under `test/` |
| `main` in `material_ui/test_fixes/material.dart` and `theme_data.dart` | mostly configuration or data |
| `getMaterialTranslation`, `getCupertinoTranslation` | generated code, in a file named `generated_*.dart` |
| `GoogleFonts.asMap`, `GoogleFonts._asMapOfTextThemes` | generated code, and a data map |
| `_coreWidgetsDefinitions`, `_materialWidgetsDefinitions` | mostly configuration or data |

The remaining 13 are long procedural declarations the prompt rule lists as well:
`_InputDecoratorState.build` at 340, `ThemeData.debugFillProperties` at 633,
`_RenderRangeSlider.paint` at 309, `RenderTableViewport._paintCells` at 302.

The count is a whole-repository count. The engine keeps only the findings in the
changed files, so one review reads the part of it the change touches.

## The carve-outs the prompt rule states

`function-length` exempts four shapes: a test, generated code, a function that
is mostly configuration or data, and an initialization function that sets many
fields. The run reproduces two of them and the author answers the other two.

- **A test** and **generated code** are the two the run reproduces, through the
  exclusion list above.
- **Configuration and data** the run does NOT drop. A data line counts like a
  code line, so the two `_*WidgetsDefinitions` tables of the measurement above
  report. `function-length-swift` records the same gap for the same reason, and its
  answer is the same: move the data out of the declaration, or write the
  annotation.
- **An initializer that sets many fields** the run does not drop either. A
  constructor IS read by this gate — the fixture pair pins that — so an
  initializer of more than 250 field assignments reports. `ThemeData.ThemeData`
  at 275 code lines is the corpus's own example.

## The annotation an author writes

To exempt one declaration, write `// ignore: source-lines-of-code` on the line
DIRECTLY above its signature, with the reason after it. `dart_code_linter`
reads the marker at the declaration line, and `lint_analyzer.dart` computes that
line with `firstTokenAfterCommentAndMetadata`, so the marker stands UNDER a doc
comment rather than above it.

Measured over one function of 302 code lines against the gate of 250, each of
these spellings gives no finding:

- `// ignore: source-lines-of-code` on the line above the signature.
- `//ignore: source-lines-of-code` with no space after the `//`.
- `// ignore: source-lines-of-code, one row for each supported locale`, with a
  reason after the name.
- a doc comment, then the marker, then the signature.
- `// ignore_for_file: source-lines-of-code` at the top of the file, which
  covers every declaration of it.

Each of these spellings gives one finding:

- the marker with a blank line between it and the signature.
- the marker ABOVE a doc comment rather than under it.
- `// ignore: long-method`, which is the name the discontinued
  `dart_code_metrics` used for a rule of its own.
- `// ignore: function-length`, which names this rule rather than the metric.

The marker does not expire. `dart_code_linter` 4.2.0 states no unfulfilled
suppression check, so a marker stands until an author takes it away. The first
fix a finding asks for is still to split the declaration; the marker is the
second fix, and the reason beside it states why.

An author cannot answer the generated-code carve-out with the marker, because
the generator writes the file again and the marker goes away each time. That is
why the run makes that test and the author does not.

## How the run is shaped

`dart_code_linter` reads its thresholds from the command line, so the gate
itself needs no configuration file. Two other things do make the probe package
necessary.

- **The project's own `analysis_options.yaml` can switch the gate off.**
  Measured over a probe holding three violations: a project file stating
  `dart_code_linter: metrics-exclude: ["lib/**"]` takes the run to 0 findings at
  exit 0, and one stating `analyzer: exclude: ["lib/**"]` leaves a report of 0
  bytes. A project file stating `dart_code_linter: metrics:
  source-lines-of-code: 5` moves nothing, because the command line wins over the
  configured threshold — but the two exclusion keys are enough on their own. The
  script therefore builds a package of its own, writes its own
  `analysis_options.yaml` into it so no file above it is read, and copies the
  changed files in.
- **`dart run dart_code_linter:metrics` needs the package that declares the
  dependency.** The probe package pins `dart_code_linter: 4.2.0` in the
  `pubspec.yaml` the script writes, which is the whole install, and the project
  under review never sees the dependency. `magic-numbers-dart` and
  `missing-docs-dart` build such a package for the same reason.

The scope is `files` because the probe package holds the files the script is
given.

### The probe package states the language version of the installed SDK

A package's language version is the LOWER bound of its `environment: sdk:`
constraint, and the analyzer refuses syntax newer than that version. A fixed
floor therefore breaks real code as the language moves.

Measured over the 3508 `.dart` files of `flutter/packages` at `a3e763e`: a probe
stating `sdk: '>=3.5.0 <4.0.0'` makes `dart analyze` write
`This requires the 'dot-shorthands' language feature to be enabled` as a
SYNTACTIC_ERROR, and the whole run reports 0 findings. The script reads the
version out of `dart --version` and writes `sdk: '^3.11.0'`, and the same run
reports its 22 findings. The caret keeps the constraint correct across a major
version as well.

An EXPERIMENTAL feature a project turns on in its own `analysis_options.yaml`
still reads as a syntax error, because the script does not read that file.
Measured over `cupertino_ui/lib/src/context_menu.dart`, which uses
`private-named-parameters`: `dart analyze` reports 4 syntactic errors and
`dart_code_linter` measures all 72 functions of the file either way. The parse
recovers, so the metric is unaffected — and that is why the syntax test below is
narrow.

### Each file is copied to a flat probe name

The script copies the Nth file it keeps to `lib/probe_N.dart` and writes the
pair into a table it maps the findings back through. The names are flat because
`dart_code_linter` SKIPS every file under a dot directory: measured,
`lib/.agents/x/a.dart` gets no record at all while `lib/plain/b.dart` gets one,
and `flutter/packages` really does carry Dart files under
`camera_android_camerax/.agents/skills/`. Under the earlier scheme, which
mirrored the source path, those two files reached the "no record" test below and
broke a run that was otherwise clean.

## A run cannot answer zero for a broken tool

Five shapes make this tool report nothing while exiting 0, and each of the five
reads exactly like a clean file. The script tests for all five.

Measured with `dart_code_linter` 4.2.0 over one file holding one function of 302
code lines:

| the run | what the tool does | what the script does |
|---|---|---|
| a file the path names but that is not there | — | exits 1, `cannot read` |
| a file that is not valid UTF-8 | one record, 0 functions, exit 0 | exits 1, `cannot decode` |
| a file that does not parse at all | one record, 0 functions, exit 0 | exits 1, `does not parse` |
| a file under a dot directory | NO record, exit 0 | exits 1, `wrote no record` |
| a directory holding no Dart file | a report of 0 bytes, exit 0 | exits 1, `wrote no report` |

The UTF-8 test is `iconv -f UTF-8 -t UTF-8`, and it is exact: measured over four
probe files, the Latin-1 one fails it and the healthy one, the unparseable one
and the empty one each pass it. Dart source is UTF-8 and nothing else, so a file
that fails the test is one the analyzer will read no token of.

### The syntax test is narrow on purpose

A file that does not parse and a file that holds no function look the same in
the report: one record, `functions: {}`. An empty Dart file, a file of only
constants and a barrel file are each legitimately in that state, so the state
alone cannot break the run.

The script therefore runs `dart analyze --format=machine` ONLY when some file
measured no function, and it breaks only on the INTERSECTION: a file that both
measured no function AND carries a `SYNTACTIC_ERROR`. Measured:

| the file | measured functions | SYNTACTIC_ERROR | the script |
|---|---|---|---|
| `this is @@@ not (((  dart ]]]` | 0 | yes | exits 1 |
| `cupertino_ui/lib/src/context_menu.dart` | 72 | yes | measures it |
| an empty `.dart` file | 0 | no | reports nothing, exit 0 |
| a healthy file over the gate | 1 | no | reports it |

Row 2 is why the test is an intersection rather than a plain search for a
syntactic error: an earlier shape that broke on any SYNTACTIC_ERROR reported 0
findings and exited 1 over the whole of `flutter/packages`, because that
repository turns on an experimental language feature the probe package does not.

The machine format is what separates the two causes. A syntax failure writes
`ERROR|SYNTACTIC_ERROR|<code>` and an import the probe cannot resolve writes
`ERROR|COMPILE_TIME_ERROR|URI_DOES_NOT_EXIST`. The probe package declares no
dependency of its own, so every `package:` import of a real Flutter file is
unresolved and `dart analyze` exits 3 on nearly every run. The script accepts 0
through 3, which are the four issue severities, and breaks above them; 64 is the
usage error, which judges nothing.

### Every status the run reads

Two more commands can fail on a machine that HAS the Dart SDK, and each fails
silently. Measured:

| the run | status | what the script does |
|---|---|---|
| `dart pub get` succeeds and writes the package config | 0 | measures |
| `dart pub get` cannot run | 127 | exits 1, and names the status |
| `dart run dart_code_linter:metrics` cannot run | 127 | exits 1, and names the status |
| `dart analyze` cannot run | 64 | exits 1, and names the status |

Each of the three was measured by putting a `dart` on the path that exits 127
for one subcommand and hands every other subcommand to the real one. Each shape
reports no finding and exits 1, and the earlier shape that read no status
reported no finding and exited 0.

`dart pub get --offline` runs first and the networked form runs only when it
fails, so a warm package cache keeps the whole rule off the network. `set -e`
stands at the head of the script, so no other command can fail without stopping
the run.

## A run whose every file the carve-out excludes

A change that touches only test files, or only generated files, leaves the
script with nothing to copy. Handing that to the tool would make it read a
package holding no Dart file, which writes a report of 0 bytes at exit 0 — the
shape the "wrote no report" test above breaks on. A change under `test/` would
then answer with a tool error rather than with a clean list.

The script counts what it copied instead, and a count of zero exits 0 with no
finding before the probe package is built. Measured: a run over one test file
alone reports nothing at exit 0, a run over one `*.g.dart` file alone reports
nothing at exit 0, and a run over one test file BESIDE one long file reports the
long one alone.

This is also why the script filters its own argument list rather than passing
`--exclude` to the tool: the script's own count is what tells "everything was
excluded" from "the tool read nothing".

## A run answers for the files it is given, and for no other

`dart_code_linter` reads the one path each run names, which is a package the
script builds, so a run with no argument would hand it a package holding no Dart
file — and pay a `pubspec.yaml`, a `dart pub get` and a metric pass for nothing.

The script counts its arguments first, and a count of zero exits 0 with no
finding before the temporary directory exists. Measured over two Dart files each
holding one function of 302 code lines, with no argument: 0 findings and exit 0.
The same script over the two files reports 2.

## The temporary directory the package stands in

`mktemp -d` makes one working directory for the whole run, and
`trap 'rm -rf "$work"' EXIT` removes it. It holds the `copied` table pairing
each probe name with the file it came from, the probe package, the `pub-get`
output, the `metrics.json` report and the lists the tests above compare —
`measured`, `wanted`, `unmeasured`, `silent`, `unparsed`, `broken` and
`findings`.

The directory is resolved with `pwd -P` before use. On macOS `mktemp -d` answers
a path through a symbolic link (`/var/...`) while the tools report the resolved
path (`/private/var/...`).

Measured over one file: one run raised the count of entries under `TMPDIR` by 1
before the trap, and leaves that count unchanged after it.

## The rule declares no install commands

`dart_code_linter` is pinned at 4.2.0 in the `pubspec.yaml` the script writes,
which is the whole install, and `dart` itself is a component of the Dart SDK
rather than a package with its own version. The `doctor.fix_hint` states
`brew install dart-sdk`. `sah doctor` shows that hint as the fix; the install
lifecycle never runs it.

Selection in the report filter is attribution, not exemption. The report carries
ten metrics for each function and this rule owns one of them, so the filter
names `source-lines-of-code` and drops the rest. The level bands are the tool's
own: a value over twice the threshold is `alarm`, a value over the threshold is
`warning`, and a value over four fifths of it is `noted`. The filter keeps
`warning` and `alarm`, which is the tool's own `isReportLevel`, so a `noted`
near-miss is not a finding.
