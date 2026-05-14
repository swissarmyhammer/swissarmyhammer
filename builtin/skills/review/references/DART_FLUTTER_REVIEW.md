# Dart/Flutter Review Guidelines (Remi Rousselet school)

Apply these when reviewing Dart or Flutter code. These supplement the universal review layers.

## Immutability

- **Data/model classes use `@freezed` or Dart 3 sealed classes.** Mutable classes holding domain state are a red flag.
- `copyWith` for modifications, never direct field mutation on model objects.
- **Union types for state variants.** Use multiple factory constructors in `@freezed` or sealed classes with pattern matching — not `bool isLoading + T? data + String? error` on a single mutable class.
- Use Dart 3 `switch` expressions and pattern matching over the older `.when`/`.map` helpers.
- If a developer hand-writes `==`, `hashCode`, `toString` on a data class, they should be using `@freezed`.

## Riverpod Providers

- **Providers are top-level `final` declarations.** Never inside classes, widgets, or functions — causes memory leaks.
- **`ref.watch` in `build` only** — creates reactive subscription.
- **`ref.read` in callbacks only** — one-time read without subscription.
- **`ref.listen` for side effects** — navigation, snackbars, logging.
- `ref.read` in `build` as a "performance optimization" is explicitly wrong — makes UI go out of sync.
- `ref.watch` in a callback is wrong — value may be stale.
- Providers self-initialize. A widget calling `ref.read(provider).init()` from `initState` is an anti-pattern — initialization belongs in the provider's `build` method.

## State Management

- **`Notifier`/`AsyncNotifier`**, not deprecated `StateNotifier`/`StateNotifierProvider`.
- Initialization logic in `build()`, not constructors.
- `AsyncValue.guard()` for async error handling — not manual try/catch with `state = AsyncError(...)`.
- `state.valueOrNull` over `state.asData!` — force-unwrapping throws on loading/error.
- `autoDispose` is the correct default. Providers without listeners should not persist. `ref.keepAlive()` is the opt-in exception, and should be conditional (keep on success, dispose on error).

## Ephemeral State

- **Providers are for shared business state**, not widget-local lifecycle concerns.
- Form fields, animation controllers, scroll controllers, selected-item state: use `flutter_hooks` (`useTextEditingController()`, `useAnimationController()`) or `StatefulWidget`.
- A `StateProvider<String>` for a text field is wrong.

## Side Effects

- **Providers represent reads, not writes.** A `FutureProvider` whose body calls `http.post(...)` is wrong.
- Mutations belong in `Notifier` methods triggered by user actions.
- `ref.onDispose` for resource cleanup (StreamControllers, timers). No side-effect-triggering code in `onDispose`.

## Code Generation

- Projects already using `freezed`/`json_serializable` should use `@riverpod` annotations.
- Functional providers (annotated functions) for read-only/derived state.
- Class-based providers (annotated Notifier subclasses) for mutable state with user-triggered methods.
- Parameterized providers expressed as parameters on the annotated function/build method, not `.family` modifier syntax.

## Composition

- Prefer `HookWidget`/`HookConsumerWidget` over `StatefulWidget` for lifecycle-dependent objects (controllers, animations).
- Extract custom hooks (functions prefixed with `use`) when the same hook combination repeats.
- **All hook calls must be unconditional and at the top level of `build`** — never inside `if`, `for`, or callbacks.

## Testability

- Business logic lives in providers/notifiers, not widgets. A widget with `if/else` business logic or direct API calls is untestable.
- One `ProviderContainer` per test — never share between tests.
- Mock at the repository/service layer by overriding providers in `overrides`, not by mocking Notifiers directly.
- Widget tests wrap with `ProviderScope` and override all I/O-touching providers.
