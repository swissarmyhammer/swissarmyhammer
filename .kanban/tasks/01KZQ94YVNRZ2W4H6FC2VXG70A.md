---
assignees:
- claude-code
position_column: todo
position_ordinal: ffce80
title: missing-docs-dart probe resolves no dependency, so every Flutter @override build() reports
---
`builtin/validators/code-hygiene/rules/missing-docs-dart.md` builds a probe package that declares no dependencies. An import of `package:flutter/material.dart` therefore does not resolve inside the probe.

`public_member_api_docs` carves out a member that overrides a member the analyzer CAN RESOLVE. The documentation of the overridden member stands for it. When the supertype does not resolve, the analyzer sees no override, and the member reports.

The result: every `@override Widget build(BuildContext context)`, every `createState`, `initState`, `dispose` and `didUpdateWidget` of a Flutter project reports a missing doc comment. The project's own `dart analyze` reports none of them.

Measured on Dart SDK 3.11.0 with a two-package probe. Package `framework` declares `abstract class Widget { String build(); }`, both documented. Package `app` path-depends on it and holds `lib/screen.dart` with `class Screen extends Widget`, one `@override String build()` and one undocumented `void undocumented()`.

- The `app` package analyzed in place, with `public_member_api_docs` on: **1** finding, the undocumented method. The override is silent.
- The shipped probe over the same file: **2** findings. The override is the false one.
- The probe run as `dart analyze --packages=<app>/.dart_tool/package_config.json <probe>`: **1** finding, the same as the project.

`--packages` restores the carve-out and the lint still fires, so the fix is one flag. Two questions the work has to answer:

- Where the package config comes from. The workspace root holds `.dart_tool/package_config.json` only after the project ran `flutter pub get` or `dart pub get`. A monorepo holds one per package.
- What the doctor run does. The doctor runs the script in a scratch fixtures directory that holds no project and no package config, so the flag has to be conditional or the fixture pair breaks.

`Object` always resolves, so `toString`, `operator ==` and `hashCode` keep the carve-out with or without this fix.

Found while implementing ^j0g7yk1, which fixed a different axis of the same probe (the position and generator exclude list). #tool-validators #objectivity