---
title: Flutter Project Guidelines
description: Best practices and tooling for Flutter/Dart projects
partial: true
---

### Flutter Project Guidelines

**FVM check:** if `.fvm/` exists, prefix every command with `fvm` (e.g. `fvm flutter test`). Otherwise call commands directly.

**Testing — do NOT glob; `flutter test` discovers `test/` automatically:**
- All: `flutter test`
- File: `flutter test test/widget_test.dart`
- Coverage: `flutter test --coverage`

**Common commands:**
- Deps: `flutter pub get`
- Run: `flutter run`
- Analyze: `flutter analyze` (lint + static analysis)
- Build: `flutter build apk` / `flutter build ios` (add `--release`)
- Format: `dart format .` (verify: `dart format --output=none --set-exit-if-changed .`) — run before committing

**Best practices:** check for `.fvm/` first; run `flutter analyze` pre-commit; `pubspec.yaml` is the config.

**File locations:** `lib/` (source), `test/` (tests), `assets/`, `pubspec.yaml`. Git-ignored: `build/`, `.dart_tool/`, `.packages`, `.fvm/`.
