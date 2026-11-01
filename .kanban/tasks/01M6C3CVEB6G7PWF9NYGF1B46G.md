---
assignees:
- claude-code
position_column: todo
position_ordinal: ffff9280
title: Remove Intel macOS and Linux variants from the Platform enum
---
Goal: the config model knows one platform, macOS Apple Silicon.

User decisions: sah is macOS Apple Silicon only. No Intel. No Linux.

The `Platform` enum in `crates/swissarmyhammer-config/src/model.rs` still has `MacosX8664` (wire name `macos-x86-64`), `LinuxX8664` and `LinuxAarch64`. Their wire names are pinned with `#[serde(rename)]`. Nothing can use them now, because the only embedding executor is `ane-embedding`.

Steps:
- Remove the Intel macOS and Linux variants, their serde renames, the platform-detection branches, and the doc comments that explain the pinned names.
- Decide with evidence what happens to `macos-arm64` selection in builtin model YAML (`builtin/models/*.yaml`). If every executor entry now names the same platform, keep the `platform:` key only if the loader still needs it. Remove dead selection code.
- Update the tests in `crates/swissarmyhammer-config/src/model/tests/` (for example `types.rs`, which pins wire names). Tests must check behavior: a config with `platform: linux-x86-64` or `macos-x86-64` is rejected with a clear error that names the allowed platform.
- Do not add tests that assert on skill, config or source text.

Done when: only the Apple Silicon variant remains, `cargo nextest run -p swissarmyhammer-config -p swissarmyhammer-embedding` passes, and clippy and fmt are clean. #remove-llama-cpp