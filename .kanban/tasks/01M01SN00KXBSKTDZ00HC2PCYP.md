---
assignees:
- claude-code
position_column: todo
position_ordinal: ffdf80
title: dart probe packages pin the language version to a stale floor
---
`missing-docs-dart` and `magic-numbers-dart` each build a probe package and write a fixed `environment: sdk:` constraint into its `pubspec.yaml`:

- `missing-docs-dart` writes `sdk: '>=3.0.0 <5.0.0'`
- `magic-numbers-dart` writes `sdk: '>=3.5.0 <4.0.0'`

A Dart package's LANGUAGE VERSION is the LOWER bound of that constraint, and the analyzer refuses syntax newer than that version. So each probe reads the copied files as Dart 3.0 or Dart 3.5 source, whatever SDK is installed, and a project using a newer language feature is analyzed wrongly.

Measured on Dart SDK 3.11.0 over one file using a dot shorthand (`Shade undocumentedField = .light;`, a Dart 3.10 feature), with `public_member_api_docs` on:

| the probe constraint | what `dart analyze` reports |
|---|---|
| `>=3.0.0 <5.0.0` | 1 `EXPERIMENT_NOT_ENABLED`, 6 `PUBLIC_MEMBER_API_DOCS` |
| `^3.11.0` | 6 `PUBLIC_MEMBER_API_DOCS` |

The same floor is worse for a whole-file parse. Measured over the 3508 `.dart` files of `flutter/packages` at `a3e763e`, through a probe stating `sdk: '>=3.5.0 <4.0.0'`: `dart analyze` writes `This requires the 'dot-shorthands' language feature to be enabled` as a SYNTACTIC_ERROR. A file the analyzer cannot parse yields no member and no diagnostic of the kind either rule selects, so both rules under-report in silence — which is the shape `builtin/validators/README.md` names as a tool that reads a dirty file as clean.

`function-length-dart` already answers this. Its script reads the version out of `dart --version` and writes `sdk: '^<version>'`, so the probe always parses with the language version of the installed SDK, and the caret keeps the constraint correct across a major version too. The section "The probe package states the language version of the installed SDK" in `builtin/validators/code-hygiene/rules/function-length-dart.md` records the measurement.

## Done when

- `missing-docs-dart` and `magic-numbers-dart` each derive the probe `sdk:` constraint from `dart --version` rather than stating a fixed floor.
- Each rule file records the measurement, the way `function-length-dart` does.
- An acceptance test drives at least one of the two over a file using a language feature newer than the old floor, and holds the run to the findings it must report.

#tool-validators #objectivity