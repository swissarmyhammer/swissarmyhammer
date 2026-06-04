---
assignees:
- claude-code
position_column: todo
position_ordinal: '80'
project: claude-hooks
title: Share a JSONC reader primitive in swissarmyhammer-common
---
Claude `.claude/settings.json` files are routinely JSONC (line/block comments, trailing commas) — mirdan already handles this in `crates/mirdan/src/jsonc.rs` (`parse_jsonc`, via the `jsonc_parser` crate) and `crates/mirdan/src/settings.rs::read_json`. The new hook-settings loader (next task) needs the same JSONC tolerance, but `agent-client-protocol-extras` must NOT depend on mirdan (wrong layering, and mirdan is the install/deploy system). Per the "no duplicate-but-different code" rule, do NOT re-implement JSONC parsing — share it.

## Scope
- Add a JSONC parse helper to `swissarmyhammer-common` (e.g. `json::parse_jsonc(&str) -> Result<serde_json::Value, serde_json::Error>` and a `read_json_file(&Path)` that returns an empty object for missing/blank files). `agent-client-protocol-extras` already depends on `swissarmyhammer-common`, so this is reachable from both the loader and mirdan.
- Refactor `mirdan/src/jsonc.rs` + `mirdan/src/settings.rs::read_json` to delegate to the common helper, preserving mirdan's existing `RegistryError` wrapping and the "empty object on missing/blank" behavior (no behavior change).
- Move the `jsonc_parser` dependency to the workspace/common as needed.

## Acceptance criteria
- One JSONC implementation, owned by `swissarmyhammer-common`; mirdan re-uses it (no second parser).
- All existing mirdan settings/jsonc tests still pass unchanged.
- New common-crate tests: line comments, block comments, trailing commas, empty input (EOF error like serde_json), missing file → empty object, invalid JSON → error preserving line/column in Display.

## Notes
If extracting to common proves disproportionately invasive, the acceptable fallback is to have the loader use the `jsonc_parser` crate directly via a tiny shared module — but the preferred outcome is a single shared primitive. Document the choice in the PR.