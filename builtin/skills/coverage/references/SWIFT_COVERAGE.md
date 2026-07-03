# Swift Test Coverage

## Running Coverage

**Swift package (`Package.swift`)** — the primary path:

```bash
# Full package, with coverage instrumentation
swift test --enable-code-coverage
```

Instrumentation lands in the build dir, not as LCOV. Convert with `llvm-cov export`:

```bash
BIN_PATH=$(swift build --show-bin-path)
XCTEST=$(find "$BIN_PATH" -maxdepth 1 -name '*.xctest')

# macOS: the test binary is inside the bundle; use xcrun to get the matching llvm-cov
COV_BIN="$XCTEST/Contents/MacOS/$(basename "$XCTEST" .xctest)"
xcrun llvm-cov export -format=lcov "$COV_BIN" \
  -instr-profile "$BIN_PATH/codecov/default.profdata" \
  -ignore-filename-regex='(\.build|Tests)/' > lcov.info
```

```bash
# Linux: the .xctest path IS the executable; llvm-cov comes from the Swift toolchain
llvm-cov export -format=lcov "$XCTEST" \
  -instr-profile "$BIN_PATH/codecov/default.profdata" \
  -ignore-filename-regex='(\.build|Tests)/' > lcov.info
```

Works for both XCTest and Swift Testing (`@Test`) suites — `swift test` runs both under the same instrumentation.

**Xcode project/workspace (no `Package.swift`)** — fallback for app targets:

```bash
xcodebuild test -scheme <Scheme> -destination 'platform=macOS' \
  -enableCodeCoverage YES -resultBundlePath build/coverage.xcresult
xcrun xccov view --report --json build/coverage.xcresult > coverage.json
```

`xccov` emits its own JSON (per-target → per-file → per-function `lineCoverage` 0–1 fractions), not LCOV — parse that directly instead of `DA:` lines. When a repo has both, prefer the SwiftPM path.

## Output

SwiftPM path writes `lcov.info` at the package root. Parse `DA:<line>,<hits>` per file as usual. The `-ignore-filename-regex` keeps `.build/` checkouts and test sources out of the denominator.

## Scoping

- `swift test --filter <TestTarget>` (or `--filter '<TestClass>/<testMethod>'`, regex allowed) scopes the test run — coverage still instruments the whole build, so parse only in-scope files from the LCOV.
- Multiple packages in one repo: `cd` into the package directory first; each package gets its own `.build` and `lcov.info`.
- Coverage of dependencies shows up under `.build/checkouts/` — always excluded by the ignore regex.

## Test File Locations

- **Mirror layout:** `Tests/<TargetName>Tests/` mirrors `Sources/<TargetName>/`. `Sources/Parser/Lexer.swift` → `Tests/ParserTests/LexerTests.swift`
- **XCTest:** `final class LexerTests: XCTestCase` with `test...` methods
- **Swift Testing:** `import Testing` with `@Test` functions / `@Suite` structs (Swift 6 toolchains; runs under `swift test` alongside XCTest)

## What Requires Tests

- All `public` and `package` types, methods, and free functions
- Throwing functions — both the success path and each thrown error
- `Codable` conformances with custom `init(from:)`/`encode(to:)` or `CodingKeys`
- Actors and `async` functions (test with `await`; cover cancellation where it matters)
- Protocol conformances with non-trivial logic (`Equatable`/`Comparable`/`Hashable` written by hand)
- Business logic in ViewModels / ObservableObject / `@Observable` classes

## Acceptable Without Direct Tests

- `private`/`fileprivate` helpers (covered through their public callers)
- Compiler-synthesized conformances (derived `Equatable`/`Hashable`/`Codable`)
- SwiftUI `View.body` with no conditional logic; previews (`#Preview`, `PreviewProvider`)
- `@main` entry points and argument-parsing wiring
- Generated sources (`.pb.swift` protobuf, SwiftGen/Sourcery output, `Package.swift` itself)
