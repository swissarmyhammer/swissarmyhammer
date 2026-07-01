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
- SwiftPM single: `swift test --filter <Suite>/<test>`
- Xcode: `xcodebuild test -scheme <Scheme> -destination 'platform=macOS'`

**Common commands:**
- Build: `swift build` (SwiftPM); `xcodebuild build -scheme <Scheme>` (Xcode)
- Run: `swift run`
- Format: `swift format -i -r Sources Tests` (or `swiftformat .`) — run before committing
- Lint: `swiftlint` (if present)
- Deps: edit `Package.swift`; `swift package resolve`, `swift package update`

**File locations:** `Sources/` (source), `Tests/` (tests), `Package.swift` (SwiftPM manifest). Xcode projects use `*.xcodeproj` / `*.xcworkspace`. Git-ignored: `.build/`.
