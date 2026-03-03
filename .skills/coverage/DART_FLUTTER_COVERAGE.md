# Dart/Flutter Test Coverage Conventions

## Test File Locations

- **Mirror layout:** `test/` directory mirrors `lib/`. `lib/src/parser.dart` → `test/src/parser_test.dart` or `test/parser_test.dart`.
- **`_test.dart` suffix:** Test files always end with `_test.dart`.
- **Widget tests:** Same `test/` directory, may be under `test/widget/` or `test/screens/`.
- **Integration tests:** `integration_test/` directory at the project root.

For a source file `lib/src/services/auth_service.dart`, look for:
1. `test/src/services/auth_service_test.dart`
2. `test/services/auth_service_test.dart`
3. `test/auth_service_test.dart`

## Treesitter AST Queries

**Find class definitions and methods:**
```scheme
(class_definition
  name: (identifier) @class_name
  body: (class_body
    (method_signature
      name: (identifier) @method_name)))
```

**Find top-level functions:**
```scheme
(function_signature
  name: (identifier) @name)
```

**Find test functions:**
```scheme
(expression_statement
  (invocation_expression
    function: (identifier) @fn
    (#match? @fn "^(test|testWidgets|group)$")
    arguments: (arguments
      (argument (string_literal) @test_name))))
```

## What Requires Tests

- All public classes and their public methods (no leading underscore)
- Riverpod providers and notifiers — test the notifier logic
- Repository and service classes — test through their public interface
- State transformations in notifiers (`build()` method, mutation methods)
- Widget logic (conditional rendering, user interaction handlers)
- Model classes with custom methods beyond `copyWith`/`toJson`/`fromJson`
- Error handling paths and edge cases

## Acceptable Without Direct Tests

- Private classes and functions (`_PrivateHelper`) called from tested public APIs
- `@freezed` or `@JsonSerializable` generated code (`.g.dart`, `.freezed.dart`)
- Simple UI-only widgets with no business logic (pure layout)
- Constants, theme definitions, route declarations
- Generated `part` files

## Test Naming Conventions

Dart tests use `group()` and `test()`/`testWidgets()` with string descriptions. Match source class and method names against test descriptions: `group('AuthService', () { test('login returns token on success', ...) })`.

## Testing Patterns

- `flutter_test` package for widget tests
- `test` package for unit tests
- `expect(value, matcher)` with `equals`, `isA<Type>()`, `throwsA`
- `ProviderContainer` with `overrides` for testing Riverpod providers
- `MockTail` or `Mockito` for mocking dependencies
- `pumpWidget` + `pump` for widget interaction tests
- `WidgetTester.tap`, `.enterText`, `.drag` for user action simulation
