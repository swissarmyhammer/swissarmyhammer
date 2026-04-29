---
assignees:
- claude-code
depends_on:
- 01KQD0DF12DJ1WPZ4MMWF69DAQ
- 01KQD0DW1GAW3KF2A33FQ0JZT3
position_column: todo
position_ordinal: ff8980
project: acp-upgrade
title: 'ACP 0.11: claude-agent: validation modules'
---
## What

Migrate validation modules to ACP 0.11.

Files:
- `claude-agent/src/capability_validation.rs`
- `claude-agent/src/request_validation.rs`
- `claude-agent/src/agent_validation.rs`
- `claude-agent/src/content_capability_validator.rs`
- `claude-agent/src/content_security_validator.rs`
- `claude-agent/src/content_security_integration_tests.rs`
- `claude-agent/src/mime_type_validator.rs` (no ACP refs but verify)
- `claude-agent/src/path_validator.rs` (no ACP refs but verify)
- `claude-agent/src/size_validator.rs` (no ACP refs but verify)
- `claude-agent/src/url_validation.rs` (no ACP refs but verify)

## Branch state at task start

B0 + B1 landed.

## Acceptance Criteria
- [ ] These modules compile under `cargo check -p claude-agent`. Downstream modules may still fail.
- [ ] One commit on `acp/0.11-rewrite`.

## Tests
- [ ] Inline tests in these files pass. `content_security_integration_tests.rs` is integration-style; ensure it compiles even if other tests fail.

## Depends on
- 01KQD0DF12DJ1WPZ4MMWF69DAQ (B0).
- 01KQD0DW1GAW3KF2A33FQ0JZT3 (B1).