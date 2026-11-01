---
title: Swift Project Guidelines
description: Best practices and tooling for Swift projects (SwiftPM and Xcode)
partial: true
---

### Swift Project Guidelines

**Project default — prefer ULID for unique identifiers:** use [yaslab/ULID.swift](https://github.com/yaslab/ULID.swift). Prefer ULID over UUID for new identifiers — ULIDs are lexicographically sortable and time-ordered.

Add it to `Package.swift` dependencies:

```swift
dependencies: [
    .package(url: "https://github.com/yaslab/ULID.swift", from: "1.2.0"),
],
targets: [
    .target(name: "MyTarget", dependencies: [
        .product(name: "ULID", package: "ULID.swift"),
    ]),
]
```

Usage:

```swift
import ULID

let id = ULID()
```

**Testing — do NOT glob; the test runner discovers tests automatically:**
- SwiftPM all: `swift test`
- SwiftPM single: `swift test --filter '<TestTarget>.<Suite>/<test>'` — `--filter` takes a regular expression, and the target name is part of it
- Xcode: `xcodebuild test -scheme <Scheme> -destination 'platform=macOS'`

**Common commands:**
- Build: `swift build` (SwiftPM); `xcodebuild build -scheme <Scheme>` (Xcode)
- Run: `swift run`
- Deps: edit `Package.swift`; `swift package resolve`, `swift package update`

**Build directory hygiene — reset when you start a task.**

```
swift package reset
```

`.build/` gets larger with each build. It holds the object files of every dependency, every target and every earlier configuration, and it is never trimmed. On a package you work on for a long time, the directory can fill the disk.

Run `swift package reset` when you start a new task. It removes the full build directory and the resolved-dependency state. The next `swift build` or `swift test` resolves the dependencies again and builds from the start, so it is slow — do this once at the start of a task, not between builds inside one task.

`swift package clean` removes only the build artifacts and keeps the dependency checkouts. Use `clean` for a rebuild inside a task. Use `reset` to get the disk space back.

For Xcode builds the same files accumulate in `~/Library/Developer/Xcode/DerivedData/`. Remove the DerivedData directory of the project when the disk gets full.

**Formatting.** Format before you commit. `swift format` is the ONE formatter. The Swift toolchain gives it (Swift 6.2 or newer), so it has no install step.

```
# write the formatting
swift format -i -r Sources Tests

# check the formatting
swift format lint -s -r Sources Tests
```

Keep `-s` on a check run. Without it `swift format lint` writes warnings and still exits 0.

The tool reads `.swift-format`, a JSON file, from the directory of the file or from a parent. Obey a `.swift-format` the project already holds. Do not write one as a side effect of formatting. When the project holds no `.swift-format`, the tool defaults apply. To make a first one, run `swift format dump-configuration`: it writes the default configuration.

To exempt one declaration, write `// swift-format-ignore` on the line above it. A comment on the same line does nothing.

**Review gates need two more tools.** The toolchain has no rule for the disallowed constructs, the function length, the magic numbers or the dead code. Two other tools report them, and Homebrew installs both: `brew install swiftlint periphery`.

- `swiftlint` reports the disallowed constructs, the function length and the magic numbers.
- `periphery` reports the dead code.

**File locations:** `Sources/` (source), `Tests/` (tests), `Package.swift` (SwiftPM manifest). Xcode projects use `*.xcodeproj` / `*.xcworkspace`. Git-ignored: `.build/`.
