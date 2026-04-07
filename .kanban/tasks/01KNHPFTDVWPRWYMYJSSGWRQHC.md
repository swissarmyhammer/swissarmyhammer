---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffff8580
title: 'Coverage: daemon.rs initialize_handshake error paths'
---
## What

`swissarmyhammer-lsp/src/daemon.rs` — `initialize_handshake()` has uncovered branches for malformed LSP responses: bad JSON, missing fields, wrong request ID. These error paths are never exercised.

## Acceptance Criteria
- [ ] Tests cover bad JSON-RPC response (unparseable JSON)
- [ ] Tests cover response with missing required fields
- [ ] Tests cover response with wrong/mismatched request ID

## Tests
- [ ] Add tests in `swissarmyhammer-lsp/src/daemon.rs` (or `tests/`) using a mock process that sends bad JSON-RPC responses
- [ ] Each error path returns the expected error variant
- [ ] `cargo test -p swissarmyhammer-lsp initialize_handshake` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass. #coverage-gap