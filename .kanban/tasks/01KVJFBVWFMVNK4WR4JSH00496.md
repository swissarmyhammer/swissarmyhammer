---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffc980
title: Raise shell max_command_length cap 4096 → 256 KiB
---
## What

`sah`'s shell executor rejected ordinary ~8 KB commands with "command too long" — the `max_command_length` cap was **4096 chars**, an arbitrary self-inflicted limit, not a system constraint. The real ceiling is `ARG_MAX` (`getconf ARG_MAX` = 1048576 / 1 MiB on macOS & Linux), since commands are handed to the shell via `execve`; heredoc bodies (the common trigger) don't even bottleneck on argv.

Raise the default to **256 KiB (262144)** — far under ARG_MAX — and single-source it through a new `DEFAULT_MAX_COMMAND_LENGTH` const so the three previously-duplicated `4096` literals can't drift.

Files:
- `crates/swissarmyhammer-shell/src/config.rs` — add `pub const DEFAULT_MAX_COMMAND_LENGTH: usize = 256 * 1024` (documented with ARG_MAX rationale); `default_max_command_length()` returns it.
- `crates/swissarmyhammer-shell/src/security.rs` — `MAX_COMMAND_LENGTH` aliases `crate::config::DEFAULT_MAX_COMMAND_LENGTH`.
- `builtin/shell/config.yaml` — `settings.max_command_length: 262144` (the runtime-operative value via stacked config).
- `crates/swissarmyhammer-shell/src/lib.rs` — re-export `DEFAULT_MAX_COMMAND_LENGTH`.
- `crates/swissarmyhammer-tools/src/mcp/tools/shell/execute_command/mod.rs` — `TEST_COMMAND_EXCEEDS_LIMIT_LENGTH` derived as `DEFAULT_MAX_COMMAND_LENGTH + 1000`.
- `crates/swissarmyhammer-shell/tests/config_stacking_test.rs` — update stale "4096" comment.

## Acceptance Criteria
- [x] Default `ShellSettings.max_command_length` resolves to 262144 (from builtin YAML and serde default).
- [x] The value is defined once (`DEFAULT_MAX_COMMAND_LENGTH`) and referenced by `security.rs` and the serde default — no duplicated `4096` literal remains for command length.
- [x] A 5000-char command that was previously rejected now passes the default policy; a >262144-char command is still rejected with `CommandTooLong`.

## Tests
- [x] `cargo test -p swissarmyhammer-shell` — config default/merge tests assert `DEFAULT_MAX_COMMAND_LENGTH`.
- [x] `cargo test -p swissarmyhammer-tools --lib` shell length tests — passed (boundary derived from the const).
- [x] `cargo build -p swissarmyhammer-shell` — exit 0.

## Workflow
- Implemented + committed, then reviewed via the `review` engine on `HEAD~1..HEAD` (two rounds). Final commit: `1d91a23e3`.

## Review Findings (2026-06-20 07:21)

### Warnings
- [x] `crates/swissarmyhammer-shell/src/config.rs` — extract `1024` env-value default to `pub const DEFAULT_MAX_ENV_VALUE_LENGTH`, parallel to `DEFAULT_MAX_COMMAND_LENGTH`. **FIXED** — const added, `security.rs MAX_ENV_VALUE_LENGTH` aliases it, re-exported from `lib.rs`.

### Nits
- [ ] `config_stacking_test.rs` — `4990` / `5000` test magic numbers. **Declined** — pre-existing test data not introduced by this change.

## Review Findings (2026-06-20 07:29) — round 2 (after warning fix)

0 blockers, 0 warnings, 8 nits (all "name this test magic number").

- [x] `config.rs` test assertions used bare `1024` for `max_env_value_length`. **FIXED** — now reference `DEFAULT_MAX_ENV_VALUE_LENGTH`.
- [x] `execute_command/mod.rs` `TEST_ENV_VALUE_EXCEEDS_LIMIT_LENGTH = 2000`. **FIXED** — now derived `DEFAULT_MAX_ENV_VALUE_LENGTH * 2`, mirroring the command-length boundary.
- [ ] Arbitrary test override values `8192` / `16384` / `4990` (config.rs + config_stacking_test.rs). **Declined** — pre-existing, genuinely-arbitrary test data; naming adds no signal and chasing it is review-churn.

**Final disposition:** review gate run twice; the one substantive finding (warning) and the two house-convention consistency nits are fixed and verified green; remaining nits are pre-existing arbitrary test data, declined. Loop capped to avoid review-churn. Card → Done. #shell-config #config