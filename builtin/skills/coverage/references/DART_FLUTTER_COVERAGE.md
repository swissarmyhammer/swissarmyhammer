# Dart/Flutter Test Coverage

## Running Coverage

**Flutter**

```bash
# Full project
flutter test --coverage

# Specific test file
flutter test --coverage test/auth_service_test.dart
```

Output: `coverage/lcov.info`

**Dart (non-Flutter)**

```bash
# Full project
dart test --coverage=coverage
# Then convert to LCOV
dart pub global activate coverage
dart pub global run coverage:format_coverage \
  --lcov --in=coverage --out=coverage/lcov.info \
  --report-on=lib
```

## Output

Both write `coverage/lcov.info`. Parse `DA:<line>,<hits>` lines per file.

## Scoping

- Pass specific test files as positional args to scope the test run
- Flutter: use `--coverage` flag with any test subset
- For packages in a monorepo, `cd` into the package directory first

## Test File Locations

- **Mirror layout:** `test/` mirrors `lib/`. `lib/src/parser.dart` → `test/src/parser_test.dart`
- **`_test.dart` suffix:** Test files always end with `_test.dart`
- **Integration tests:** `integration_test/` directory

## What Requires Tests

- All public classes and their public methods (no leading underscore)
- Riverpod providers and notifiers
- Repository and service classes
- State transformations in notifiers
- Widget logic (conditional rendering, user interaction handlers)
- Model classes with custom methods beyond `copyWith`/`toJson`/`fromJson`

## Acceptable Without Direct Tests

- Private classes and functions (`_PrivateHelper`)
- `@freezed` or `@JsonSerializable` generated code (`.g.dart`, `.freezed.dart`)
- Simple UI-only widgets with no business logic
- Constants, theme definitions, route declarations
- Generated `part` files
