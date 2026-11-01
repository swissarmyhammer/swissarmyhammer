---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m6c4b21s455y309m9ns26gyx
  text: |-
    ### implement — changed
    - evidence: files changed: crates/swissarmyhammer-config/src/model.rs, crates/swissarmyhammer-config/src/model/tests.rs, crates/swissarmyhammer-config/src/model/tests/types.rs, crates/swissarmyhammer-config/src/model/tests/resolution.rs. Decision: keep the `platform:` key as a typed one-variant check (without it serde would silently accept `platform: linux-x86-64`); remove Platform::current() and the platform filter in select_executor (it takes the first executor; empty list gives "model config lists no executors"). builtin/models/qwen-embedding.yaml unchanged. Only users of the removed variants were in config tests; git grep finds none elsewhere. RED: model_config_naming_a_removed_platform_is_rejected failed first (macos-x86-64 was accepted). Then `cargo nextest run -p swissarmyhammer-config -p swissarmyhammer-embedding`: 424 passed, 0 failed; `cargo clippy ... --all-targets` clean; `cargo fmt --check` clean.
    - next: /review
  timestamp: 2026-11-01T18:00:56.121867+00:00
- actor: claude-code
  id: 01m6c4y95qxn889hk3mgxv9phh
  text: |-
    ### test — green
    - evidence: cargo check --workspace --all-targets clean; cargo nextest run --workspace --override-version-check: 13241 passed, 0 failed, 0 skipped; cargo clippy --workspace --all-targets -- -D warnings clean; cargo fmt --all --check clean
    - next: review
  timestamp: 2026-11-01T18:11:26.007657+00:00
- actor: claude-code
  id: 01m6c4z7307hpemcc7e5rdhcfc
  text: |-
    ### commit — changed
    - evidence: one local commit, 4 crate files (swissarmyhammer-config model.rs, tests.rs, tests/types.rs, tests/resolution.rs) plus kanban ledger files of this batch. Commit sha is in the final report of the commit step (a sha cannot sit inside its own commit).
    - next: review
  timestamp: 2026-11-01T18:11:56.640666+00:00
position_column: doing
position_ordinal: '80'
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