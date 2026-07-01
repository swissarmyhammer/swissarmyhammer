---
name: composable-architecture
description: Value-typed @ObservableState, pure reducers, effects via .run, actions as events, exhaustive TestStore
---

# Swift Composable Architecture

These rules apply **only to files that use the Composable Architecture** — detected by `import ComposableArchitecture`, `@Reducer`, `Reduce { }`, `Effect`, `Store`, or `TestStore`. Skip them entirely for plain SwiftUI/UIKit files.

- **Feature `State` is a value type with `@ObservableState`, conforming to `Equatable`.** DON'T: `final class State`. DO: `@ObservableState struct State: Equatable { … }`.
- **Reducers are pure: no side effects performed inline in the `body`/`Reduce` closure.** Any I/O (network, disk, dates, analytics) is returned as an `Effect`, never executed while computing the next state.
  - DON'T: `case .saveTapped: try? database.save(state.item); return .none`
  - DO: `case .saveTapped: return .run { [item = state.item] _ in try await database.save(item) }`
- **Effects capture the state values they need in the capture list; don't read `state` inside a `.run` closure** (`state` is `inout`, not valid to read asynchronously). Feed effect results back as actions via `await send(...)` — state only changes in the reducer body.
- **A synchronous, effect-free action returns `.none`** — not an effect that secretly does work.
- **`Action` is a flat `enum` of events, switched exhaustively — no `default:` that swallows new actions.** Name UI actions after what happened, not imperative commands. DO: `case saveButtonTapped`, `case factResponse(String)`. DON'T: `case save`, `case fetchFact`.
- **Compose children with the provided operators** (`Scope`, `ifLet`, `forEach`, presentation reducers), and model mutually-exclusive navigation as one `@CasePathable` `@Presents` destination enum, not several optional child-state properties.
- **`TestStore` assertions are exhaustive and hermetic.** Every `send` that mutates state asserts the change with absolute values (`{ $0.count = 1 }`, not `+= 1`); every effect-emitted action is asserted with `store.receive(...)`; every dependency is controlled via `withDependencies:` (immediate/test clock, no live network). DON'T leave snapshot recording on (`record: .all`, `isRecording = true`) in committed tests.
