---
name: controlled-dependencies
description: Inject Date/UUID/clock/network/filesystem via @Dependency; keep feature logic free of uncontrolled globals
---

# Swift Controlled Dependencies

**Universal:** a function whose signature promises a pure transformation but calls `Date()`, the network, or disk is doing hidden global I/O — flag it as a testability smell and inject the effect as a parameter or dependency.

**Library-conditional (Point-Free `swift-dependencies` / Composable Architecture):** the rules below apply when the changed file imports `Dependencies` or `ComposableArchitecture`, or already uses `@Dependency`. In such a file, calling an uncontrolled global from feature/business logic is a violation, not a preference — route it through `@Dependency`. Skip these for files that haven't adopted the library.

- **No `Date()` / `Date.now` for the current time.** DO: `@Dependency(\.date.now) var now`.
- **No `UUID()` to mint identifiers.** DO: `@Dependency(\.uuid) var uuid` … `uuid()`.
- **No `Task.sleep` / `DispatchQueue.asyncAfter` for delays — use an injected clock.** DO: `@Dependency(\.continuousClock) var clock` … `try await clock.sleep(for: .seconds(1))`.
- **No `URLSession.shared` or raw network calls in a reducer/model** — wrap behind an injected API-client dependency.
- **No direct `FileManager.default`, `UserDefaults.standard`, `NotificationCenter.default`, `Bundle.main`, `Locale.current`, `TimeZone.current`, or `Calendar.current`** in feature logic — inject the controlled version (`@Dependency(\.locale)`, `@Dependency(\.calendar)`, a purpose-built client, etc.).
- **No uncontrolled randomness** (`Int.random(in:)`, `.randomElement()`, `SystemRandomNumberGenerator`) in feature logic. DO: `@Dependency(\.withRandomNumberGenerator)`.
- **No reaching through `.shared` / `static let shared` singletons from feature logic** — inject the collaborator instead.
- **A dependency added to observed state is `@ObservationIgnored`** — dependencies are not state. DO: `@ObservationIgnored @Dependency(\.uuid) var uuid`.
