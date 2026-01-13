---
title: Flutter Project Guidelines
description: Best practices and tooling for Flutter/Dart projects
partial: true
---

### Flutter Project Guidelines

**Flutter Version Management:**
- Check for `.fvm/` directory → using Flutter Version Management (FVM)
- If using FVM, prefix all commands with `fvm`: e.g., `fvm flutter test`
- Without FVM, use commands directly: `flutter test`

**Common Commands:**
- Install dependencies: `flutter pub get` or `fvm flutter pub get`
- **Run ALL tests:** `flutter test` or `fvm flutter test`
- **Run specific test file:** `flutter test test/widget_test.dart`
- **Run tests with coverage:** `flutter test --coverage`
- Build (debug): `flutter build apk` / `flutter build ios` (or with `fvm` prefix)
- Build (release): `flutter build apk --release` / `flutter build ios --release`
- Run app: `flutter run` or `fvm flutter run`
- Analyze code: `flutter analyze` or `fvm flutter analyze`
- Format code: `dart format .` or `fvm dart format .`

**IMPORTANT:** Do NOT glob for test files. Use `flutter test` to run all tests - it automatically discovers all test files in the `test/` directory.

**Best Practices:**
- Always check for FVM before running commands (look for `.fvm/` directory)
- Run `flutter analyze` before committing to catch potential issues
- Use `dart format .` to maintain consistent code style
- Check `pubspec.yaml` for project configuration and dependencies
- Run tests frequently during development

**File Locations:**
- Source code: `lib/`
- Tests: `test/`
- Assets: `assets/` (referenced in `pubspec.yaml`)
- Configuration: `pubspec.yaml` (dependencies, assets, metadata)
- Build output: `build/` (git-ignored)
- Dependencies: `.dart_tool/`, `.packages` (git-ignored)
- FVM config: `.fvm/` (if using FVM)
