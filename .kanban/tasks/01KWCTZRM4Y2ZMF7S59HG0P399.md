---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kwcvbwxa77f1pjhwe31555j6
  text: |-
    Implemented Swift project detection end-to-end.

    Files changed:
    - crates/swissarmyhammer-project-detection/src/types.rs: ProjectType::Swift variant (serde "swift"), marker_files [Package.swift, *.xcodeproj, *.xcworkspace], ProjectSymbols.swift field + get() arm; updated 3 enumerated tests.
    - crates/swissarmyhammer-project-detection/src/detect.rs: added Swift to detect_project_at_path priority list.
    - crates/swissarmyhammer-tools/src/mcp/tools/code_context/detect.rs: project_type_name=>"Swift", project_type_key=>"swift", partial_name_for_type=>Some("_partials/project-types/swift"); added Swift to test_project_type_name_all_types, test_all_project_types_have_renderable_guidelines, test_project_type_key_all_types.
    - crates/swissarmyhammer-lsp/src/registry.rs: added Swift to test_all_project_types_queryable.
    - builtin/lsp/sourcekit-lsp.yaml: project_types [] -> [swift].
    - builtin/project-detection/config.yaml: added swift glyph (reused sourcekit-lsp icon U+E699).
    - builtin/_partials/project-types/swift.md: new partial covering SwiftPM + Xcode, prefers ULID via yaslab/ULID.swift (Package.swift dep entry, product ULID, `let id = ULID()`).

    Verification (all green):
    - cargo nextest run -p swissarmyhammer-project-detection: 31/31 passed
    - cargo nextest run -p swissarmyhammer-lsp: 223/223 passed
    - cargo nextest run -p swissarmyhammer-tools code_context::detect: 26/26 passed (incl. renderable-guidelines test rendering swift partial through Liquid)
    - cargo fmt: clean (reverted unrelated formatting drift in review/validators files I did not touch)
    - cargo clippy --workspace --all-targets -- -D warnings: exit 0, no warnings
  timestamp: 2026-06-30T18:06:22.634599+00:00
- actor: claude-code
  id: 01kwcvf0k54vaq7rnqg63t7zp0
  text: 'Implement landed green in `doing`. 6 files changed + new partial `builtin/_partials/project-types/swift.md`. Verified: project-detection 31/31, lsp 223/223, tools code_context::detect 26/26 (swift partial renders through Liquid), fmt clean, clippy -D warnings clean, adversarial double-check PASS. config.yaml swift glyph is byte-identical to sourcekit-lsp.yaml icon (U+E699). sourcekit-lsp.yaml wired project_types: [swift]. Next: full /test → checkpoint commit → /review HEAD~1..HEAD.'
  timestamp: 2026-06-30T18:08:04.709915+00:00
position_column: doing
position_ordinal: '8480'
title: Add Swift project detection + project-types partial (prefer ULID via yaslab/ULID.swift)
---
Add first-class Swift support to project detection and create the matching `swift` project-types partial. The partial filename MUST match the detection key string (`project_type_key` == "swift" → `builtin/_partials/project-types/swift.md`), exactly as `flutter`, `java-gradle`, etc. do today.

## Architecture note — there is NO duplicate detection
Both the code-context `detect projects` op AND the LSP registry share ONE `ProjectType` enum (`swissarmyhammer-project-detection`; `swissarmyhammer-lsp/src/registry.rs:9` and `types.rs:14` import it). The thing that looks like duplication is actually an INCOMPLETE enum: Swift has no `ProjectType` variant, so `builtin/lsp/sourcekit-lsp.yaml` declares `project_types: []` and the Swift LSP is routed only by `file_extensions: [swift]` (the extension-fallback path `servers_for_extensions`). Kotlin and Ruby are in the same boat (also `project_types: []`) — but those are OUT OF SCOPE here. Adding `ProjectType::Swift` to the single enum fixes both layers at once: code-context detects Swift projects, and the LSP can be routed by project type.

## Background / how the pieces wire together
- `crates/swissarmyhammer-project-detection/src/types.rs`: `ProjectType` enum (serde `rename_all = "lowercase"`), `ProjectSymbols` struct + `get()`, `marker_files()`, and tests that enumerate every variant.
- `crates/swissarmyhammer-project-detection/src/detect.rs`: `detect_project_at_path` priority list (must include the new variant or it is never detected).
- `crates/swissarmyhammer-tools/src/mcp/tools/code_context/detect.rs`: `project_type_name` (display), `project_type_key` (stable key, matches serde rename + partial filename), `partial_name_for_type` (`Some("_partials/project-types/{key}")`). Tests `test_all_project_types_have_renderable_guidelines`, `test_project_type_name_all_types`, `test_project_type_key_all_types` enumerate every variant and render each partial through Liquid — they will fail until the new variant is added everywhere AND the partial file exists and renders non-empty.
- `builtin/project-detection/config.yaml`: `symbols:` map; `ProjectSymbols::default()` parses this and will fail if a new struct field has no corresponding yaml key.
- `crates/swissarmyhammer-lsp/src/registry.rs`: `test_all_project_types_queryable` enumerates variants — add Swift there too.
- `builtin/lsp/sourcekit-lsp.yaml`: currently `project_types: []`.

## Work items

### 1. types.rs (project-detection)
- Add `Swift` variant to `ProjectType` (lowercase serde => "swift").
- Add `marker_files()` arm: `&["Package.swift", "*.xcodeproj", "*.xcworkspace"]` (Package.swift = SwiftPM; xcodeproj/xcworkspace = Xcode).
- Add `swift: String` field to `ProjectSymbols`; add `ProjectType::Swift => &self.swift` to `get()`.
- Update the variant lists / symbol assertions in the types.rs tests (`project_symbols_default_loads_successfully`, `project_symbols_get_returns_nonempty_for_all_variants`, `project_symbols_get_maps_variants_to_correct_fields`).

### 2. config.yaml
- Add `swift: "<glyph> "` to the `symbols:` map (Nerd Font swift glyph — reuse the same glyph as `builtin/lsp/sourcekit-lsp.yaml`'s `icon` for consistency).

### 3. detect.rs (project-detection crate)
- Add `ProjectType::Swift` to the `project_types` array in `detect_project_at_path`.

### 4. code_context/detect.rs (tools crate)
- `project_type_name`: `ProjectType::Swift => "Swift"`.
- `project_type_key`: `ProjectType::Swift => "swift"`.
- `partial_name_for_type`: `ProjectType::Swift => Some("_partials/project-types/swift")`.
- Add `Swift` to the variant lists in `test_project_type_name_all_types`, `test_all_project_types_have_renderable_guidelines`, and `test_project_type_key_all_types` (expected key "swift").

### 5. LSP wiring
- `crates/swissarmyhammer-lsp/src/registry.rs`: add `ProjectType::Swift` to the loop in `test_all_project_types_queryable`.
- `builtin/lsp/sourcekit-lsp.yaml`: change `project_types: []` → `project_types:\n  - swift` so the Swift LSP routes by project type (not extension-only). Confirm the YAML still loads (LSP_REGISTRY tests).

### 6. New partial: builtin/_partials/project-types/swift.md
Follow the exact format of the existing partials (frontmatter `title`/`description`/`partial: true`, then `### Swift Project Guidelines`, testing section, common commands, file locations). Cover both SwiftPM and Xcode flows. Include the project default convention:

**Prefer ULID for unique identifiers** as the default — use https://github.com/yaslab/ULID.swift (SwiftPM package URL `https://github.com/yaslab/ULID.swift`, product `ULID`). Show adding it to `Package.swift` dependencies and a one-line usage example (`let id = ULID()`), and state ULID is preferred over UUID for new identifiers.

Suggested command coverage:
- Testing (do NOT glob; `swift test` discovers tests; Xcode uses `xcodebuild test`): `swift test`, `swift test --filter <Suite>/<test>`.
- Build/run: `swift build`, `swift run`.
- Format/lint: `swift format` (or `swiftformat`), `swiftlint` if present.
- Deps: edit `Package.swift`; `swift package resolve` / `swift package update`.
- File locations: `Sources/`, `Tests/`, `Package.swift`; git-ignored `.build/`.

## Verification
- `cargo nextest run -p swissarmyhammer-project-detection`
- `cargo nextest run -p swissarmyhammer-lsp`
- `cargo nextest run -p swissarmyhammer-tools` (detect tests — confirms the partial renders non-empty and frontmatter is stripped).
- `cargo fmt` + `cargo clippy -- -D warnings`.

## Out of scope
- Kotlin / Ruby (same empty-`project_types` situation) — separate follow-up.