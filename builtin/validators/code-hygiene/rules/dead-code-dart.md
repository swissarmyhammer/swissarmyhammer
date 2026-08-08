---
name: dead-code-dart
description: Dart declarations, fields, locals and imports nothing uses — checked by dart analyze, not by prompt.
match:
  files:
    - "**/*.dart"
  project_types:
    - flutter
supersedes: dead-code
tool:
  scope: workspace
  run: |
    root="$(pwd -P)"
    dart analyze --format=machine . |
      awk -F'|' -v prefix="$root/" '
        ($3 == "UNUSED_ELEMENT" || $3 == "UNUSED_FIELD" || $3 == "UNUSED_IMPORT" || $3 == "UNUSED_LOCAL_VARIABLE") && index($4, prefix) == 1 {
          printf "%s:%s: %s\n", substr($4, length(prefix) + 1), $5, $8
        }'
  doctor:
    check_command: "which dart awk"
    check_version_command: "dart --version"
    fix_hint: "brew install dart-sdk"
---

# Dead Code — Dart

`dart analyze` answers the dead-code question for Dart, and it needs no plugin
and no opt-in lint to do it. Four diagnostics carry the rule, all of them on by
default:

- `unused_import` — an import no name in the file uses.
- `unused_local_variable` — a local nothing reads.
- `unused_field` — a private field nothing reads.
- `unused_element` — a private declaration nothing references.

The last two are private-only on purpose, and that is what makes the analyzer's
answer complete rather than a guess. Dart privacy is per library, so the
analyzer already sees every caller a `_`-prefixed name could ever have. A public
declaration is the library's surface for callers outside it, and the analyzer
never reports it — the same exemption the compiler makes for free in Rust and
Go.

## The staging contract

Write `// ignore: <diagnostic>` on the line above a declaration a later change
will consume — `// ignore: unused_element`, `// ignore: unused_field`,
`// ignore: unused_local_variable`, `// ignore: unused_import`. Nothing else
counts. A staged declaration with no marker is dead.

All four were measured against Dart 3.11.0: a probe file holding one of each
kind reports five findings unannotated and none with the markers in place.

Dart's `// ignore:` carries no reason field of its own, so write the reason on
the same line after the diagnostic name. The marker says the code is staged and
the reason says what lands the consumer.

For a whole file the analyzer should leave alone — a generated file — the
file-level form `// ignore_for_file: unused_element` sits at the top.

## What this rule reads, and what it never reads

The scope is `workspace`, so `dart analyze .` runs once at the workspace root
and the engine keeps only the findings in the changed files. `dart analyze`
takes no rule flag, so a project's `analysis_options.yaml` is the only place
these four diagnostics can be turned down.

That file is where an exemption belongs. These four are analyzer diagnostics
that are on by default; an `analysis_options.yaml` cannot make them stricter,
only quieter, and a project that quiets one has recorded that decision in tool
configuration a reader can find. This rule reads it and never writes it, exactly
as it reads a `pubspec.yaml` without writing one. No lint configuration is ever
changed.

Measured over `dart-lang/http` at HEAD (a five-package monorepo): **1** finding
in 2.2 s — `pkgs/web_socket_channel/test/html_test.dart`, a private declaration
named `firstAsInt` nothing references. Hand-checked, and real. No false positive.

## How the run is shaped

`--format=machine` prints one pipe-separated record for each diagnostic, with
the diagnostic name in the third field and an absolute path in the fourth. The
`awk` selects the four names this rule owns and strips the workspace prefix back
off the path.

The workspace root is resolved with `pwd -P` before use. On macOS a path under
`/var` reaches `dart analyze` as `/private/var`, and a prefix strip against the
unresolved form would match nothing.

The pipe ends in `awk` rather than `grep` because `grep` exits nonzero when it
matches nothing, which the engine reads as a broken tool on every clean run.
`dart analyze` itself exits 2 when it has findings, and `awk` normalizes that.

Selection in the `awk` is attribution, not exemption. `dart analyze` reports
every other diagnostic the project's configuration turns on, and those belong to
their own rules or to the build, not to this one.

The rule declares no install commands. `dart analyze` is a component of the Dart
SDK, not a package with its own version, so no install command can pin it. The
`doctor.fix_hint` states `brew install dart-sdk` instead. `sah doctor` shows that
hint as the fix; the install lifecycle never runs it.
